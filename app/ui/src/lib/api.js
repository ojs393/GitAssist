// Tauri exposes `invoke` on window.__TAURI__ (withGlobalTauri), so no npm runtime package is needed.
export function invoke(cmd, args = {}) {
  const t = window.__TAURI__;
  if (!t) return Promise.reject(new Error('GitAssist must run inside its desktop app.'));
  return t.core.invoke(cmd, args);
}

const plain = (o) => JSON.parse(JSON.stringify(o));

export const api = {
  state: () => invoke('get_state'),
  save: (config) => invoke('save_config', { config: plain(config) }),
  preview: (config, tab, title, body) => invoke('preview', { config: plain(config), tab, title, body }),
  ignorePreview: (config, remote, visibility, files) => invoke('ignore_preview', { config: plain(config), remote, visibility, files }),
  detectGithubUser: () => invoke('detect_github_user'),
  instructions: (config) => invoke('instructions', { config: plain(config) }),
  health: (probe = true) => invoke('health', { probe }),
  turnOn: () => invoke('turn_on'),
  install: (step) => invoke('install_step', { step }),
  uninstallStep: (step) => invoke('uninstall_step', { step }),
  disableAutostart: () => invoke('disable_autostart'),
  repair: () => invoke('repair'),
  uninstall: () => invoke('uninstall_all'),
  applyAdapter: (id) => invoke('apply_adapter', { id }),
  removeAdapter: (id) => invoke('remove_adapter', { id }),
  activity: (limit) => invoke('activity', { limit }),
  storage: () => invoke('storage'),
  repos: () => invoke('repos_list'),
  discoverRepos: (root) => invoke('repos_discover', { root: root || null }),
  clearStorage: (what) => invoke('clear_storage', { what }),
  scanRepos: (root) => invoke('scan_repos', { root }),
  connectRepo: (path) => invoke('connect_repo', { path }),
  disconnectRepo: (path) => invoke('disconnect_repo', { path }),
  targetsGet: (path) => invoke('targets_get', { path }),
  targetsSave: (path, data) => invoke('targets_save', { path, data: plain(data) }),
  targetsPush: (path, which) => invoke('targets_push', { path, which }),
  jobsList: () => invoke('jobs_list'),
  jobsCancel: (id) => invoke('jobs_cancel', { id }),
  jobsResume: (id) => invoke('jobs_resume', { id }),
  jobStartPublish: (path, which) => invoke('job_start_publish', { path, which }),
  jobStartFixAll: (plan) => invoke('job_start_fix_all', { plan: plain(plan) }),
  jobStartApply: (id) => invoke('job_start_apply', { id }),
  clearActivity: () => invoke('clear_activity'),
};

export const MODES = [
  { id: 'warn', label: 'Warn', icon: 'alert', title: 'Let it through and say what is wrong' },
  { id: 'block', label: 'Block', icon: 'x', title: 'Stop it; agents read the reason and try again' },
  { id: 'fix', label: 'Fix', icon: 'wrench', title: 'Fix it automatically' },
];

// Top-level header links. Groups open their first page; in-page tabs switch siblings.
export const NAV = [
  { id: 'overview', label: 'Overview', icon: 'shield' },
  { id: 'commits', label: 'Commits', icon: 'commit', target: 'commit', children: ['commit', 'docs_commit', 'merge'] },
  { id: 'prs', label: 'Pull requests', icon: 'pr', target: 'pr_create', children: ['pr_create', 'pr_update', 'pr_comment', 'review', 'issue'] },
  { id: 'repogroup', label: 'Repos', icon: 'folder', target: 'repos', children: ['repos', 'branches', 'ignore'] },
  { id: 'activity', label: 'Activity', icon: 'activity' },
];

// Non-rules pages that share a header group (shown as in-page tabs).
export const PAGES = {
  repos: { label: 'Your repos', icon: 'folder' },
  branches: { label: 'Branches', icon: 'branch' },
  ignore: { label: 'Ignore', icon: 'eye-off' },
};

export const TAB_ICONS = {
  commit: 'commit',
  docs_commit: 'docs',
  merge: 'merge',
  pr_create: 'pr',
  pr_update: 'edit',
  pr_comment: 'message',
  review: 'review',
  issue: 'issue',
};

