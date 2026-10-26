use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use gitassist_core::config::{Config, TabConfig, Tabs};
use gitassist_core::instructions::{is_repo_rule, render_repo_rule, REPO_RULE_PATH};
use gitassist_core::message::{join_git_comments, split_git_comments, subject_and_body};
use gitassist_core::scope::{patterns_for, scope_label, Matcher};
use gitassist_core::tab::{classify_commit, Tab};
use gitassist_core::trailers::strip_lines;
use gitassist_core::{effective, process, Input, Mode, Outcome};
use serde::Deserialize;

use crate::failopen::{guard, quiet_panics, BUDGET};
use crate::log::{self, Entry};
use crate::paths::Paths;
use crate::{chain, env_flag, git, repo, BYPASS_ENV, INTERNAL_ENV};

const SOURCE_FILE: &str = "gitassist-commit-source";

/// Entry point for `gitassist hook <name> [args...]`. Returns the exit code
/// git should see. GitAssist's own step fails open; chained hooks still run.
pub fn run_hook(name: &str, args: Vec<String>, no_chain: bool) -> i32 {
    quiet_panics();
    let Ok(paths) = Paths::resolve() else {
        return 0;
    };
    let stdin_data = (name == "pre-push").then(|| {
        let mut buf = Vec::new();
        let _ = std::io::stdin().read_to_end(&mut buf);
        buf
    });

    let own = if env_flag(INTERNAL_ENV) {
        0
    } else if env_flag(BYPASS_ENV) {
        let mut e = Entry::new(format!("hook:{name}"), "bypass");
        e.repo = git::toplevel().unwrap_or_default();
        log::append(&paths.log_file, &e);
        0
    } else {
        let (n, a, p, s) = (
            name.to_string(),
            args.clone(),
            paths.clone(),
            stdin_data.clone(),
        );
        match guard(BUDGET, move || handle(&n, &a, s.as_deref(), &p)) {
            Ok(code) => code,
            Err(err) => {
                let mut e = Entry::new(format!("hook:{name}"), "error");
                e.messages = vec![format!("{err:#}")];
                e.repo = git::toplevel().unwrap_or_default();
                log::append(&paths.log_file, &e);
                0
            }
        }
    };
    if own != 0 || no_chain {
        return own;
    }
    chain::run_chain(name, &args, stdin_data.as_deref(), &paths)
}

