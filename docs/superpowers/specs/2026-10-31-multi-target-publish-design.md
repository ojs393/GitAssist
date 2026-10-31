# Multi-target publish — Design Spec

Date: 2026-10-31
Status: Approved for implementation

## 1. Purpose

Let a single local repository publish to multiple remotes (personal, public,
organization, or custom) with per-target push policy, message/branch rule
overlays, publish filters (cleaned mirrors), and pre-push checks. Agents are
told the configured targets and prefer `gitassist push <label|--all>`.

## 2. Decisions

- **Pipeline:** one publish path for UI, CLI, and `pre-push`.
- **Working tree:** one branch the user edits; optional under-the-hood mirrors
  `refs/gitassist/mirror/<label>/<branch>` when a target has a publish filter.
- **Config storage:** GitAssist app data only (`targets/<key>.toml`), never in
  the git tree. Agent rules get a generated summary without private URLs.
- **Multi-push:** all-or-nothing preflight; mid-flight failure stops and reports
  drift.
- **Plain `git push <remote>`:** still allowed; that target’s rules apply.
- **Unconfigured repos:** unchanged.
- **Global ignore:** still commit-time. `publish_filter` is push-time only.

## 3. Per-target fields

1. Identity — label, role, remote name, URL  
2. Push policy — manual | after_commit | with_all | agent_only; include_in_push_all  
3. Publish filter — gitignore paths never sent to this target  
4. Message rule overlays — sparse TabConfig map  
5. Branch rule overlays — optional BranchConfig  
6. Checks — enabled, on_fail (block|warn), timeout_secs, named shell commands  

## 4. Phases

1. Store + detail UI + configure modal  
2. Pipeline + CLI + hooks + agent Targets section (same-tree push)  
3. Filters + mirrors  
4. Check runner (local commands; GitHub Actions via user-supplied `gh` recipes)

## 5. Non-goals

- Committed target config in the repo  
- Identical SHAs across filtered remotes  
- In-app CI dashboard (commands only)  
- Changing global personal/org/public/private ignore lists  

## 6. Agent contract

Prefer:

```text
gitassist push personal
gitassist push public
gitassist push --all
```

Plain `git push <remote>` remains valid when the remote matches a target.
