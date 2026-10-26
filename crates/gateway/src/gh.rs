use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result};
use gitassist_core::config::Config;
use gitassist_core::tab::{classify_gh, Tab};
use gitassist_core::template::{render_change_table, ChangeRow};
use gitassist_core::{process, Input, Outcome};

use crate::failopen::{guard, quiet_panics};
use crate::hooks::project_tab;
use crate::log::{self, Entry};
use crate::paths::{exe_name, Paths};
use crate::{env_flag, git, BYPASS_ENV, INTERNAL_ENV};

/// Fetching the current PR body goes over the network through the user's own
/// `gh`, so the gh gateway gets a longer budget than git hooks.
const GH_BUDGET: Duration = Duration::from_secs(20);

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// First `gh` on PATH that is not GitAssist's shim. Resolved on every call so
/// upgrades or moves of the real `gh` are picked up.
pub fn find_real_gh(shim_dir: Option<&Path>) -> Option<PathBuf> {
    let me = std::env::current_exe().ok();
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| {
        if shim_dir.is_some_and(|s| same_file(&dir, s) || dir == s) {
            return None;
        }
        let candidate = dir.join(exe_name("gh"));
        if !candidate.is_file() {
            return None;
        }
        if me.as_deref().is_some_and(|m| same_file(&candidate, m)) {
            return None;
        }
        Some(candidate)
    })
}

#[cfg(unix)]
fn passthrough(real: &Path, args: &[String]) -> i32 {
    use std::os::unix::process::CommandExt;
    let err = Command::new(real).args(args).exec();
    eprintln!("GitAssist: could not run {}: {err}", real.display());
    126
}

#[cfg(not(unix))]
fn passthrough(real: &Path, args: &[String]) -> i32 {
    match Command::new(real).args(args).status() {
        Ok(s) => s.code().unwrap_or(1),
        Err(err) => {
            eprintln!("GitAssist: could not run {}: {err}", real.display());
            126
        }
    }
}

/// Entry point when invoked as `gh`.
pub fn run_gh_shim(args: Vec<String>) -> i32 {
    quiet_panics();
    let paths = Paths::resolve().ok();
    let Some(real) = find_real_gh(paths.as_ref().map(|p| p.bin_dir.as_path())) else {
        eprintln!("GitAssist: the real `gh` was not found on PATH.");
        return 127;
    };
    let Some(paths) = paths else {
        return passthrough(&real, &args);
    };
    let Some(tab) = classify_gh(&args) else {
        return passthrough(&real, &args);
    };
    if env_flag(INTERNAL_ENV) {
        return passthrough(&real, &args);
    }
    if env_flag(BYPASS_ENV) {
        let mut e = Entry::new("gh", "bypass");
        e.tab = tab.as_str().into();
        e.repo = git::toplevel().unwrap_or_default();
        log::append(&paths.log_file, &e);
        return passthrough(&real, &args);
    }

    let (a, p, r) = (args.clone(), paths.clone(), real.clone());
    match guard(GH_BUDGET, move || plan(tab, &a, &p, &r)) {
        Ok(Plan::Passthrough) => passthrough(&real, &args),
        Ok(Plan::Blocked) => 1,
        Ok(Plan::Run {
            args: new_args,
            tmp,
        }) => {
            let code = match Command::new(&real).args(&new_args).status() {
                Ok(s) => s.code().unwrap_or(1),
                Err(err) => {
                    eprintln!("GitAssist: could not run {}: {err}", real.display());
                    126
                }
            };
            if let Some(t) = tmp {
                let _ = fs::remove_file(t);
            }
            code
        }
        Err(err) => {
            let mut e = Entry::new("gh", "error");
            e.tab = tab.as_str().into();
            e.messages = vec![format!("{err:#}")];
            log::append(&paths.log_file, &e);
            passthrough(&real, &args)
        }
    }
}

enum Plan {
    Passthrough,
    Blocked,
    Run {
        args: Vec<String>,
        tmp: Option<PathBuf>,
    },
}

enum BodySource {
    Inline(String),
    File(String),
}

#[derive(Default)]
struct Parsed {
    title: Option<String>,
    body: Option<BodySource>,
    /// Arguments with title/body flags removed.
    rest: Vec<String>,
    base: Option<String>,
    repo: Option<String>,
    selector: Option<String>,
}

