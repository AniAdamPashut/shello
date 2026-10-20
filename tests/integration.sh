#!/usr/bin/env bash
# End-to-end tests: real interactive shells inside util-linux `script`, with
# the shello rc blocks installed, run commands and then `shello --show`.
#
# SHELLS="bash zsh fish" picks the shells to test (missing ones are skipped).
set -eu

repo=$(cd "$(dirname "$0")/.." && pwd)
start_dir=$PWD
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fail() {
	echo "FAIL: [$sh] $*" >&2
	exit 1
}

# --- helpers -------------------------------------------------------------------

# setup_home SHELL: fresh $HOME with shello installed for SHELL.
setup_home() {
	sh=$1
	export HOME="$tmp/home-$sh"
	rm -rf "$HOME"
	unset XDG_STATE_HOME XDG_CONFIG_HOME SHELLO_SESSION SHELLO_TTY SHELLO_DIR SHELLO_DISABLE SHELLO_PENDING
	mkdir -p "$HOME/.local/share/shello/hooks" "$HOME/bin"
	cp "$repo/shello" "$HOME/bin/"
	cp "$repo"/hooks/* "$HOME/.local/share/shello/hooks/"
	case $sh in
	bash | zsh) write_rc "$sh" > "$HOME/.${sh}rc" ;;
	fish) mkdir -p "$HOME/.config/fish/conf.d" && cp "$repo/hooks/shello.fish" "$HOME/.config/fish/conf.d/" ;;
	esac
}

# write_rc SHELL: an rc file like install.sh writes, with some typical
# content in between (bash's PROMPT_COMMAND is overwritten by the rc, as
# many real setups do).
write_rc() {
	cat <<RC
# >>> shello wrap >>>
__shello_sh=$1; [ -f "\$HOME/.local/share/shello/hooks/wrap.sh" ] && . "\$HOME/.local/share/shello/hooks/wrap.sh"
# <<< shello wrap <<<
PS1='\$ '
RC
	[ "$1" = bash ] && echo "PROMPT_COMMAND='history -a'"
	cat <<RC
# >>> shello hooks >>>
[ -f "\$HOME/.local/share/shello/hooks/shello.$1" ] && . "\$HOME/.local/share/shello/hooks/shello.$1"
# <<< shello hooks <<<
RC
}

rc_file() {
	case $sh in
	fish) echo "$HOME/.config/fish/conf.d/shello.fish" ;;
	*) echo "$HOME/.${sh}rc" ;;
	esac
}

# run_session COMMAND INPUT [DELAY]: run COMMAND on a pty with INPUT as
# keyboard input, typed one line every DELAY seconds (default: all at once).
# An input line "@pause N" waits N seconds instead of typing anything.
run_session() {
	type_lines "$2" "${3:-0}" \
		| PATH="$HOME/bin:$PATH" SHELL=$(command -v "$sh") timeout 60 script -q -O /dev/null -c "$1" \
			> "$tmp/terminal.txt" 2>&1 \
		|| fail "session did not exit cleanly; terminal output:
$(cat "$tmp/terminal.txt")"
}

type_lines() {
	if [ "$2" = 0 ]; then
		cat "$1"
		return
	fi

	while IFS= read -r line; do
		case $line in
		"@pause "*) sleep "${line#@pause }" ;;
		*) printf '%s\n' "$line" && sleep "$2" ;;
		esac
	done < "$1"
}

# record FILE LINE FIELD: one field of one .cmds record.
record() {
	sed -n "$2p" "$1" | python3 -c 'import json,sys; print(json.load(sys.stdin).get(sys.argv[1], ""), end="")' "$3"
}

# block_output FILE CMD: the output shown for "$ CMD" in a shello --show prompt.
block_output() {
	python3 - "$1" "$2" <<'PY'
import sys
lines = open(sys.argv[1]).read().split("\n")
i = lines.index("$ " + sys.argv[2])
j = lines.index("```", i)
k = lines.index("```", j + 1)
print("\n".join(lines[j + 1:k]))
PY
}

expect_in() {
	grep -qF -- "$1" "$2" || fail "$2 lacks [$1]:
$(cat "$2")"
}

expect_not_in() {
	! grep -qF -- "$1" "$2" || fail "$2 unexpectedly contains [$1]:
$(cat "$2")"
}

outer_exited() {
	[ ! -e "$HOME/outer-shell-survived" ] || fail "outer shell kept running after the recorded shell exited"
}

hide_setup() {
	case $sh in
	bash) echo "HISTCONTROL=ignorespace" ;;
	zsh) echo "setopt histignorespace" ;;
	fish) echo "true" ;;
	esac
}

# --- scenarios -------------------------------------------------------------------

test_recording() {
	setup_home "$sh"
	{
		hide_setup
		cat <<'IN'
cd /tmp
echo hello
printf 'q"uo\\te\n'
 echo secret-hidden
false

cp "$SHELLO_DIR/$SHELLO_SESSION.cmds" "$HOME/cmds.copy"
stat -c %a "$SHELLO_DIR/$SHELLO_SESSION.cmds" > "$HOME/perms"
shello --show -n 7 > "$HOME/out.txt" 2>&1
exit
touch "$HOME/outer-shell-survived"
exit
IN
	} > "$tmp/input"
	run_session "$sh -i" "$tmp/input"

	cmds="$HOME/cmds.copy"
	[ "$(wc -l < "$cmds")" -eq 6 ] || fail "expected 6 records: $(cat "$cmds")"
	[ "$(record "$cmds" 2 cwd)" = "$start_dir" ] || fail "cd should record the dir it ran in: $(sed -n 2p "$cmds")"
	[ "$(record "$cmds" 3 cwd)" = /tmp ] || fail "echo should record cwd /tmp: $(sed -n 3p "$cmds")"
	[ "$(record "$cmds" 4 cmd)" = "printf 'q\"uo\\\\te\\n'" ] || fail "command text did not round-trip: $(sed -n 4p "$cmds")"
	[ "$(record "$cmds" 5 hidden)" = True ] || fail "hidden command not marked hidden: $(sed -n 5p "$cmds")"
	[ "$(record "$cmds" 6 exit)" = 1 ] || fail "false should record exit 1"
	expect_not_in secret-hidden "$cmds"
	[ "$(cat "$HOME/perms")" = 600 ] || fail ".cmds mode is $(cat "$HOME/perms"), want 600"

	out="$HOME/out.txt"
	for want in '$ echo hello' 'cwd: /tmp' 'hello' 'exit 1' '$ false' 'q"uo' 'hidden command'; do
		expect_in "$want" "$out"
	done
	expect_not_in 'shello --show' "$out"
	expect_not_in secret-hidden "$out"
	outer_exited
	[ -z "$(ls -A "$HOME/.local/state/shello/sessions")" ] || fail "session files left after the terminal closed"
	echo "ok: [$sh] recording, cwd, hidden commands, cleanup on exit"
}

test_exec_reload() {
	setup_home "$sh"
	# fish 4 waits up to 10s at startup for terminal replies the test can't give.
	pause=0
	[ "$sh" != fish ] || pause=11
	cat > "$tmp/input" <<IN
echo first-run
exec $sh
@pause $pause
echo after-exec
shello --show -n 2 > "\$HOME/out.txt" 2>&1
exit
touch "\$HOME/outer-shell-survived"
exit
IN
	# Typed slowly: fish reads ahead, and exec would drop what it had buffered.
	run_session "$sh -i" "$tmp/input" 0.5

	[ "$(block_output "$HOME/out.txt" "echo first-run")" = first-run ] || fail "wrong output after exec $sh:
$(cat "$HOME/out.txt")"
	[ "$(block_output "$HOME/out.txt" "echo after-exec")" = after-exec ] || fail "exec'd shell not recorded"
	outer_exited
	echo "ok: [$sh] exec $sh keeps outputs apart"
}

test_resource_rc() {
	setup_home "$sh"
	cat > "$tmp/input" <<IN
source $(rc_file)
echo after-source
shello --show -n 1 > "\$HOME/out.txt" 2>&1
exit
exit
IN
	run_session "$sh -i" "$tmp/input"

	expect_in '$ echo after-source' "$HOME/out.txt"
	echo "ok: [$sh] still recording after re-sourcing the rc"
}

test_repeated_command() {
	setup_home "$sh"
	cat > "$tmp/input" <<'IN'
HISTCONTROL=ignoredups
echo again
echo again
shello --show -n 2 > "$HOME/out.txt" 2>&1
exit
exit
IN
	run_session "$sh -i" "$tmp/input"

	[ "$(grep -cxF '$ echo again' "$HOME/out.txt")" = 2 ] || fail "repeated command not recorded twice:
$(cat "$HOME/out.txt")"
	echo "ok: [$sh] repeated commands recorded with ignoredups"
}

test_command_string() {
	setup_home "$sh"
	run_session "$sh -ic 'echo RAN-IT'" /dev/null

	expect_in RAN-IT "$tmp/terminal.txt"
	echo "ok: [$sh] $sh -ic runs its command"
}

test_login_shell() {
	setup_home "$sh"
	case $sh in
	bash) printf '. ~/.bashrc\nexport PROFILE_AFTER=yes\n' > "$HOME/.bash_profile" ;;
	zsh) echo 'export PROFILE_AFTER=yes' > "$HOME/.zlogin" ;;
	fish) printf 'if status is-login\n  set -gx PROFILE_AFTER yes\nend\n' > "$HOME/.config/fish/config.fish" ;;
	esac
	{
		echo 'echo "pa=$PROFILE_AFTER"'
		# Prints rec=42 only inside a recording; computed so the echoed input can't match.
		case $sh in
		fish) echo 'set -q SHELLO_SESSION; and echo rec=(math 40 + 2)' ;;
		*) echo '[ -n "$SHELLO_SESSION" ] && echo "rec=$((40 + 2))"' ;;
		esac
		printf 'exit\nexit\n'
	} > "$tmp/input"
	run_session "$sh -l -i" "$tmp/input"

	expect_in pa=yes "$tmp/terminal.txt"
	expect_in rec=42 "$tmp/terminal.txt"

	TERM=linux run_session "$sh -l -i" "$tmp/input"
	expect_not_in rec=42 "$tmp/terminal.txt"
	echo "ok: [$sh] login shells keep their profile; Linux console not recorded"
}

test_sweep() {
	dir="$tmp/sweep"
	mkdir -p "$dir"
	sleep 300 &
	alive=$!
	for id in "20200101-000000-$alive" "20200101-000000-999999" "20990101-000000-999998"; do
		touch "$dir/$id.log" "$dir/$id.cmds" "$dir/$id.started"
	done
	touch -d '10 days ago' "$dir"/20200101-*

	sh "$repo/hooks/sweep.sh" "$dir"
	kill "$alive"

	[ -e "$dir/20200101-000000-$alive.started" ] || fail "sweep deleted a live session"
	[ ! -e "$dir/20200101-000000-999999.log" ] || fail "sweep kept an old dead session"
	[ -e "$dir/20990101-000000-999998.log" ] || fail "sweep deleted a recent session"
	echo "ok: sweep removes only old sessions whose shell is gone"
}

# --- falls through to a normal shell when `script` is missing or broken --------

# run_fallback DIR: run an interactive bash with only DIR on PATH.
run_fallback() {
	setup_home bash
	printf 'echo fallback-$((40+2))\nexit\n' > "$tmp/input2"
	PATH="$1" timeout 20 "$script_bin" -q -O /dev/null -c "bash -i" < "$tmp/input2" > "$tmp/terminal2.txt" 2>&1 \
		|| fail "fallback session failed: $(cat "$tmp/terminal2.txt")"
	grep -q 'fallback-42' "$tmp/terminal2.txt" || fail "no output from fallback shell: $(cat "$tmp/terminal2.txt")"
}

test_fallbacks() {
	script_bin=$(command -v script)
	nobin="$tmp/noscript"
	mkdir -p "$nobin"
	for t in bash sh tty date find mkdir cat env timeout rm kill sed; do
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
}

# --- main ------------------------------------------------------------------------

# ONLY=test_name runs a single scenario.
run() {
	[ -z "${ONLY:-}" ] || [ "$ONLY" = "$1" ] || return 0
	"$1"
}

for sh in ${SHELLS:-bash zsh fish}; do
	if ! command -v "$sh" > /dev/null; then
		echo "skip: $sh not installed"
		continue
	fi

	run test_recording
	run test_exec_reload
	run test_resource_rc
	run test_command_string
	run test_login_shell
	[ "$sh" != bash ] || run test_repeated_command
done

sh=bash
run test_sweep
run test_fallbacks
