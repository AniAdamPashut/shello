#!/bin/sh
# Installs shello: the binary, the shell hooks, a default config, and the
# lines in your shell's startup file that turn recording on.
#
#   ./install.sh [--shell bash|zsh|fish|nu]   install (shell defaults to $SHELL)
#   ./install.sh --uninstall [--purge]     remove; --purge also deletes config and logs
set -eu

REPO=$(cd "$(dirname "$0")" && pwd)
BIN_DIR="$HOME/.local/bin"
SHARE_DIR="$HOME/.local/share/shello"
HOOKS_DIR="$SHARE_DIR/hooks"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/shello"
STATE_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/shello"
FISH_HOOK="${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/shello.fish"
NU_HOOK="${XDG_CONFIG_HOME:-$HOME/.config}/nushell/autoload/shello.nu"
NEWLINE_NOTE="# (shello added a newline at the end of the line above)"

die() {
	echo "install.sh: $*" >&2
	exit 1
}

usage() {
	sed -n '2,6s/^# \{0,1\}//p' "$0"
}

main() {
	shell=""
	uninstall=0
	purge=0
	parse_args "$@"

	if [ "$uninstall" = 1 ]; then
		uninstall
	else
		install
	fi
}

parse_args() {
	while [ $# -gt 0 ]; do
		case $1 in
		--shell) [ $# -ge 2 ] || die "--shell needs a value"; shell=$2; shift ;;
		--uninstall) uninstall=1 ;;
		--purge) purge=1 ;;
		-h | --help) usage; exit 0 ;;
		*) die "unknown option $1 (see --help)" ;;
		esac
		shift
	done
}

# --- install -----------------------------------------------------------------

install() {
	shell=${shell:-$(basename "${SHELL:-}")}
	case $shell in
	bash | zsh | fish | nu) ;;
	*) die "unsupported shell '$shell'; use --shell bash|zsh|fish|nu" ;;
	esac

	check_script
	install_binary
	install_hooks
	install_config
	enable_recording
	finish_message
}

check_script() {
	if ! script --version 2>/dev/null | grep -q util-linux; then
		die "shello needs util-linux script"
	fi
}

# Builds from source when cargo is available, so a stale binary is never
# installed; otherwise uses a prebuilt ./shello next to this script.
install_binary() {
	if [ -f "$REPO/Cargo.toml" ] && command -v cargo >/dev/null 2>&1; then
		echo "Building shello..."
		cargo build --release --quiet --manifest-path "$REPO/Cargo.toml"
		binary="$REPO/target/release/shello"
	elif [ -x "$REPO/shello" ]; then
		binary="$REPO/shello"
	else
		die "no prebuilt shello binary and cargo is not installed"
	fi

	mkdir -p "$BIN_DIR"
	cp "$binary" "$BIN_DIR/shello.tmp"
	chmod 755 "$BIN_DIR/shello.tmp"
	mv "$BIN_DIR/shello.tmp" "$BIN_DIR/shello"
}

install_hooks() {
	mkdir -p "$HOOKS_DIR"
	for f in wrap.sh sweep.sh shello.bash shello.zsh shello.fish shello.nu; do
		cp "$REPO/hooks/$f" "$HOOKS_DIR/$f"
		chmod 644 "$HOOKS_DIR/$f"
	done
}

# Copies the default config and prompt, never overwriting the user's.
install_config() {
	mkdir -p "$CONFIG_DIR"
	for f in config.toml prompt.md; do
		[ -e "$CONFIG_DIR/$f" ] || cp "$REPO/$f" "$CONFIG_DIR/$f"
	done
}

enable_recording() {
	case $shell in
	bash) add_rc_blocks "$HOME/.bashrc" bash ;;
	zsh) add_rc_blocks "${ZDOTDIR:-$HOME}/.zshrc" zsh ;;
	fish) mkdir -p "$(dirname "$FISH_HOOK")" && cp "$HOOKS_DIR/shello.fish" "$FISH_HOOK" ;;
	nu) mkdir -p "$(dirname "$NU_HOOK")" && cp "$HOOKS_DIR/shello.nu" "$NU_HOOK" ;;
	esac
}

# Puts the wrapper block at the top of the rc file (so recording starts before
# the rest of it runs) and the hooks block at the bottom (after other tools
# have set up the prompt). Old blocks are removed first, so reruns are safe.
add_rc_blocks() {
	rc=$1
	sh_name=$2
	touch "$rc"
	remove_rc_blocks "$rc"

	# A last line without a newline would merge with the hooks marker.
	newline=no
	if [ -s "$rc" ] && [ -n "$(tail -c1 "$rc")" ]; then
		newline=yes
	fi

	{
		wrap_block "$sh_name"
		cat "$rc"
		[ "$newline" = no ] || echo
		hooks_block "$sh_name" "$newline"
	} > "$rc.shello-tmp"

	replace_file "$rc" "$rc.shello-tmp"
}

wrap_block() {
	echo "# >>> shello wrap >>>"
	echo "__shello_sh=$1; [ -f \"\$HOME/.local/share/shello/hooks/wrap.sh\" ] && . \"\$HOME/.local/share/shello/hooks/wrap.sh\""
	echo "# <<< shello wrap <<<"
}

# hooks_block SHELL ADDED_NEWLINE: the block notes when install.sh had to end
# the user's last line, so uninstall can take that newline back out.
hooks_block() {
	echo "# >>> shello hooks >>>"
	[ "$2" = no ] || echo "$NEWLINE_NOTE"
	echo "[ -f \"\$HOME/.local/share/shello/hooks/shello.$1\" ] && . \"\$HOME/.local/share/shello/hooks/shello.$1\""
	echo "# <<< shello hooks <<<"
}

finish_message() {
	case ":$PATH:" in
	*":$BIN_DIR:"*) ;;
	*) echo "Warning: $BIN_DIR is not on your PATH; add it to run shello." ;;
	esac

	echo "Installed shello for $shell. Open a new terminal to start recording."
}

# --- uninstall ---------------------------------------------------------------

uninstall() {
	rm -f "$BIN_DIR/shello" "$FISH_HOOK" "$NU_HOOK"
	rm -rf "$SHARE_DIR"
	for rc in "$HOME/.bashrc" "${ZDOTDIR:-$HOME}/.zshrc"; do
		[ -f "$rc" ] && remove_rc_blocks "$rc"
	done

	if [ "$purge" = 1 ]; then
		rm -rf "$CONFIG_DIR" "$STATE_DIR"
	fi

	echo "Uninstalled shello. Open a new terminal to stop recording."
}

remove_rc_blocks() {
	added_newline=no
	grep -qxF "$NEWLINE_NOTE" "$1" && added_newline=yes
	sed -e '/^# >>> shello wrap >>>$/,/^# <<< shello wrap <<<$/d' \
		-e '/^# >>> shello hooks >>>$/,/^# <<< shello hooks <<<$/d' \
		"$1" > "$1.shello-tmp"

	# $(...) drops the trailing newline install.sh added.
	if [ "$added_newline" = yes ]; then
		content=$(cat "$1.shello-tmp")
		printf '%s' "$content" > "$1.shello-tmp"
	fi

	replace_file "$1" "$1.shello-tmp"
}

# Writes new's content into target in place, keeping target's permissions
# and any symlink (dotfile managers often symlink rc files).
replace_file() {
	cat "$2" > "$1"
	rm -f "$2"
}

main "$@"