fn flag_value(args: &[String], i: &mut usize, long: &str, short: &str) -> Option<String> {
    let a = &args[*i];
    if a == long || (!short.is_empty() && a == short) {
        *i += 1;
        return args.get(*i).cloned();
    }
    if let Some(v) = a.strip_prefix(&format!("{long}=")) {
        return Some(v.to_string());
    }
    if !short.is_empty() && a.starts_with(short) && a.len() > short.len() && !a.starts_with("--") {
        return Some(a[short.len()..].trim_start_matches('=').to_string());
    }
    None
}

/// Flags (besides title/body) that take a value, so their values are not
/// mistaken for the PR selector.
const VALUE_FLAGS: &[&str] = &[
    "-R",
    "--repo",
    "-B",
    "--base",
    "-H",
    "--head",
    "-a",
    "--assignee",
    "--add-assignee",
    "--remove-assignee",
    "-l",
    "--label",
    "--add-label",
    "--remove-label",
    "-p",
    "--project",
    "--add-project",
    "--remove-project",
    "-r",
    "--reviewer",
    "--add-reviewer",
    "--remove-reviewer",
    "-m",
    "--milestone",
    "-T",
    "--template",
    "--recover",
    "--match-head-commit",
    "--author-email",
];

fn parse(tab: Tab, args: &[String]) -> Parsed {
    let title_flags: (&str, &str) = if tab == Tab::Merge {
        ("--subject", "-t")
    } else {
        ("--title", "-t")
    };
    let has_title = matches!(tab, Tab::PrCreate | Tab::PrUpdate | Tab::Issue | Tab::Merge);
    let mut p = Parsed::default();
    let mut positional = 0;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--" {
            p.rest.extend(args[i..].iter().cloned());
            break;
        }
        if has_title {
            if let Some(v) = flag_value(args, &mut i, title_flags.0, title_flags.1) {
                p.title = Some(v);
                i += 1;
                continue;
            }
        }
        if let Some(v) = flag_value(args, &mut i, "--body-file", "-F") {
            p.body = Some(BodySource::File(v));
            i += 1;
            continue;
        }
        if let Some(v) = flag_value(args, &mut i, "--body", "-b") {
            p.body = Some(BodySource::Inline(v));
            i += 1;
            continue;
        }
        p.rest.push(a.clone());
        if a.starts_with('-') {
            let name = a.split('=').next().unwrap_or(a);
            if VALUE_FLAGS.contains(&name) && !a.contains('=') {
                // Some short flags are booleans on other subcommands (`pr review -r`).
                if let Some(v) = args.get(i + 1).filter(|v| !v.starts_with('-')) {
                    if name == "-B" || name == "--base" {
                        p.base = Some(v.clone());
                    }
                    if name == "-R" || name == "--repo" {
                        p.repo = Some(v.clone());
                    }
                    p.rest.push(v.clone());
                    i += 1;
                }
            }
        } else {
            positional += 1;
            if positional == 3 {
                p.selector = Some(a.clone());
            }
        }
        i += 1;
    }
    p
}

fn read_body(src: &BodySource) -> Result<(String, bool)> {
    match src {
        BodySource::Inline(s) => Ok((s.clone(), false)),
        BodySource::File(f) if f == "-" => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            Ok((s, true))
        }
        BodySource::File(f) => Ok((
            fs::read_to_string(f).with_context(|| format!("reading {f}"))?,
            false,
        )),
    }
}

