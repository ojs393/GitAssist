use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use gitassist_core::instructions::{render_agent_instructions, render_repo_rule_with_extra};
use gitassist_core::{effective, process, Config, Input, Tab};
use gitassist_gateway::publish::{self, Selection};
use gitassist_gateway::targets;
use gitassist_gateway::{gh, hooks, Paths};
use gitassist_setup::ops::{self, Step};
use gitassist_setup::{health, repos, Env};

const USAGE: &str = "GitAssist — local gateway for commits, pushes, PRs and issues

Usage:
  gitassist on                                 Turn on: set up everything, keep it repaired
  gitassist install [hooks|shim|agents|autostart|all]   Install one part
  gitassist uninstall                          Undo everything GitAssist changed
  gitassist status [--json]                    Check that everything is wired up
  gitassist repair                             Reinstall anything that drifted
  gitassist preview <tab> [--title T] < msg    Show what a message becomes
  gitassist instructions [--repo PATH]         Print the rules given to agents
  gitassist connect <repo>                     Hook into a repo with its own hooks path
  gitassist push [<label>|--all] [--repo PATH] Publish to a configured target
  gitassist targets [--repo PATH]              List publish targets for a repo
  gitassist hook <name> [args...]              (called by git)

Tabs: commit, docs_commit, merge, pr_create, pr_update, pr_comment, review, issue";

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().collect();
    let argv0 = args.remove(0);
    let stem = Path::new(&argv0)
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if stem == "gh" {
        return code(gh::run_gh_shim(args));
    }
    if args.first().map(String::as_str) == Some("hook") {
        let name = args.get(1).cloned().unwrap_or_default();
        let mut rest: Vec<String> = args.into_iter().skip(2).collect();
        let no_chain = rest.first().map(String::as_str) == Some("--no-chain");
        if no_chain {
            rest.remove(0);
        }
        return code(hooks::run_hook(&name, rest, no_chain));
    }
    match run(args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("gitassist: {e:#}");
            ExitCode::from(1)
        }
    }
}

fn code(c: i32) -> ExitCode {
    ExitCode::from(u8::try_from(c).unwrap_or(1))
}

fn self_exe() -> Result<PathBuf> {
    std::env::current_exe().context("locating the gitassist binary")
}

/// Inside an installed bundle the desktop app sits next to this binary.
fn bundled_app() -> Option<PathBuf> {
    let app = self_exe()
        .ok()?
        .parent()?
        .join(gitassist_gateway::paths::exe_name("gitassist-app"));
    app.is_file().then_some(app)
}

