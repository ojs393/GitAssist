<p align="center">
  <img src=".github/assets/logo.png" width="112" alt="GitAssist logo">
</p>

<h1 align="center">GitAssist</h1>

<p align="center">
  <strong>Pull request and commit management for you and your AI coding agents.</strong><br>
  One desktop app sets the rules for every commit, push, pull request, review and issue on your computer,
  in every repo, for every agent. It works offline and needs no account.
</p>

<p align="center">
  <a href="#install">Install</a> ·
  <a href="#first-time-setup">First-time setup</a> ·
  <a href="#what-you-can-configure">Configure</a> ·
  <a href="#how-it-works">How it works</a> ·
  <a href="#security-and-privacy">Security</a>
</p>

---

## Why GitAssist

AI coding agents write a lot of your Git history. Each one has its own habits. Some add
`Co-authored-by` lines, some add "Generated with" footers, and some write long, wordy pull
request descriptions, commit straight to `main`, or forget your team's conventions.

GitAssist sits between your computer and GitHub and applies **your** rules to all of it:

- **Commits and pushes**: length limits, layout, conventional prefixes, wrapping and banned phrases. It can also make you the author every time.
- **Pull requests**: title and description rules, a limit on how much a description may grow per update, and an auto-updating change table.
- **Comments, reviews and issues**: the same rules, set separately for each.
- **Co-author lines**: removes agent attribution and adds your own lines if you want.
- **Branches**: which branch to start from, protected branches, naming rules, and what to do with new ideas.
- **Ignore lists**: files that never get committed, scoped by personal or organization repo and public or private repo.
- **Agents**: Claude Code, Cursor, GitHub Copilot, OpenAI Codex, Gemini CLI and Aider are set up automatically, including agents you install later.
- **Repos**: one list of every repo on your computer, where each one pushes, and when it was last committed to and pushed.

It's simple by default: run the setup once and you're done. It's also detailed when you need it:
every rule can be tuned per kind of text, with a live preview.

## Install

| Platform | Download | Notes |
|---|---|---|
| macOS 11+ | `GitAssist_<version>_aarch64.dmg` / `_x64.dmg` | Drag GitAssist to Applications and open it. |
| Windows 10+ | `GitAssist_<version>_x64-setup.exe` (or `.msi`) | The `.exe` installs for the current user without admin rights. |
| Linux | `.AppImage` or `.deb` | Needs WebKitGTK 4.1, which most desktops already include. |

Installers are attached to each [release](../../releases). They are built by the CI workflow in
this repo on macOS, Windows and Linux.