fn handle(name: &str, args: &[String], stdin: Option<&[u8]>, paths: &Paths) -> Result<i32> {
    let cfg = Config::load(&paths.config_file)?;
    if !cfg.enabled {
        return Ok(0);
    }
    match name {
        "pre-commit" => pre_commit(&cfg, paths),
        "post-checkout" => post_checkout(&cfg, paths),
        "prepare-commit-msg" => prepare_commit_msg(args, &cfg, paths),
        "commit-msg" => commit_msg(args, &cfg, paths),
        "post-commit" => post_commit(&cfg, paths),
        "pre-push" => pre_push(args, stdin.unwrap_or_default(), &cfg, paths),
        _ => Ok(0),
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ProjectFile {
    tabs: Tabs,
}

pub fn project_tab(cfg: &Config, tab: Tab) -> Option<TabConfig> {
    if !cfg.allow_project_overrides {
        return None;
    }
    let root = git::toplevel()?;
    let raw = fs::read_to_string(Path::new(&root).join(".gitassist.toml")).ok()?;
    let file: ProjectFile = toml::from_str(&raw).ok()?;
    Some(file.tabs.get(tab).clone())
}

fn rules_for(cfg: &Config, tab: Tab) -> gitassist_core::EffectiveRules {
    effective(cfg, tab, project_tab(cfg, tab).as_ref())
}

fn report(outcome: &Outcome) {
    for f in &outcome.findings {
        eprintln!("{}", f.message);
    }
}

fn entry_for(source: &str, tab: Tab, outcome: &Outcome, cfg: &Config) -> Entry {
    let mut e = Entry::new(source, outcome.status());
    e.tab = tab.as_str().into();
    e.rules = outcome
        .findings
        .iter()
        .map(|f| f.rule.to_string())
        .collect();
    e.messages = outcome.findings.iter().map(|f| f.message.clone()).collect();
    e.repo = git::toplevel().unwrap_or_default();
    if cfg.log_bodies {
        e.body = outcome.body.clone();
    }
    e
}

fn prepare_commit_msg(args: &[String], cfg: &Config, paths: &Paths) -> Result<i32> {
    let file = args
        .first()
        .context("prepare-commit-msg: missing message file")?;
    let source = args.get(1).map(String::as_str);
    if let Some(dir) = git::git_dir() {
        let _ = fs::write(dir.join(SOURCE_FILE), source.unwrap_or(""));
    }
    let tab = classify_commit(source, &git::staged_files(), &cfg.global.docs.patterns);
    let rules = rules_for(cfg, tab);
    // Only strip here: this still runs under `--no-verify`, which skips commit-msg.
    if !rules.enabled || !rules.strip_trailers || rules.strip_mode != Mode::Fix {
        return Ok(0);
    }
    let raw = fs::read_to_string(file)?;
    let msg = split_git_comments(&raw, git::comment_char());
    let (stripped, n) = strip_lines(&msg.content, &rules.strip_patterns);
    if n > 0 {
        fs::write(file, join_git_comments(&stripped, &msg.comments))?;
        let mut e = Entry::new("hook:prepare-commit-msg", "fixed");
        e.tab = tab.as_str().into();
        e.rules = vec!["strip_trailers".into()];
        e.messages = vec![format!(
            "GitAssist [{}] removed {n} agent co-author line{}.",
            tab.as_str(),
            if n == 1 { "" } else { "s" }
        )];
        e.repo = git::toplevel().unwrap_or_default();
        log::append(&paths.log_file, &e);
    }
    Ok(0)
}

fn commit_source() -> Option<String> {
    let dir = git::git_dir()?;
    let saved = fs::read_to_string(dir.join(SOURCE_FILE)).ok();
    let _ = fs::remove_file(dir.join(SOURCE_FILE));
    match saved.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => Some(s.to_string()),
        _ => match git::in_progress_op() {
            Some("merge") => Some("merge".into()),
            Some("squash") => Some("squash".into()),
            _ => None,
        },
    }
}

fn commit_msg(args: &[String], cfg: &Config, paths: &Paths) -> Result<i32> {
    let file = args.first().context("commit-msg: missing message file")?;
    let raw = fs::read_to_string(file)?;
    let msg = split_git_comments(&raw, git::comment_char());
    if msg.content.trim().is_empty() {
        return Ok(0);
    }
    let source = commit_source();
    let tab = classify_commit(
        source.as_deref(),
        &git::staged_files(),
        &cfg.global.docs.patterns,
    );
    let rules = rules_for(cfg, tab);
    let outcome = process(
        &Input {
            body: Some(&msg.content),
            ..Default::default()
        },
        &rules,
    );
    if outcome.changed && !outcome.blocked() {
        if let Some(body) = &outcome.body {
            fs::write(file, join_git_comments(body, &msg.comments))?;
        }
    }
    report(&outcome);
    log::append(
        &paths.log_file,
        &entry_for("hook:commit-msg", tab, &outcome, cfg),
    );
    Ok(if outcome.blocked() { 1 } else { 0 })
}

fn post_commit(cfg: &Config, paths: &Paths) -> Result<i32> {
    crate::registry::touch(paths, |r| r.last_commit = Some(crate::registry::now()));
    let id = &cfg.global.identity;
    if !id.enforce_author || !id.is_set() || git::in_progress_op().is_some() {
        return Ok(0);
    }
    let current = git::git(&["log", "-1", "--format=%an%x00%ae"])?;
    let (name, email) = current.trim().split_once('\0').unwrap_or(("", ""));
    if name == id.author_name.trim() && email == id.author_email.trim() {
        return Ok(0);
    }
    let status = Command::new("git")
        .args([
            "commit",
            "--amend",
            "--no-edit",
            "--no-verify",
            "--allow-empty",
            "--reset-author",
            "--quiet",
        ])
        .env("GIT_AUTHOR_NAME", id.author_name.trim())
        .env("GIT_AUTHOR_EMAIL", id.author_email.trim())
        .env(INTERNAL_ENV, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .status()?;
    let mut e = Entry::new(
        "hook:post-commit",
        if status.success() { "fixed" } else { "error" },
    );
    e.tab = Tab::Commit.as_str().into();
    e.rules = vec!["author".into()];
    e.messages = vec![format!(
        "GitAssist [commit] author {name} <{email}> replaced with your identity."
    )];
    e.repo = git::toplevel().unwrap_or_default();
    log::append(&paths.log_file, &e);
    Ok(0)
}

struct PushedCommit {
    sha: String,
    merge: bool,
    message: String,
    /// Files added or modified (not deleted) by the commit.
    files: Vec<String>,
}

fn parse_name_status(block: &str) -> Vec<String> {
    block
        .lines()
        .filter_map(|l| {
            let mut cols = l.split('\t');
            let status = cols.next()?.trim();
            if status.is_empty() || status.starts_with('D') {
                return None;
            }
            cols.next_back()
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
        })
        .collect()
}

fn commits_in_range(local: &str, remote_sha: &str, remote: &str) -> Result<Vec<PushedCommit>> {
    let zero = |s: &str| s.chars().all(|c| c == '0');
    let format = "--format=%x1e%H%x1f%P%x1f%B%x1f";
    let not_remote = format!("--remotes={remote}");
    let range = format!("{remote_sha}..{local}");
    let mut args = vec![
        "log",
        "--name-status",
        "--no-renames",
        "--max-count=200",
        format,
    ];
    if zero(remote_sha) {
        args.extend([local, "--not", not_remote.as_str()]);
    } else {
        args.push(range.as_str());
    }
    let out = git::git(&args)?;
    Ok(out
        .split('\x1e')
        .filter(|r| !r.trim().is_empty())
        .filter_map(|rec| {
            let mut parts = rec.splitn(4, '\x1f');
            let sha = parts.next()?.trim().to_string();
            let parents = parts.next()?.split_whitespace().count();
            let message = parts.next()?.trim().to_string();
            let files = parse_name_status(parts.next().unwrap_or(""));
            Some(PushedCommit {
                sha,
                merge: parents > 1,
                message,
                files,
            })
        })
        .collect())
}

fn pre_push(args: &[String], stdin: &[u8], cfg: &Config, paths: &Paths) -> Result<i32> {
    let remote = args.first().map(String::as_str).unwrap_or("origin");
    // The configured URL names the GitHub repo even when a separate pushurl is used.
    let url = git::git_opt(&["config", "--get", &format!("remote.{remote}.url")])
        .or_else(|| args.get(1).cloned());
    let scope = repo::scope_for_url(cfg, paths, url.as_deref());
    let ignore_matcher = cfg
        .ignore
        .enabled
        .then(|| Matcher::new(&patterns_for(&cfg.ignore, &scope)));
    let input = String::from_utf8_lossy(stdin);
    let pushing_to = remote.to_string();
    crate::registry::touch(paths, |r| {
        r.last_push = Some(crate::registry::now());
        r.last_push_remote = Some(pushing_to);
    });
    let mut blocked: Vec<String> = Vec::new();
    let mut warned: Vec<String> = Vec::new();
    for line in input.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        let [_, local_sha, remote_ref, remote_sha] = f.as_slice() else {
            continue;
        };
        if local_sha.chars().all(|c| c == '0') {
            continue;
        }
        let new_branch = remote_sha.chars().all(|c| c == '0');
        for bf in gitassist_core::branch::check_push(&cfg.branches, remote_ref, new_branch) {
            if bf.mode == Mode::Block {
                blocked.push(format!("  {remote_ref}\n    {}", bf.message));
            } else {
                warned.push(format!("  {remote_ref} ({})", bf.message));
            }
        }
        for c in commits_in_range(local_sha, remote_sha, remote)? {
            let source = c.merge.then_some("merge");
            let tab = classify_commit(source, &c.files, &cfg.global.docs.patterns);
            let outcome = process(
                &Input {
                    body: Some(&c.message),
                    ..Default::default()
                },
                &rules_for(cfg, tab),
            );
            let short = &c.sha[..c.sha.len().min(10)];
            let subject = subject_and_body(&c.message).0;
            let mut reasons: Vec<String> = outcome
                .findings
                .iter()
                .filter(|f| f.mode == Mode::Block)
                .map(|f| f.message.clone())
                .collect();
            if let Some(m) = &ignore_matcher {
                let hits: Vec<&String> = m.filter(&c.files);
                if !hits.is_empty() {
                    let list = hits
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let msg = format!(
                        "GitAssist [ignore] commits {list}, which your {} rules keep out. Remove it from the commit and push again.",
                        scope_label(&scope)
                    );
                    if cfg.ignore.mode == Mode::Warn {
                        warned.push(format!("  {short} {subject} ({msg})"));
                    } else {
                        reasons.push(msg);
                    }
                }
            }
            if !reasons.is_empty() {
                blocked.push(format!(
                    "  {short} {subject}\n    {}",
                    reasons.join("\n    ")
                ));
            } else if outcome.changed
                && outcome
                    .findings
                    .iter()
                    .any(|f| f.mode == Mode::Fix && f.rule == "strip_trailers")
            {
                warned.push(format!(
                    "  {short} {subject} (still contains co-author/attribution lines)"
                ));
            }
        }
    }
    if !warned.is_empty() {
        eprintln!("GitAssist: check these commits before they reach GitHub:");
        eprintln!("{}", warned.join("\n"));
    }
    let status = if !blocked.is_empty() {
        "blocked"
    } else if !warned.is_empty() {
        "warned"
    } else {
        "clean"
    };
    let mut e = Entry::new("hook:pre-push", status);
    e.repo = git::toplevel().unwrap_or_default();
    e.messages = blocked
        .iter()
        .chain(warned.iter())
        .map(|m| m.trim().to_string())
        .collect();
    log::append(&paths.log_file, &e);
    if blocked.is_empty() {
        return Ok(0);
    }
    eprintln!("GitAssist: push blocked. These commits break your rules:");
    eprintln!("{}", blocked.join("\n"));
    eprintln!("  Fix them with `git rebase -i` (or `git commit --amend` for the last one) and push again.");
    Ok(1)
}

const EXCLUDE_BEGIN: &str = "# >>> gitassist ignore >>>";
const EXCLUDE_END: &str = "# <<< gitassist ignore <<<";

/// Keeps the repo's local exclude list in sync with the lists for its scope,
/// so `git add .` and `git status` skip those files from now on.
fn sync_exclude(patterns: &[String], cfg: &Config) {
    let Some(dir) = git::common_dir() else { return };
    let mut patterns = patterns.to_vec();
    if cfg.repo_rule_files {
        patterns.push(format!("/{REPO_RULE_PATH}"));
    }
    let file = dir.join("info").join("exclude");
    let text = fs::read_to_string(&file).unwrap_or_default();
    let block = format!("{EXCLUDE_BEGIN}\n{}\n{EXCLUDE_END}\n", patterns.join("\n"));
    let new = match (text.find(EXCLUDE_BEGIN), text.find(EXCLUDE_END)) {
        (Some(s), Some(e)) if e > s => {
            let mut end = e + EXCLUDE_END.len();
            if text[end..].starts_with('\n') {
                end += 1;
            }
            format!("{}{}{}", &text[..s], block, &text[end..])
        }
        _ if text.is_empty() || text.ends_with('\n') => format!("{text}{block}"),
        _ => format!("{text}\n{block}"),
    };
    if new != text {
        let _ = fs::create_dir_all(dir.join("info"));
        let _ = fs::write(&file, new);
    }
}

/// Writes (or removes) the per-repo rule file so agents that only read
/// project rules, like Cursor, see the same rules as everyone else.
fn sync_repo_rule(cfg: &Config) {
    let Some(root) = git::toplevel() else { return };
    let file = Path::new(&root).join(REPO_RULE_PATH);
    let existing = fs::read_to_string(&file).ok();
    if !cfg.repo_rule_files {
        if existing.as_deref().is_some_and(is_repo_rule) {
            let _ = fs::remove_file(&file);
        }
        return;
    }
    if existing.as_deref().is_some_and(|t| !is_repo_rule(t)) {
        return;
    }
    let want = render_repo_rule(cfg);
    if existing.as_deref() != Some(want.as_str()) {
        if let Some(d) = file.parent() {
            let _ = fs::create_dir_all(d);
        }
        let _ = fs::write(&file, want);
    }
}

/// Runs after clone and checkout: the repo is ready for agents before the first commit.
fn post_checkout(cfg: &Config, paths: &Paths) -> Result<i32> {
    crate::registry::touch(paths, |_| {});
    let scope = if cfg.ignore.enabled {
        patterns_for(&cfg.ignore, &repo::current_scope(cfg, paths))
    } else {
        Vec::new()
    };
    sync_exclude(&scope, cfg);
    sync_repo_rule(cfg);
    Ok(0)
}

/// Direct commits to a protected branch. Merges and the first commit are fine.
fn branch_guard(cfg: &Config, paths: &Paths) -> Result<Option<i32>> {
    let Some(branch) = git::current_branch() else {
        return Ok(None);
    };
    if git::in_progress_op().is_some() {
        return Ok(None);
    }
    let Some(f) = gitassist_core::branch::check_commit(&cfg.branches, &branch) else {
        return Ok(None);
    };
    if git::is_unborn() {
        return Ok(None);
    }
    eprintln!("{}", f.message);
    let blocked = f.mode == Mode::Block;
    let mut e = Entry::new(
        "hook:pre-commit",
        if blocked { "blocked" } else { "warned" },
    );
    e.tab = "branch".into();
    e.rules = vec!["branch".into()];
    e.messages = vec![f.message];
    e.repo = git::toplevel().unwrap_or_default();
    log::append(&paths.log_file, &e);
    Ok(blocked.then_some(1))
}

fn pre_commit(cfg: &Config, paths: &Paths) -> Result<i32> {
    sync_repo_rule(cfg);
    if let Some(code) = branch_guard(cfg, paths)? {
        return Ok(code);
    }
    if !cfg.ignore.enabled {
        sync_exclude(&[], cfg);
        return Ok(0);
    }
    let scope = repo::current_scope(cfg, paths);
    let patterns = patterns_for(&cfg.ignore, &scope);
    sync_exclude(&patterns, cfg);
    let staged: Vec<String> = git::git(&[
        "diff",
        "--cached",
        "--name-only",
        "--diff-filter=ACMR",
        "-z",
    ])
    .map(|s| {
        s.split('\0')
            .filter(|p| !p.is_empty())
            .map(String::from)
            .collect()
    })
    .unwrap_or_default();
    let matcher = Matcher::new(&patterns);
    let hits: Vec<String> = matcher.filter(&staged).into_iter().cloned().collect();
    if hits.is_empty() {
        return Ok(0);
    }
    let list = hits.join(", ");
    let label = scope_label(&scope);
    let (outcome, msg, code) = match cfg.ignore.mode {
        Mode::Fix => {
            let mut args: Vec<&str> = if git::git_opt(&["rev-parse", "--verify", "-q", "HEAD"]).is_some() {
                vec!["reset", "-q", "--"]
            } else {
                vec!["rm", "--cached", "-q", "--"]
            };
            args.extend(hits.iter().map(String::as_str));
            git::git(&args)?;
            ("fixed", format!("GitAssist [ignore] left {list} out of this commit ({label} rules)."), 0)
        }
        Mode::Block => (
            "blocked",
            format!("GitAssist [ignore] {list} must not be committed to this {label}. Unstage it and retry."),
            1,
        ),
        Mode::Warn => ("warned", format!("GitAssist [ignore] {list} is on your ignore list for this {label}."), 0),
    };
    eprintln!("{msg}");
    let mut e = Entry::new("hook:pre-commit", outcome);
    e.tab = "ignore".into();
    e.rules = vec!["ignore".into()];
    e.messages = vec![msg];
    e.repo = git::toplevel().unwrap_or_default();
    log::append(&paths.log_file, &e);
    Ok(code)
}
