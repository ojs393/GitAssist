// Browser harness backend for UI checks (open /harness.html with `npm run dev`).
// Mirrors the shapes the Rust commands return; one agent fails on purpose so
// failure states can be seen.
const warn = (n) => ({ value: n, mode: 'warn' });
const base = { enabled: true, conventional: { value: 'off', mode: 'warn' }, template: '', sections_mode: 'warn', managed_table: false, banned_phrases: { value: [], mode: 'warn' }, strip_trailers: true };
const TABS = [
  ['commit', 'Commit messages', true, true, { subject_max: warn(72), body_max: warn(5000), wrap: { value: 72, mode: 'fix' } }],
  ['docs_commit', 'Docs commits', true, true, { subject_max: warn(72), body_max: warn(5000), wrap: { value: 72, mode: 'fix' } }],
  ['merge', 'Merge & squash commits', true, true, { subject_max: warn(100), body_max: warn(5000), wrap: { value: 72, mode: 'fix' } }],
  ['pr_create', 'New pull requests', false, true, { subject_max: warn(72), body_max: warn(10000) }],
  ['pr_update', 'Pull request updates', false, true, { subject_max: warn(72), body_max: warn(10000), growth_max: warn(3000) }],
  ['pr_comment', 'PR comments', false, false, { body_max: warn(5000) }],
  ['review', 'Code reviews', false, false, { body_max: warn(5000) }],
  ['issue', 'Issues', false, true, { subject_max: warn(100), body_max: warn(10000) }],
].map(([id, label, commit_message, has_title, extra]) => ({ id, label, commit_message, has_title, defaults: { ...base, ...extra } }));

let config = {
  version: 1, enabled: true, allow_project_overrides: false, log_bodies: false, watched_repos: [], auto_repair: true, repo_rule_files: true,
  branches: { enabled: true, base: 'main', protected: ['main', 'master'], protect_mode: 'warn', prefixes: [], naming_mode: 'warn', ideas: 'Open an issue for the idea first, then branch off main.' },
  global: {
    identity: { author_name: '', author_email: '', enforce_author: false },
    trailers: { strip_patterns: ['^\\s*Co-authored-by:.*$', '^.*Generated with \\[?Claude Code\\]?.*$', '^\\s*\\x{1F916}.*$'], strip_mode: 'fix', add: [] },
    style: { tone: 'terse', voice: 'imperative', detail: 'medium', notes: 'Write like I talk.' },
    docs: { patterns: ['*.md', 'docs/**', 'README*'] },
  },
  tabs: Object.fromEntries(TABS.map((t) => [t.id, {}])),
  ignore: { enabled: true, mode: 'fix', github_user: 'octo', lookup_visibility: true, every_repo: ['.DS_Store', '*.swp'], personal: [], organization: ['CLAUDE.local.md'], public: ['.env', '.env.*', '!.env.example', '*.pem'], private: [] },
};

let turnedOn = new URLSearchParams(location.search).get('off') === null;
const adapters = [
  ['claude', 'Claude Code', 'current'], ['cursor', 'Cursor', 'current'], ['copilot', 'GitHub Copilot', 'not_applied'],
  ['codex', 'OpenAI Codex', 'stale'], ['gemini', 'Gemini CLI', 'current'], ['aider', 'Aider', 'not_detected'],
].map(([id, name, status]) => ({ id, name, status, files: [`/home/user/.${id}/settings.json`], limits: ['Cloud runs are remote; local hooks cannot see them.'], manual: id === 'cursor' ? ['Cursor Settings → Git & Pull Requests: turn off Commit Attribution.', 'Cursor Settings → Rules: paste the GitAssist rules.'] : [] }));

const REPOS = [
  { path: '/home/user/code/app', name: 'app', host: 'github.com', slug: 'octo/app', owner: 'personal', visibility: 'public', connected: false, remotes: [{ name: 'origin', url: 'git@github.com:octo/app.git' }], branch: 'feat/retries', last_commit: Math.floor(Date.now() / 1000) - 300, last_push: Math.floor(Date.now() / 1000) - 3600, last_push_remote: 'origin', local_hooks_path: null },
  { path: '/home/user/work/platform-with-a-really-long-repository-name', name: 'platform-with-a-really-long-repository-name', host: 'github.com', slug: 'acme/platform-with-a-really-long-repository-name', owner: 'organization', visibility: 'private', connected: false, remotes: [{ name: 'origin', url: 'https://github.com/acme/platform.git' }], branch: 'main', last_commit: Math.floor(Date.now() / 1000) - 90000, last_sync: Math.floor(Date.now() / 1000) - 7200, local_hooks_path: '.husky/_' },
  { path: '/home/user/notes', name: 'notes', host: null, slug: null, owner: 'unknown', visibility: 'unknown', connected: false, remotes: [], branch: 'main', last_commit: Math.floor(Date.now() / 1000) - 400000, local_hooks_path: null },
];
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const now = Math.floor(Date.now() / 1000);
const log = [
  { ts: now - 20, source: 'hook:commit-msg', tab: 'commit', outcome: 'fixed', rules: ['strip_trailers'], messages: ['GitAssist [commit] removed 2 co-author/attribution line(s).'], repo: '/home/user/code/app' },
  { ts: now - 400, source: 'gh', tab: 'pr_create', outcome: 'blocked', rules: ['subject_max'], messages: ['GitAssist [pr_create] title is 91 chars; limit is 72. Shorten the title and retry.'], repo: '/home/user/code/a-repository-with-a-very-long-name-that-should-be-truncated-nicely' },
  { ts: now - 5000, source: 'hook:pre-commit', tab: 'ignore', outcome: 'fixed', rules: ['ignore'], messages: ['GitAssist [ignore] left .env out of this commit (personal public repo rules).'], repo: '/home/user/code/app' },
  { ts: now - 90000, source: 'setup', tab: '', outcome: 'fixed', rules: [], messages: ['Self-repair restored: git hooks.'], repo: '' },
];