**What you need:** `git`. The [GitHub CLI](https://cli.github.com) (`gh`) is optional. With it,
GitAssist also checks pull requests, comments, reviews and issues, and can tell public repos
from private ones.

**Permissions:** none beyond your own user account. GitAssist never asks for administrator
rights, accessibility access or full disk access.

## First-time setup

Open GitAssist and the **setup wizard** starts. It asks one question per step. **Back** and
**Continue** move between steps, and the step list at the top lets you jump to any step.

1. **Welcome**: what GitAssist does.
2. **Account**: your GitHub username, found automatically when the GitHub CLI is signed in.
3. **Commits**: Short, Standard (under 500 characters) or Detailed, plus a tone.
4. **Co-authors**: remove agent co-author lines, and optionally add your own.
5. **Branches**: Solo, Team or Strict, and the branch new work starts from.
6. **Ignore**: secrets in public repos, system junk everywhere, personal agent files in work repos.
7. **Agents**: every coding agent found on your computer, each with its own on/off switch.
8. **Background**: start at login, and let GitAssist fix itself.
9. **Turn on**: one click installs everything and shows the result of each step.

Every step has a sensible default, so pressing **Continue** all the way through gives you a
complete setup. Run it again any time from **Settings → Install**.

## Using the app

The tabs along the top are:

| Tab | What it's for |
|---|---|
| **Overview** | Whether you're protected. Shows commits and pushes, the GitHub CLI, start at login and self-repair, then every agent on one line with its status and an **Apply** or **Remove** button. **Fix all** repairs anything that drifted. |
| **Commits ▾** | Commit messages, docs-only commits, and merge and squash commits. |
| **Pull requests ▾** | New pull requests, pull request updates, comments, code reviews and issues. |
| **Repos ▾** | **Your repos** (every repo on this computer, where it pushes, and when it was last committed to and pushed), **Branches** and **Ignore**. |
| **Activity** | Everything GitAssist did, with filters and search. |
| **Settings** (gear) | Your identity and voice, co-author lines, safety, storage, and install. |

The switch at the top right pauses or resumes all checks.

Each rules page has its own inner tabs:

- **Quick**: one-click Short, Standard or Detailed presets, the most common switches, and a reset to defaults.
- **Length**: title, whole message and body limits, line wrapping, and growth per update (PR updates only).
- **Layout**: conventional prefixes, a template with required sections, and the auto-updating change table (pull requests only).
- **Words**: banned phrases.
- **Co-authors**: remove agent attribution, and add your own lines for this kind of text.
- **Voice**: tone, voice, detail and extra notes for this kind of text.

The **Preview** on the right shows exactly what happens to a sample message, as a before and
after view with the reason for every change. It uses your changes as you make them, and it
can be enlarged, shrunk or hidden. Every setting has an **(i)** next to it with the details.
Changes save on their own, and the top bar shows when they are saved.

Every rule has one of three modes:

- **Fix**: GitAssist corrects it automatically, for example by removing a co-author line or wrapping a long line.
- **Block**: the commit or command stops with a one-line reason. Agents read the reason, fix the text and try again.
- **Warn**: it goes through, with a note about what's wrong.

Out of the box nothing blocks. Limits warn, and only agent co-author lines are fixed.

## What you can configure

### Messages, per kind of text

| Setting | Commits | Docs commits | Merges | New PR | PR update | Comments | Reviews | Issues |
|---|---|---|---|---|---|---|---|---|
| Title or subject length | yes | yes | yes | yes | yes | | | yes |
| Whole message length | yes | yes | yes | yes | yes | yes | yes | yes |
| Body length | yes | yes | yes | yes | yes | yes | yes | yes |
| Wrap long lines | yes | yes | yes | | | | | |
| Conventional prefix (`feat:`, `fix(scope):`) | yes | yes | yes | yes | yes | | | |
| Template with required sections | yes | yes | yes | yes | yes | yes | yes | yes |
| Growth per update | | | | | yes | | | |
| Auto-updating change table | | | | yes | yes | | | |
| Banned phrases | yes | yes | yes | yes | yes | yes | yes | yes |
| Remove agent co-author lines | yes | yes | yes | yes | yes | yes | yes | yes |
| Add your own lines | yes | yes | yes | yes | yes | yes | yes | yes |
| Tone, voice, detail, notes | yes | yes | yes | yes | yes | yes | yes | yes |

A commit counts as a **docs commit** when every changed file matches your docs patterns
(`*.md`, `docs/**` and `README*` by default).

### Branches and ideas

- **Start new work from**: the base branch, `main` by default. Agents are told to branch off it.
- **Protected branches**: direct commits and pushes to `main` or `master` warn by default, or block if you choose. Merges and a repo's first commit are always allowed.
- **Branch names**: optional allowed types, for example `feat/…`, `fix/…`, `docs/…` and `chore/…`.
- **New ideas**: what agents should do with a new idea. The default is to open an issue first (`gh issue create --label idea`), then branch and link the issue in the pull request.

There are three presets: **Solo** (anything goes), **Team** (warn on main, named branches) and
**Strict** (block main, enforce names).

### Ignore lists, scoped per repo

There are five lists, all in `.gitignore` syntax, with `!` to allow a file:

| List | Applies to |
|---|---|
| Every repo | All repos, including local-only ones. It also becomes git's global ignore file if you don't have one. |
| Personal repos | Repos owned by your GitHub account. |
| Organization repos | Repos owned by anyone else. |
| Public repos | Public repos, and repos whose visibility is unknown. Unknown counts as public because that's the safe mistake. |
| Private repos | Private and internal repos. |

GitAssist works out each repo's scope from its remote. It asks your own `gh` once per repo
whether the repo is public, and remembers the answer for a week. On commit, listed files are
left out of the commit and added to the repo's local exclude list. On push, a listed file
anywhere in the pushed history blocks the push. The **Try it** box shows what happens for any
repo and file name.

The defaults keep `.env` files, keys and certificates out of public repos, `.DS_Store` and
editor swap files out everywhere, and personal agent notes out of organization repos.

### Your repos

Every repo GitAssist knows about is listed with:

- its remote (GitHub or any other host, owner and name)
- whether it's personal or organization, and public or private
- the current branch
- when it was last committed to and last pushed

Click a repo to open its **project detail** page. There you can **Configure** named
**publish targets** (for example personal, public, and org): remote URL, when to push,
a publish filter (paths that never go to that remote, via a cleaned mirror), per-target
message and branch rule overlays, and pre-push checks (local shell commands; block or warn).

Agents are told the targets and should prefer:

```text
gitassist push personal
gitassist push public
gitassist push --all
```

Plain `git push <remote>` still works when the remote matches a target. Multi-target
push is all-or-nothing: preflight (rules + checks + filters) runs for every selected
target before any remote is updated.

The list updates itself as you commit and push. **Find repos** adds repos you haven't touched since
installing, by searching your home folder. Desktop, Documents and Downloads are only searched when
you choose them, because macOS asks for permission first. Repos that use Husky or lefthook show a
**Connect** button.

### Settings

- **You**: name, email (your GitHub no-reply address keeps your email private), and "always make me the author".
- **Co-authors**: what happens when an agent adds a co-author line, the patterns that count as attribution, your own lines to add to every commit, and docs patterns.
- **Safety**: self-repair, the rules file in each repo (see Cursor below), whether repos may add stricter rules, and whether Activity keeps message text.
- **Storage**: the size of the activity log, backups and cache, with a **Clear** button for each.
- **Install**: run setup again, repair, or uninstall.

## Agents

| Agent | How it gets your rules | Co-author line switched off |
|---|---|---|
| Claude Code | `~/.claude/CLAUDE.md` | `attribution` in `~/.claude/settings.json` |
| Cursor | `.cursor/rules/gitassist.mdc` in each repo, kept out of commits | Cursor CLI: `~/.cursor/cli-config.json`. Editor: one switch in Cursor Settings, shown in the app |
| GitHub Copilot | `~/.copilot/copilot-instructions.md`, plus VS Code's commit message and pull request description settings | Not added by Copilot |
| OpenAI Codex | `~/.codex/AGENTS.md` (or `AGENTS.override.md` if you use one) | Not added by Codex |
| Gemini CLI | `~/.gemini/GEMINI.md` | Not added by Gemini |
| Aider | `commit-prompt` in `~/.aider.conf.yml` | `attribute-co-authored-by`, `attribute-author` and `attribute-committer` set to false |

GitAssist only ever edits a clearly marked GitAssist section or a single setting, and backs up
each file first. Uninstall puts every file back byte for byte. If you edited a file in the
meantime, uninstall removes only GitAssist's part.

Cursor reads no global rules file, so GitAssist writes the rules into each repo when you clone,
check out or commit. That file is listed in the repo's local exclude list, so it never shows in
`git status` and can never be committed.

Agents you install later are picked up by self-repair within 15 minutes, or straight away when
you open the app. Whatever an agent does, the git hooks clean its local commits.

## How it works

| Part | What it does |
|---|---|
| Global git hooks | `core.hooksPath` points to GitAssist, so every commit, merge, rebase, cherry-pick and push runs through it in every repo. Each repo's own hooks still run afterwards. |
| GitHub CLI gateway | A small `gh` placed first on your PATH checks `gh pr` and `gh issue` text, then hands off to the real `gh`. Everything else passes straight through. |
| Agent files | Your rules, written into each agent's own instruction files. |
| Desktop app | The settings, the live preview, status and Activity. It runs in the menu bar or tray and uses about 25 MB of memory with the window closed. |
| Self-repair | Every 15 minutes, and at login, it puts back anything another tool changed. If a step fails, it tries again from scratch, up to three times. |

The checks run inside git and `gh` themselves, so they work even when the app is closed.
A commit takes about 0.1 seconds longer.

**If GitAssist itself breaks**, for example a damaged settings file or a check that takes more
than 5 seconds, the commit goes through unchanged and the error is written to Activity. Only your
rules ever stop anything.

**What GitAssist can't see:**

- Cloud agents (Copilot coding agent, Cursor cloud agents, Codex cloud) run on other servers. They only follow the rules they're given.
- Tools that call the GitHub API directly instead of `gh`, such as GitHub MCP servers and editor pull request panels, get your rules as instructions only.
- Repos that use Husky or lefthook send git to their own hooks folder. Find them under **Repos → Your repos** (filter **Own hooks**) and press **Connect**.

To skip GitAssist once, set `GITASSIST_BYPASS=1` for that command. It's logged, and agents
are told never to use it.

## Command line

Installing the app also puts `gitassist` on your PATH:

```sh
gitassist on            # set up everything and keep it repaired (same as the wizard's Turn on)
gitassist status        # what's on, what needs fixing, and each agent
gitassist repair        # put back anything that changed
gitassist preview commit < message.txt   # what a message would become
gitassist instructions  # the rules agents are given
gitassist connect ~/code/app             # hook into a repo that uses Husky or lefthook
gitassist uninstall     # remove everything and restore your files
```

Tabs for `preview` are `commit`, `docs_commit`, `merge`, `pr_create`, `pr_update`,
`pr_comment`, `review` and `issue`.

## Settings file

Everything in the app is saved to one readable TOML file. Edit it by hand if you prefer.

| | macOS | Linux | Windows |
|---|---|---|---|
| Settings | `~/Library/Application Support/GitAssist/config.toml` | `~/.config/gitassist/config.toml` | `%APPDATA%\GitAssist\config\config.toml` |
| Hooks, log, backups | `~/Library/Application Support/GitAssist/` | `~/.local/share/gitassist/` | `%APPDATA%\GitAssist\data\` |

For example, to keep every commit under 500 characters, block direct commits to `main` and
require named branches:

```toml
[tabs.commit]
total_max = { value = 500, mode = "block" }

[branches]
protect_mode = "block"
prefixes = ["feat", "fix", "docs", "chore"]
naming_mode = "block"
```

A repo can add stricter rules in a `.gitassist.toml` (same `[tabs.*]` format) if you switch on
**Settings → Safety → Let repos add stricter rules**. A repo can never change your identity or
co-author settings, or switch GitAssist off.

## Activity and storage

Activity lists every check: commits, pushes, `gh` commands, ignore decisions, branch checks and
setup or self-repair, each marked fixed, blocked, warned or passed. It's stored as JSON lines
on your computer only and rotates at 5 MB, keeping two older files. Message text isn't kept
unless you switch it on. Backups of edited files are pruned to the newest 20 sets. Clear the
log, backups or cache any time in **Settings → Storage**.

## Security and privacy

- **No network.** GitAssist contains no HTTP or TLS code. `deny.toml` bans every HTTP client crate, and CI enforces this with `cargo deny`. The only GitHub traffic is from your own `gh`, when you run it or when the visibility check asks it about a repo.
- **CI proves the offline claim.** It runs the hooks and the `gh` gateway with networking disabled and records every connection attempt.
- **A locked-down window.** The settings window has a strict content security policy and no file system, shell or web permissions. It can only call GitAssist's own commands.
- **No surprises from repos.** A cloned repo can't change your rules unless you allow it, and even then it can't touch identity, co-author settings or the on/off switch.
- **Private files.** Settings, Activity and the repo list are readable by your user only (`0600`, in a `0700` folder), and stay on your computer.
- **No permission prompts from the background.** Background checks never look inside Desktop, Documents or Downloads; those folders are only read when you choose them.
- **Reversible.** Every change is backed up, and uninstall restores the original files.
- **Supply chain.** Lock files are committed. CI checks Rust advisories, licences and sources, and runs `npm audit`.

## Troubleshooting

| What you see | What to do |
|---|---|
| "Fix needed" on Overview | Press **Fix all**. Another tool changed git's hooks setting or your shell profile. |
| GitHub CLI shows "except bash (login)" | Your `~/.bash_profile` can't be edited (often it's owned by root). Run `sudo chown $USER ~/.bash_profile`, then press **Fix all**. |
| An agent shows "Out of date" | Press **Apply**. It also updates on its own after you change settings. |
| A repo's commits aren't checked | The repo probably uses Husky or lefthook. Find it under **Repos → Your repos** and press **Connect**. |
| A commit was blocked | The message says which rule and the limit. Change the text, or change the rule's mode to Warn. |

## Build from source

You need Rust (stable) and Node 22. On Linux you also need
`libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev`.

```sh
cd app/ui && npm ci && cd ../src-tauri
../ui/node_modules/.bin/tauri build      # installers in target/release/bundle/
cd ../.. && cargo test --workspace       # Rust tests, including a randomized stress test
cd app/ui && npm test                    # UI helper tests
```

The project is laid out like this:

| Folder | What's in it |
|---|---|
| `crates/core` | The rules engine: config, rules, templates, branches, ignore scopes, agent instructions |
| `crates/gateway` | Git hooks, the `gh` gateway, the activity log, repo detection |
| `crates/setup` | Installers, agent adapters, start at login, health checks, storage |
| `crates/cli` | The `gitassist` command, plus end-to-end tests against real git |
| `app/src-tauri` | The desktop app: window, tray and background monitor |
| `app/ui` | The interface (Svelte), plus a browser harness for checking it (`npm run dev`, then open `/harness.html`) |

## Turning it off

- **Pause**: the switch at the top right stops all checks until you switch it back on. Nothing is uninstalled.
- **Uninstall**: **Settings → Install → Uninstall** (or `gitassist uninstall`) removes the hooks, the `gh` gateway, the login item, every agent's GitAssist section and the per-repo rules files, and restores your files exactly.
- **Turn on again**: run the setup wizard, or `gitassist on`.

## Shortcut on the desktop

- **macOS**: drag GitAssist from Applications to the Desktop while holding Option and Command, or run `ln -s /Applications/GitAssist.app ~/Desktop/GitAssist.app`.
- **Windows**: the installer adds a Start menu entry. Right-click it and choose **Pin to Start** or **More → Open file location** to make a desktop shortcut.
- **Linux**: the `.deb` adds GitAssist to your applications menu.

## License

GitAssist is released under the MIT license. See [LICENSE](LICENSE).

It bundles the [Inter](https://rsms.me/inter/) typeface under the SIL Open Font License 1.1, and it is built
with [Tauri](https://tauri.app) and [Svelte](https://svelte.dev) (both MIT or Apache-2.0). Every Rust
dependency's license is checked in CI against the allow list in `deny.toml`.
