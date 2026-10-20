#!/usr/bin/env bash
# Runs every scenario (scenarios/*/run.sh) in a real recorded bash session, so
# `shello` sees the scenario's commands exactly as it would in your terminal.
# This calls the configured AI agent for real, so it needs network access.
#
# Usage: scenarios/run_all.sh [SCENARIO...]    (default: all of them)
set -eu

repo=$(cd "$(dirname "$0")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

failed=()

# setup_home: a throwaway $HOME with this checkout's shello installed, but
# with the agent's login and settings shared from the real one.
setup_home() {
	export REAL_HOME=${REAL_HOME:-$HOME}
	export HOME="$tmp/home"
	mkdir -p "$HOME/.local/share/shello/hooks" "$HOME/bin" "$HOME/.config"
	cp "$repo/shello" "$HOME/bin/"
	cp "$repo"/hooks/* "$HOME/.local/share/shello/hooks/"
	write_bashrc > "$HOME/.bashrc"
	link_from_real_home .claude .claude.json .config/shello
	unset XDG_STATE_HOME XDG_CONFIG_HOME SHELLO_SESSION SHELLO_TTY SHELLO_DIR SHELLO_DISABLE SHELLO_PENDING
}

write_bashrc() {
	cat <<'RC'
__shello_sh=bash; . "$HOME/.local/share/shello/hooks/wrap.sh"
PS1='$ '
. "$HOME/.local/share/shello/hooks/shello.bash"
RC
}

link_from_real_home() {
	local name

	for name in "$@"; do
		[ -e "$REAL_HOME/$name" ] && ln -s "$REAL_HOME/$name" "$HOME/$name"
	done

	return 0
}

# scenario_input DIR: the lines to type into the shell (no shebang or blanks).
scenario_input() {
	awk '!/^#!/ && NF' "$1/run.sh"
	echo exit
}

run_scenario() {
	local dir=$1 name work
	name=$(basename "$dir")
	work="$tmp/work/$name"

	mkdir -p "$tmp/work"
	cp -r "$dir" "$work"

	echo "=== $name"
	scenario_input "$dir" \
		| (cd "$work" && PATH="$HOME/bin:$PATH" SHELL=$(command -v bash) timeout 300 script -q -O /dev/null -c bash) \
		|| failed+=("$name")
}

scenario_dirs() {
	local name

	if [ $# -eq 0 ]; then
		printf '%s\n' "$repo"/scenarios/*/
		return
	fi

	for name in "$@"; do
		printf '%s\n' "$repo/scenarios/$name"
	done
}

main() {
	local dir

	[ -x "$repo/shello" ] || { echo "no $repo/shello; run 'make build' first" >&2; exit 1; }
	setup_home

	while IFS= read -r dir; do
		run_scenario "${dir%/}"
	done < <(scenario_dirs "$@")

	if [ ${#failed[@]} -gt 0 ]; then
		echo "FAILED: ${failed[*]}" >&2
		exit 1
	fi
}

main "$@"