fn current_pr_body(real: &Path, p: &Parsed) -> Option<String> {
    let mut cmd = Command::new(real);
    cmd.args(["pr", "view"]);
    if let Some(sel) = &p.selector {
        cmd.arg(sel);
    }
    if let Some(repo) = &p.repo {
        cmd.args(["--repo", repo]);
    }
    let out = cmd
        .args(["--json", "body", "--jq", ".body"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

fn default_base() -> String {
    git::git_opt(&["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])
        .unwrap_or_else(|| "origin/main".into())
}

/// Built from local git data only: changed files grouped by top-level directory.
fn change_table(base: Option<&str>) -> Option<String> {
    let base = base
        .map(|b| {
            if b.contains('/') {
                b.to_string()
            } else {
                format!("origin/{b}")
            }
        })
        .unwrap_or_else(default_base);
    let out = git::git(&["diff", "--numstat", &format!("{base}...HEAD")]).ok()?;
    let mut groups: BTreeMap<String, ChangeRow> = BTreeMap::new();
    for line in out.lines() {
        let mut f = line.split('\t');
        let (Some(ins), Some(del), Some(path)) = (f.next(), f.next(), f.next()) else {
            continue;
        };
        let area = match path.split_once('/') {
            Some((top, _)) => format!("{top}/"),
            None => "(root)".into(),
        };
        let row = groups.entry(area.clone()).or_insert(ChangeRow {
            area,
            files: 0,
            insertions: 0,
            deletions: 0,
        });
        row.files += 1;
        row.insertions += ins.parse::<usize>().unwrap_or(0);
        row.deletions += del.parse::<usize>().unwrap_or(0);
    }
    if groups.is_empty() {
        return None;
    }
    Some(render_change_table(
        &groups.into_values().collect::<Vec<_>>(),
    ))
}

fn plan(tab: Tab, args: &[String], paths: &Paths, real: &Path) -> Result<Plan> {
    let cfg = Config::load(&paths.config_file)?;
    if !cfg.enabled {
        return Ok(Plan::Passthrough);
    }
    let parsed = parse(tab, args);
    if parsed.title.is_none() && parsed.body.is_none() {
        log_entry(paths, tab, "passthrough", None, &cfg);
        return Ok(Plan::Passthrough);
    }
    let rules = gitassist_core::effective(&cfg, tab, project_tab(&cfg, tab).as_ref());
    let (body, from_stdin) = match &parsed.body {
        Some(src) => {
            let (b, s) = read_body(src)?;
            (Some(b), s)
        }
        None => (None, false),
    };
    let previous = if rules.growth_max.is_some() && body.is_some() {
        current_pr_body(real, &parsed)
    } else {
        None
    };
    let table = if rules.managed_table && body.is_some() {
        change_table(parsed.base.as_deref())
    } else {
        None
    };
    let input = Input {
        title: parsed.title.as_deref(),
        body: body.as_deref(),
        previous_body: previous.as_deref(),
        managed_block: table.as_deref(),
    };
    let outcome = process(&input, &rules);
    for f in &outcome.findings {
        eprintln!("{}", f.message);
    }
    log_entry(paths, tab, outcome.status(), Some(&outcome), &cfg);
    if outcome.blocked() {
        return Ok(Plan::Blocked);
    }
    if !outcome.changed && !from_stdin {
        return Ok(Plan::Passthrough);
    }

    let mut new_args = parsed.rest.clone();
    let mut tmp = None;
    if let Some(t) = &outcome.title {
        let flag = if tab == Tab::Merge {
            "--subject"
        } else {
            "--title"
        };
        insert_before_terminator(&mut new_args, [flag.to_string(), t.clone()]);
    }
    if let Some(b) = &outcome.body {
        fs::create_dir_all(&paths.tmp_dir)?;
        let file = paths
            .tmp_dir
            .join(format!("gh-body-{}-{}.md", std::process::id(), unique()));
        fs::write(&file, b)?;
        insert_before_terminator(
            &mut new_args,
            [
                "--body-file".to_string(),
                file.to_string_lossy().into_owned(),
            ],
        );
        tmp = Some(file);
    }
    Ok(Plan::Run {
        args: new_args,
        tmp,
    })
}

fn unique() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

fn insert_before_terminator(args: &mut Vec<String>, items: [String; 2]) {
    let at = args.iter().position(|a| a == "--").unwrap_or(args.len());
    args.splice(at..at, items);
}

fn log_entry(paths: &Paths, tab: Tab, outcome: &str, o: Option<&Outcome>, cfg: &Config) {
    let mut e = Entry::new("gh", outcome);
    e.tab = tab.as_str().into();
    e.repo = git::toplevel().unwrap_or_default();
    if let Some(o) = o {
        e.rules = o.findings.iter().map(|f| f.rule.to_string()).collect();
        e.messages = o.findings.iter().map(|f| f.message.clone()).collect();
        if cfg.log_bodies {
            e.body = o.body.clone();
        }
    }
    log::append(&paths.log_file, &e);
}
