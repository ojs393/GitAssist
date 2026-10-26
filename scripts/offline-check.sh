#!/usr/bin/env bash
# Proves the gitassist binary works with no network and never opens a
# non-local connection. Linux only (uses unshare and strace).
set -euo pipefail

BIN="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

export HOME="$WORK/home" GITASSIST_HOME="$WORK/ga" GIT_CONFIG_GLOBAL="$WORK/gitconfig" GIT_CONFIG_NOSYSTEM=1
mkdir -p "$HOME" "$WORK/repo" "$WORK/fakegh"
printf '[user]\n\tname = CI\n\temail = ci@example.com\n[init]\n\tdefaultBranch = main\n' > "$GIT_CONFIG_GLOBAL"
printf '#!/bin/sh\nexit 0\n' > "$WORK/fakegh/gh" && chmod +x "$WORK/fakegh/gh"
export PATH="$GITASSIST_HOME/bin:$WORK/fakegh:$PATH"

"$BIN" install hooks
"$BIN" install shim
cd "$WORK/repo"
git init -q
echo a > a.txt && git add a.txt

: > "$WORK/trace"
chmod 666 "$WORK/trace"
# Unprivileged network namespaces are blocked on some hosts (GitHub's Ubuntu
# runners); fall back to making the namespace as root and dropping back to us.
if unshare -rn true 2>/dev/null; then
  run() { strace -f -qq -A -e trace=connect -o "$WORK/trace" unshare -rn -- "$@"; }
else
  ME="$(id -un)"
  run() {
    sudo unshare -n -- sudo -u "$ME" env PATH="$PATH" HOME="$HOME" GITASSIST_HOME="$GITASSIST_HOME" \
      GIT_CONFIG_GLOBAL="$GIT_CONFIG_GLOBAL" GIT_CONFIG_NOSYSTEM=1 \
      strace -f -qq -A -e trace=connect -o "$WORK/trace" "$@"
  }
fi

run git commit -q -m $'feat: offline\n\nCo-authored-by: Claude <c@x>'
test "$(git log -1 --format=%B | tr -d '\n')" = "feat: offline"
run "$GITASSIST_HOME/bin/gh" pr create --title t --body b
run "$BIN" status --json > /dev/null || true

if grep -E 'connect\(' "$WORK/trace" | grep -vE 'AF_UNIX|AF_NETLINK|sin_addr=inet_addr\("127\.0\.0\.1"\)' ; then
  echo "unexpected network connection" >&2
  exit 1
fi
echo "offline check passed"