function report() {
  return {
    ok: adapters.every((a) => ['current', 'not_detected'].includes(a.status)), turned_on: turnedOn, enabled: config.enabled, github_user: config.ignore.github_user,
    items: [
      { id: 'hooks', label: 'Commits & pushes', ok: turnedOn, state: turnedOn ? 'on' : 'off', summary: 'Every commit, merge and push is checked', details: ['/home/user/.local/share/gitassist/hooks'] },
      { id: 'shim', label: 'GitHub CLI', ok: true, state: turnedOn ? 'on' : 'off', summary: 'PRs, comments, reviews and issues are checked', details: ['zsh (login): uses GitAssist', 'bash (login): goes straight to gh'] },
      { id: 'autostart', label: 'Start at login', ok: true, state: 'on', summary: 'Runs quietly in the menu bar', details: [] },
      { id: 'repair', label: 'Fixes itself', ok: true, state: turnedOn ? 'on' : 'off', summary: 'Fixes anything another tool undoes', details: [] },
    ],
    adapters, repos: [],
    limits: ['Cloud agents run on remote servers.', 'Tools that call the GitHub API directly get rules as instructions only.'],
  };
}

function preview({ config: cfg, tab, title, body }) {
  const strip = cfg.tabs[tab].strip_trailers ?? true;
  const lines = body.replace(/\r/g, '').split('\n');
  const kept = strip ? lines.filter((l) => !/co-authored-by|generated with|made with|^\s*\u{1F916}/iu.test(l)) : lines;
  const out = kept.join('\n').replace(/\n{3,}/g, '\n\n').trim();
  const findings = [];
  if (kept.length !== lines.length) findings.push({ rule: 'strip_trailers', mode: 'fix', message: `GitAssist [${tab}] removed ${lines.length - kept.length} co-author/attribution line(s).` });
  const lim = cfg.tabs[tab].subject_max ?? TABS.find((t) => t.id === tab).defaults.subject_max;
  const subject = title ?? out.split('\n')[0];
  if (lim && lim.value && subject.length > lim.value) findings.push({ rule: 'subject_max', mode: lim.mode === 'fix' ? 'block' : lim.mode, message: `GitAssist [${tab}] subject is ${subject.length} chars; limit is ${lim.value}.` });
  return { title, body: out, findings, changed: out !== body };
}

const handlers = {
  get_state: () => ({ config: structuredClone(config), tabs: TABS }),
  save_config: (a) => { config = a.config; },
  preview,
  ignore_preview: ({ config: cfg, visibility, files }) => {
    const lists = [...cfg.ignore.every_repo, ...(visibility === 'private' ? cfg.ignore.private : cfg.ignore.public)];
    const hit = (f) => lists.some((p) => !p.startsWith('!') && new RegExp('(^|/)' + p.replace(/\./g, '\\.').replace(/\*/g, '[^/]*') + '$').test(f)) && !lists.some((p) => p.startsWith('!') && f.endsWith(p.slice(1)));
    return { scope: `personal ${visibility} repo`, patterns: lists, files: files.filter((f) => f.trim()).map((f) => [f, hit(f)]) };
  },
  detect_github_user: async () => { await sleep(500); return 'octo'; },
  instructions: () => '## Git and GitHub writing rules (managed by GitAssist)',
  health: async () => { await sleep(150); return report(); },
  turn_on: async () => { await sleep(900); turnedOn = true; return [{ step: 'hooks', ok: true, message: 'Every commit and push now goes through GitAssist.' }, { step: 'agents', ok: true, message: 'Set up Claude Code, Cursor.' }]; },
  install_step: async () => { await sleep(500); return 'ok'; },
  disable_autostart: async () => sleep(300),
  repair: async () => { await sleep(700); adapters.forEach((a) => a.status !== 'not_detected' && (a.status = 'current')); return ['agents']; },
  uninstall_all: async () => { await sleep(500); turnedOn = false; },
  adapter_plan: () => adapters[0],
  apply_adapter: async ({ id }) => { await sleep(600); if (id === 'codex') throw 'Codex settings did not stick; check the files are writable'; adapters.find((a) => a.id === id).status = 'current'; return 'current'; },
  remove_adapter: async ({ id }) => { await sleep(400); adapters.find((a) => a.id === id).status = 'not_applied'; return 'not_applied'; },
  activity: () => structuredClone(log),
  clear_activity: () => { log.length = 0; },
  repos_list: () => REPOS,
  repos_discover: async () => { await sleep(400); return REPOS; },
  storage: () => ({ logs: 48213, backups: 1203400, backup_sets: 7, cache: 812 }),
  clear_storage: ({ what }) => ({ logs: what === 'logs' || what === 'all' ? 0 : 48213, backups: what === 'backups' || what === 'all' ? 0 : 1203400, backup_sets: what === 'backups' || what === 'all' ? 0 : 7, cache: 0 }),
  scan_repos: async () => { await sleep(400); return [{ path: '/home/user/code/lefthook-tool', local_hooks_path: '.lefthook', connected: false }]; },
  connect_repo: async () => sleep(300),
  disconnect_repo: async () => sleep(300),
};

window.__TAURI__ = {
  core: {
    invoke: async (cmd, args) => {
      const h = handlers[cmd];
      if (!h) throw `mock: no handler for ${cmd}`;
      return structuredClone(await h(args ?? {}));
    },
  },
};
