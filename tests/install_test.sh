#!/usr/bin/env bash
# Tests install.sh against a throwaway $HOME: install, reinstall, uninstall, purge.
set -eu

repo=$(cd "$(dirname "$0")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fail() { echo "FAIL: $*" >&2; exit 1; }

# Keep the real Rust toolchain reachable once $HOME is swapped out.
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}" CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export HOME="$tmp/home"
unset XDG_CONFIG_HOME XDG_STATE_HOME XDG_DATA_HOME ZDOTDIR
mkdir -p "$HOME"
printf 'echo existing\n' > "$HOME/.bashrc"
cp "$HOME/.bashrc" "$tmp/bashrc.orig"

count() { grep -cxF -- "$1" "$HOME/.bashrc" || true; }

# --- install twice --------------------------------------------------------
SHELL=/bin/bash "$repo/install.sh" > "$tmp/install.log" 2>&1 || fail "install failed: $(cat "$tmp/install.log")"
SHELL=/bin/bash "$repo/install.sh" > "$tmp/install.log" 2>&1 || fail "reinstall failed: $(cat "$tmp/install.log")"

[ -x "$HOME/.local/bin/shello" ] || fail "binary not installed"
for f in wrap.sh shello.bash shello.zsh shello.fish; do
	[ -f "$HOME/.local/share/shello/hooks/$f" ] || fail "hook $f not installed"
done
[ -f "$HOME/.config/shello/config.toml" ] || fail "config not installed"
[ -f "$HOME/.config/shello/prompt.md" ] || fail "prompt not installed"
[ "$(head -n1 "$HOME/.bashrc")" = "# >>> shello wrap >>>" ] || fail "wrap block not at top: $(cat "$HOME/.bashrc")"
[ "$(tail -n1 "$HOME/.bashrc")" = "# <<< shello hooks <<<" ] || fail "hooks block not at bottom"
for m in "# >>> shello wrap >>>" "# <<< shello wrap <<<" "# >>> shello hooks >>>" "# <<< shello hooks <<<"; do
	[ "$(count "$m")" = 1 ] || fail "marker [$m] appears $(count "$m") times"
done
grep -qxF 'echo existing' "$HOME/.bashrc" || fail "existing rc content lost"
grep -q 'Open a new terminal' "$tmp/install.log" || fail "no next-step message"
echo "ok: install is complete and idempotent"

# --- user config is never overwritten ---------------------------------------
printf 'agent = "codex"\n' > "$HOME/.config/shello/config.toml"
SHELL=/bin/bash "$repo/install.sh" > /dev/null 2>&1
[ "$(cat "$HOME/.config/shello/config.toml")" = 'agent = "codex"' ] || fail "config was overwritten"
echo "ok: config kept on reinstall"

# --- uninstall ---------------------------------------------------------------
mkdir -p "$HOME/.local/state/shello/sessions"
"$repo/install.sh" --uninstall > /dev/null 2>&1 || fail "uninstall failed"
cmp -s "$HOME/.bashrc" "$tmp/bashrc.orig" || fail "rc not restored: $(cat "$HOME/.bashrc")"
[ ! -e "$HOME/.local/bin/shello" ] || fail "binary left behind"
[ ! -e "$HOME/.local/share/shello" ] || fail "hooks left behind"
[ -f "$HOME/.config/shello/config.toml" ] || fail "config removed without --purge"
[ -d "$HOME/.local/state/shello" ] || fail "logs removed without --purge"
echo "ok: uninstall restores rc and keeps config"

"$repo/install.sh" --uninstall --purge > /dev/null 2>&1 || fail "purge failed"
[ ! -e "$HOME/.config/shello" ] || fail "config left after purge"
[ ! -e "$HOME/.local/state/shello" ] || fail "logs left after purge"
echo "ok: purge removes config and logs"

# --- fish and errors -----------------------------------------------------------
"$repo/install.sh" --shell fish > /dev/null 2>&1 || fail "fish install failed"
[ -f "$HOME/.config/fish/conf.d/shello.fish" ] || fail "fish conf.d hook not installed"
"$repo/install.sh" --uninstall > /dev/null 2>&1
[ ! -e "$HOME/.config/fish/conf.d/shello.fish" ] || fail "fish hook left behind"
echo "ok: fish install and uninstall"

if SHELL=/bin/tcsh "$repo/install.sh" > "$tmp/err.log" 2>&1; then
	fail "unsupported shell accepted"
fi
grep -q -- '--shell' "$tmp/err.log" || fail "unsupported-shell error doesn't mention --shell"
echo "ok: unsupported shell rejected"
