#!/usr/bin/env bash
# End-to-end test: a real interactive bash inside util-linux `script`, with
# the shello rc blocks installed, runs a few commands and then `shello --show`.
set -eu

repo=$(cd "$(dirname "$0")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fail() { echo "FAIL: $*" >&2; exit 1; }

setup_home() {
	export HOME="$tmp/home"
	unset XDG_STATE_HOME XDG_CONFIG_HOME SHELLO_SESSION SHELLO_TTY SHELLO_DIR SHELLO_DISABLE
	mkdir -p "$HOME/.local/share/shello/hooks" "$HOME/bin"
	cp "$repo"/hooks/wrap.sh "$repo"/hooks/shello.bash "$HOME/.local/share/shello/hooks/"
	cp "$repo/shello" "$HOME/bin/"
	cat > "$HOME/.bashrc" <<'RC'
# >>> shello wrap >>>
__shello_sh=bash; [ -f "$HOME/.local/share/shello/hooks/wrap.sh" ] && . "$HOME/.local/share/shello/hooks/wrap.sh"
# <<< shello wrap <<<
PS1='$ '
# >>> shello hooks >>>
[ -f "$HOME/.local/share/shello/hooks/shello.bash" ] && . "$HOME/.local/share/shello/hooks/shello.bash"
# <<< shello hooks <<<
RC
}

# run_session FILE: feed FILE as keyboard input to an interactive bash on a pty.
run_session() {
	PATH="$HOME/bin:$PATH" timeout 20 script -q -O /dev/null -c "bash -i" < "$1" > "$tmp/terminal.txt" 2>&1 \
		|| fail "session did not exit cleanly (see terminal output below)
$(cat "$tmp/terminal.txt")"
}

# --- recording + hooks + shello --show -------------------------------------
setup_home
cat > "$tmp/input" <<'IN'
echo hello
printf 'q"uo\\te\n'
false

shello --show > "$HOME/out.txt" 2>&1
exit
exit
IN
run_session "$tmp/input"

sessions="$HOME/.local/state/shello/sessions"
cmds_files=("$sessions"/*.cmds)
[ ${#cmds_files[@]} -eq 1 ] && [ -f "${cmds_files[0]}" ] || fail "expected exactly one .cmds file, got: ${cmds_files[*]}"
lines=$(wc -l < "${cmds_files[0]}")
[ "$lines" -eq 4 ] || fail "expected 4 .cmds lines, got $lines: $(cat "${cmds_files[0]}")"

second=$(sed -n 2p "${cmds_files[0]}" | python3 -c 'import json,sys; print(json.load(sys.stdin)["cmd"], end="")')
[ "$second" = "printf 'q\"uo\\\\te\\n'" ] || fail "command text did not round-trip: [$second]"

third_exit=$(sed -n 3p "${cmds_files[0]}" | python3 -c 'import json,sys; print(json.load(sys.stdin)["exit"])')
[ "$third_exit" = 1 ] || fail "false should record exit 1, got $third_exit"

out="$HOME/out.txt"
for want in '$ echo hello' 'hello' 'exit 1' '$ false' 'q"uo\te'; do
	grep -qF -- "$want" "$out" || fail "shello --show output lacks [$want]:
$(cat "$out")"
done
! grep -qF 'shello --show' "$out" || fail "shello's own invocation leaked into the prompt"
started=("$sessions"/*.started)
[ -f "${started[0]}" ] || fail "no .started file"
perms=$(stat -c %a "${cmds_files[0]}")
[ "$perms" = 600 ] || fail ".cmds mode is $perms, want 600"
echo "ok: recording, hooks and shello --show"

# --- falls through to a normal shell when `script` is missing or broken ----
script_bin=$(command -v script)
printf 'echo fallback-$((40+2))\nexit\n' > "$tmp/input2"

# run_fallback DIR: run an interactive bash with only DIR on PATH.
run_fallback() {
	setup_home
	PATH="$1" timeout 20 "$script_bin" -q -O /dev/null -c "bash -i" < "$tmp/input2" > "$tmp/terminal2.txt" 2>&1 \
		|| fail "fallback session failed: $(cat "$tmp/terminal2.txt")"
	grep -q 'fallback-42' "$tmp/terminal2.txt" || fail "no output from fallback shell: $(cat "$tmp/terminal2.txt")"
}

nobin="$tmp/noscript"
mkdir -p "$nobin"
for t in bash tty date find mkdir cat env timeout rm; do
	p=$(command -v "$t") && ln -sf "$p" "$nobin/$t"
done
run_fallback "$nobin"
echo "ok: shell works without script"

badbin="$tmp/badscript"
cp -a "$nobin" "$badbin"
printf '#!/bin/sh\nexit 1\n' > "$badbin/script"
chmod +x "$badbin/script"
run_fallback "$badbin"
grep -q 'recording failed to start' "$tmp/terminal2.txt" || fail "no warning when script fails"
echo "ok: shell works when script fails"