fn run(args: Vec<String>) -> Result<ExitCode> {
    let paths = Paths::resolve()?;
    let env = Env::detect()?;
    let sub = args.first().map(String::as_str).unwrap_or("help");
    match sub {
        "install" => {
            let what = args.get(1).map(String::as_str).unwrap_or("all");
            let app = bundled_app();
            let steps: Vec<Step> = match what {
                "hooks" => vec![Step::Hooks],
                "shim" => vec![Step::Shim],
                "agents" => vec![Step::Agents],
                "autostart" => vec![Step::Autostart],
                "all" if app.is_some() => {
                    vec![Step::Hooks, Step::Shim, Step::Agents, Step::Autostart]
                }
                "all" => vec![Step::Hooks, Step::Shim, Step::Agents],
                other => bail!("unknown install step `{other}`"),
            };
            for s in steps {
                println!(
                    "{}",
                    ops::install(s, &paths, &env, &self_exe()?, app.as_deref())?
                );
            }
        }
        "on" | "turn-on" => {
            let mut all_ok = true;
            for r in ops::turn_on(&paths, &env, &self_exe()?, bundled_app().as_deref()) {
                all_ok &= r.ok;
                println!("{} {}", if r.ok { "[ok]" } else { "[!!]" }, r.message);
            }
            return Ok(if all_ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            });
        }
        "heal" => {
            let done = ops::heal(&paths, &env, &self_exe()?, bundled_app().as_deref());
            println!(
                "{}",
                if done.is_empty() {
                    "Nothing to repair.".into()
                } else {
                    format!("Repaired: {}", done.join(", "))
                }
            );
        }
        "uninstall" => {
            ops::uninstall_all(&paths, &env)?;
            println!("GitAssist removed. Your previous git and agent settings were restored.");
        }
        "repair" => {
            let done = ops::repair(&paths, &env, &self_exe()?, bundled_app().as_deref())?;
            println!(
                "Repaired: {}",
                if done.is_empty() {
                    "nothing needed".into()
                } else {
                    done.join(", ")
                }
            );
        }
        "status" => {
            let report = health::check(&paths, &env);
            if args.iter().any(|a| a == "--json") {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                for i in &report.items {
                    println!(
                        "{} {:<20} {}",
                        if i.ok { "[ok]" } else { "[!!]" },
                        i.label,
                        i.summary
                    );
                }
                for a in &report.adapters {
                    println!("  {:<16} {:?}", a.name, a.status);
                }
                for r in &report.repos {
                    println!(
                        "  repo {} {}",
                        r.path,
                        if r.connected {
                            "connected"
                        } else {
                            "needs Connect"
                        }
                    );
                }
            }
            return Ok(if report.ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            });
        }
        "preview" => {
            let tab = args
                .get(1)
                .and_then(|t| Tab::parse(t))
                .context("preview needs a tab name")?;
            let title = args
                .iter()
                .position(|a| a == "--title")
                .and_then(|i| args.get(i + 1))
                .cloned();
            let mut body = String::new();
            std::io::stdin().read_to_string(&mut body)?;
            let cfg = Config::load(&paths.config_file)?;
            let input = Input {
                title: title.as_deref(),
                body: Some(&body),
                ..Default::default()
            };
            let out = process(&input, &effective(&cfg, tab, None));
            println!("{}", serde_json::to_string_pretty(&out)?);
            return Ok(if out.blocked() {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            });
        }
        "instructions" => {
            let cfg = Config::load(&paths.config_file)?;
            let repo = opt_repo(&args)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            let data = targets::load(&paths, &repo);
            let extra = targets::render_targets_section(&data);
            if args.iter().any(|a| a == "--repo") {
                println!("{}", render_repo_rule_with_extra(&cfg, &extra));
            } else {
                let mut body = render_agent_instructions(&cfg);
                if !extra.is_empty() {
                    body.push_str("\n\n");
                    body.push_str(extra.trim());
                }
                println!("{body}");
            }
        }
        "targets" => {
            let repo = opt_repo(&args)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            let data = targets::load(&paths, &repo);
            if data.targets.is_empty() {
                println!("No publish targets configured for {}.", repo.display());
            } else {
                for t in &data.targets {
                    println!(
                        "{}  remote={}  enabled={}  push_all={}  mirror={}  checks={}",
                        t.label,
                        t.remote_name,
                        t.enabled,
                        t.include_in_push_all,
                        t.needs_mirror(),
                        t.checks.enabled
                    );
                }
            }
        }
        "push" => {
            let repo = opt_repo(&args).unwrap_or_else(|| {
                std::env::current_dir()
                    .context("cwd")
                    .unwrap_or_else(|_| PathBuf::from("."))
            });
            let cfg = Config::load(&paths.config_file)?;
            let report = if args.iter().any(|a| a == "--all") {
                publish::publish(&paths, &repo, &cfg, Selection::All)?
            } else {
                let skip = opt_repo_flag_value(&args).unwrap_or("");
                let label = args
                    .iter()
                    .skip(1)
                    .filter(|a| !a.starts_with('-') && a.as_str() != skip)
                    .next()
                    .cloned()
                    .or_else(|| targets::load(&paths, &repo).default_target)
                    .context("push needs a target label or --all")?;
                publish::publish(&paths, &repo, &cfg, Selection::Label(&label))?
            };
            for r in &report.results {
                println!(
                    "{} {} — {}",
                    if r.ok { "[ok]" } else { "[!!]" },
                    r.label,
                    r.message
                );
            }
            if !report.drift.is_empty() {
                println!(
                    "drift: these remotes already received a push: {}",
                    report.drift.join(", ")
                );
            }
            return Ok(if report.ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            });
        }
        "connect" => {
            let repo = args.get(1).context("connect needs a repository path")?;
            repos::connect(Path::new(repo), &paths)?;
            println!("Connected {repo}.");
        }
        "version" | "--version" => println!("gitassist {}", env!("CARGO_PKG_VERSION")),
        _ => println!("{USAGE}"),
    }
    Ok(ExitCode::SUCCESS)
}

fn opt_repo(args: &[String]) -> Option<PathBuf> {
    args.iter()
        .position(|a| a == "--repo")
        .and_then(|i| args.get(i + 1))
        .map(|p| {
            let p = p.trim();
            if let Some(rest) = p.strip_prefix("~/") {
                directories::BaseDirs::new()
                    .map(|b| b.home_dir().join(rest))
                    .unwrap_or_else(|| PathBuf::from(p))
            } else {
                PathBuf::from(p)
            }
        })
}

fn opt_repo_flag_value(args: &[String]) -> Option<&str> {
    args.iter()
        .position(|a| a == "--repo")
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}