// Samples show what agents typically write, so the preview has something to clean up.
export const SAMPLES = {
  commit:
    'feat: add retry to the upload client\n\nRetries failed uploads three times with exponential backoff so flaky networks no longer drop files halfway through a sync.\n\nGenerated with [Claude Code](https://claude.com/claude-code)\n\nCo-Authored-By: Claude <noreply@anthropic.com>',
  docs_commit: 'docs: explain upload retries\n\nCo-authored-by: Cursor <cursoragent@cursor.com>',
  merge: "Merge branch 'feature/retries' into main\n\nCo-authored-by: Copilot <copilot@github.com>",
  pr_create: '## Summary\nAdds retries to the upload client.\n\n## Testing\nUnit tests for the backoff schedule.\n\nMade with [Cursor](https://cursor.com)',
  pr_update: '## Summary\nAdds retries to the upload client, now configurable.\n\nGenerated with [Claude Code](https://claude.com/claude-code)',
  pr_comment: 'Good catch, fixed in the latest push.\n\nGenerated with [Claude Code](https://claude.com/claude-code)',
  review: 'Looks good overall; please add a test for the timeout path.',
  issue: 'Uploads fail on flaky networks\n\nSteps to reproduce:\n1. Throttle the network\n2. Upload a large file',
};

export const SAMPLE_TITLES = {
  pr_create: 'Add retries to the upload client',
  pr_update: 'Add configurable retries to the upload client',
  issue: 'Uploads fail on flaky networks',
};

export const TEMPLATES = {
  commit: [
    { name: 'Subject only', text: '{type}: {summary}' },
    { name: 'Subject + why', text: '{type}: {summary}\n\nWhy:\n{body}' },
    { name: 'What / Why / Testing', text: '{type}: {summary}\n\nWhat:\n{what}\n\nWhy:\n{why}\n\nTesting:\n{tests}' },
  ],
  markdown: [
    { name: 'Summary + testing', text: '## Summary\n{summary}\n\n## Testing\n{tests}' },
    { name: 'Summary + table', text: '## Summary\n{summary}\n\n## Changes\n| Area | Change |\n|------|--------|\n{table}\n\n## Testing\n{tests}' },
    { name: 'Problem / Fix / Risk', text: '## Problem\n{problem}\n\n## Fix\n{fix}\n\n## Risk\n{risk}' },
  ],
};

// One-click length presets per tab kind. `total` is the whole message.
export const LENGTH_PRESETS = {
  commit: [
    { id: 'short', label: 'Short', icon: 'minimize', note: 'One line', rules: { subject_max: [72, 'block'], total_max: [200, 'block'], body_max: [0, 'warn'] } },
    { id: 'standard', label: 'Standard', icon: 'layout', note: 'Under 500', rules: { subject_max: [72, 'warn'], total_max: [500, 'block'], body_max: [0, 'warn'] } },
    { id: 'detailed', label: 'Detailed', icon: 'maximize', note: 'Up to 2,000', rules: { subject_max: [72, 'warn'], total_max: [2000, 'warn'], body_max: [0, 'warn'] } },
  ],
  markdown: [
    { id: 'short', label: 'Short', icon: 'minimize', note: 'A few lines', rules: { subject_max: [72, 'block'], total_max: [800, 'block'], body_max: [0, 'warn'] } },
    { id: 'standard', label: 'Standard', icon: 'layout', note: 'Up to 3,000', rules: { subject_max: [72, 'warn'], total_max: [3000, 'warn'], body_max: [0, 'warn'] } },
    { id: 'detailed', label: 'Detailed', icon: 'maximize', note: 'Up to 10,000', rules: { subject_max: [100, 'warn'], total_max: [10000, 'warn'], body_max: [0, 'warn'] } },
  ],
};

export const BRANCH_PRESETS = [
  { id: 'solo', label: 'Solo', icon: 'user', note: 'Anything goes', apply: (b) => ({ ...b, protected: [], prefixes: [] }) },
  { id: 'team', label: 'Team', icon: 'building', note: 'Warn on main', apply: (b) => ({ ...b, protected: ['main', 'master'], protect_mode: 'warn', prefixes: ['feat', 'fix', 'docs', 'chore'], naming_mode: 'warn' }) },
  { id: 'strict', label: 'Strict', icon: 'lock', note: 'Block main', apply: (b) => ({ ...b, protected: ['main', 'master'], protect_mode: 'block', prefixes: ['feat', 'fix', 'docs', 'chore'], naming_mode: 'block' }) },
];

export function formatBytes(n) {
  if (!n) return '0 KB';
  const units = ['B', 'KB', 'MB', 'GB'];
  const i = Math.min(units.length - 1, Math.floor(Math.log(n) / Math.log(1024)));
  return `${(n / 1024 ** i).toFixed(i ? 1 : 0)} ${units[i]}`;
}
