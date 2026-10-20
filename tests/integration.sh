#!/usr/bin/env bash
# End-to-end test: a real interactive bash inside util-linux `script`, with
# the shello rc blocks installed, runs a few commands and then `shello --show`.
set -eu

repo=$(cd "$(dirname "$0")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fail() { echo "FAIL: $*" >&2; exit 1; }

# setup_home [SHELL]: fresh $HOME with shello installed for SHELL (default bash).
setup_home() {
	sh=${1:-bash}
	export HOME="$tmp/home-$sh"
	rm -rf "$HOME"
	unset XDG_STATE_HOME XDG_CONFIG_HOME SHELLO_SESSION SHELLO_TTY SHELLO_DIR SHELLO_DISABLE SHELLO_PENDING
	hooks="$HOME/.local/share/shello/hooks"
	mkdir -p "$hooks" "$HOME/bin"
	cp "$repo/shello" "$HOME/bin/"
	case $sh in
	bash|zsh)
		cp "$repo/hooks/wrap.sh" "$repo/hooks/shello.$sh" "$hooks/"
		cat > "$HOME/.${sh}rc" <<RC
# >>> shello wrap >>>
__shello_sh=$sh; [ -f "\$HOME/.local/share/shello/hooks/wrap.sh" ] && . "\$HOME/.local/share/shello/hooks/wrap.sh"
# <<< shello wrap <<<
PS1='\$ '
# >>> shello hooks >>>
[ -f "\$HOME/.local/share/shello/hooks/shello.$sh" ] && . "\$HOME/.local/share/shello/hooks/shello.$sh"
# <<< shello hooks <<<
RC
		;;
	fish)
		mkdir -p "$HOME/.config/fish/conf.d"
		cp "$repo/hooks/shello.fish" "$HOME/.config/fish/conf.d/"
		;;
	esac
}

# run_session SHELL FILE: feed FILE as keyboard input to an interactive SHELL on a pty.
run_session() {
	PATH="$HOME/bin:$PATH" SHELL=$(command -v "$1") timeout 20 script -q -O /dev/null -c "$1 -i" < "$2" > "$tmp/terminal.txt" 2>&1 \
		|| fail "session did not exit cleanly (see terminal output below)
$(cat "$tmp/terminal.txt")"
}

# --- recording + hooks + shello --show, for each shell ----------------------
for sh in ${SHELLS:-bash zsh fish}; do
if ! command -v "$sh" >/dev/null; then
	echo "skip: $sh not installed"
	continue
fi
setup_home "$sh"
cat > "$tmp/input" <<'IN'
echo hello
printf 'q"uo\\te\n'
false

shello --show > "$HOME/out.txt" 2>&1
exit
touch "$HOME/outer-shell-survived"
exit
IN
run_session "$sh" "$tmp/input"

sessions="$HOME/.local/state/shello/sessions"
cmds_files=("$sessions"/*.cmds)
[ ${#cmds_files[@]} -eq 1 ] && [ -f "${cmds_files[0]}" ] || fail "[$sh] expected exactly one .cmds file, got: ${cmds_files[*]}"
# fish records the final `exit` too (its postexec fires first); ignore it.
lines=$(grep -vc '"cmd":"exit"' "${cmds_files[0]}")
[ "$lines" -eq 4 ] || fail "[$sh] expected 4 .cmds lines, got $lines: $(cat "${cmds_files[0]}")"

second=$(sed -n 2p "${cmds_files[0]}" | python3 -c 'import json,sys; print(json.load(sys.stdin)["cmd"], end="")')
[ "$second" = "printf 'q\"uo\\\\te\\n'" ] || fail "[$sh] command text did not round-trip: [$second]"

third_exit=$(sed -n 3p "${cmds_files[0]}" | python3 -c 'import json,sys; print(json.load(sys.stdin)["exit"])')
[ "$third_exit" = 1 ] || fail "[$sh] false should record exit 1, got $third_exit"

out="$HOME/out.txt"
for want in '$ echo hello' 'hello' 'exit 1' '$ false' 'q"uo'; do
	grep -qF -- "$want" "$out" || fail "[$sh] shello --show output lacks [$want]:
$(cat "$out")"
done
! grep -qF 'shello --show' "$out" || fail "[$sh] shello's own invocation leaked into the prompt"
[ ! -e "$HOME/outer-shell-survived" ] || fail "outer shell kept running after the recorded shell exited"
started=("$sessions"/*.started)
[ -f "${started[0]}" ] || fail "[$sh] no .started file"
perms=$(stat -c %a "${cmds_files[0]}")
[ "$perms" = 600 ] || fail "[$sh] .cmds mode is $perms, want 600"
echo "ok: [$sh] recording, hooks and shello --show"
done

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
