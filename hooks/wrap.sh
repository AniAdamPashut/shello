# shello: sourced at the top of ~/.bashrc or ~/.zshrc (set __shello_sh first).
#
# Re-runs this interactive shell inside util-linux `script`, which records
# everything the terminal shows to a log that shello reads. If recording
# can't start, the shell carries on as normal.

# True when this shell is an interactive session worth recording.
__shello_wanted() {
	case $- in *i*) ;; *) return 1 ;; esac
	[ -z "${SHELLO_DISABLE:-}" ] || return 1
	[ -t 0 ] && [ -t 1 ] || return 1
	# `bash -ic CMD` and `zsh -ic CMD` run one command, not a session.
	[ -z "${BASH_EXECUTION_STRING:-}${ZSH_EXECUTION_STRING:-}" ] || return 1
	# The Linux console, where login profiles often start a desktop session.
	[ "${TERM:-}" != linux ]
}

# zsh's `[` can't test options, so this uses `[[`, which bash parses too.
__shello_is_login() {
	if [ -n "${BASH_VERSION:-}" ]; then
		shopt -q login_shell
	else
		[[ -o login ]]
	fi
}

__shello_wrap() {
	__shello_wanted || return 0
	__shello_tty=$(tty 2>/dev/null) || return 0

	# Inside the recording: the first shell on the new pty claims the session.
	if [ -n "${SHELLO_PENDING:-}" ] && [ -n "${SHELLO_SESSION:-}" ]; then
		unset SHELLO_PENDING
		export SHELLO_TTY="$__shello_tty"
		(umask 077 && : > "$SHELLO_DIR/$SHELLO_SESSION.started")
		return 0
	fi

	# This terminal is already recorded (e.g. a subshell).
	if [ -n "${SHELLO_SESSION:-}" ] && [ "${SHELLO_TTY:-}" = "$__shello_tty" ]; then
		return 0
	fi

	command -v script >/dev/null 2>&1 || return 0
	__shello_record
}

# Runs the shell again inside `script`. When that session ends, its files
# are deleted (they're only useful while the terminal is open) and this
# shell exits with the same status.
__shello_record() {
	__shello_dir=${XDG_STATE_HOME:-$HOME/.local/state}/shello/sessions
	__shello_id=$(date +%Y%m%d-%H%M%S)-$$
	(umask 077 && mkdir -p "$__shello_dir" \
		&& : > "$__shello_dir/$__shello_id.log" \
		&& : > "$__shello_dir/$__shello_id.cmds") 2>/dev/null || return 0
	sh "$HOME/.local/share/shello/hooks/sweep.sh" "$__shello_dir" 2>/dev/null

	__shello_flags=-i
	__shello_is_login && __shello_flags="-l -i"
	# Closing the window sends SIGHUP; trapping it lets the cleanup run.
	trap : HUP
	SHELLO_SESSION=$__shello_id SHELLO_DIR=$__shello_dir SHELLO_PENDING=1 \
		script -q -e -f -O "$__shello_dir/$__shello_id.log" -c "exec ${__shello_sh:-sh} $__shello_flags"
	__shello_status=$?
	trap - HUP

	__shello_started=no
	[ -f "$__shello_dir/$__shello_id.started" ] && __shello_started=yes
	rm -f "$__shello_dir/$__shello_id".log "$__shello_dir/$__shello_id".cmds "$__shello_dir/$__shello_id".started
	[ "$__shello_started" = yes ] && exit "$__shello_status"

	echo "shello: recording failed to start; continuing without it" >&2
}

__shello_wrap
unset -f __shello_wanted __shello_is_login __shello_wrap __shello_record
unset __shello_tty __shello_dir __shello_id __shello_flags __shello_status __shello_started
