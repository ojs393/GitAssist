// Builds the `gitassist` command and places it where Tauri's `externalBin`
// expects it: app/src-tauri/binaries/gitassist-<target-triple>[.exe].
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync, chmodSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..', '..');
const target =
  process.env.TAURI_ENV_TARGET_TRIPLE ||
  execFileSync('rustc', ['--print', 'host-tuple'], { encoding: 'utf8' }).trim();
const ext = target.includes('windows') ? '.exe' : '';

execFileSync('cargo', ['build', '--release', '-p', 'gitassist-cli', '--target', target], { cwd: root, stdio: 'inherit' });

const src = join(root, 'target', target, 'release', `gitassist${ext}`);
const destDir = join(root, 'app', 'src-tauri', 'binaries');
mkdirSync(destDir, { recursive: true });
const dest = join(destDir, `gitassist-${target}${ext}`);
copyFileSync(src, dest);
if (!ext) chmodSync(dest, 0o755);
console.log(`sidecar: ${dest}`);
