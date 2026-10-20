# shello: sourced at the top of ~/.bashrc or ~/.zshrc (set __shello_sh first).
#
# Re-runs this interactive shell inside util-linux `script`, which records
# everything the terminal shows to a log that shello reads. If recording
# can't start, the shell carries on as normal.

__shello_wrap() {
	case $- in *i*) ;; *) return 0 ;; esac
	[ -n "${SHELLO_DISABLE:-}" ] && return 0
	[ -t 0 ] && [ -t 1 ] || return 0
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
	__shello_dir=${XDG_STATE_HOME:-$HOME/.local/state}/shello/sessions
	__shello_id=$(date +%Y%m%d-%H%M%S)-$$
	(umask 077 && mkdir -p "$__shello_dir" \
		&& : > "$__shello_dir/$__shello_id.log" \
		&& : > "$__shello_dir/$__shello_id.cmds") 2>/dev/null || return 0
	find "$__shello_dir" -type f -mtime +7 -delete 2>/dev/null

	SHELLO_SESSION=$__shello_id SHELLO_DIR=$__shello_dir SHELLO_PENDING=1 \
		script -q -e -f -O "$__shello_dir/$__shello_id.log" -c "exec ${__shello_sh:-sh} -i"
	__shello_status=$?
	if [ -f "$__shello_dir/$__shello_id.started" ]; then
		exit "$__shello_status"
	fi
	rm -f "$__shello_dir/$__shello_id.log" "$__shello_dir/$__shello_id.cmds"
	echo "shello: recording failed to start; continuing without it" >&2
}

__shello_wrap
unset -f __shello_wrap
unset __shello_tty __shello_dir __shello_id __shello_status
