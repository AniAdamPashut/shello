# shello: sourced at the end of ~/.bashrc. Safe to source again.
#
# Marks where each command's output starts and ends in the terminal log, and
# appends the command's text, directory and exit code to the session's .cmds
# file.

if [[ -n ${SHELLO_SESSION:-} && ${SHELLO_TTY:-} == "$(tty 2>/dev/null)" && -w ${SHELLO_DIR:-/nonexistent} ]]; then
	# A fresh id per shell start (not exported), so `exec bash` never reuses
	# a seq from the shell it replaced.
	if [[ -z ${__shello_id:-} ]]; then
		__shello_id="$$-${EPOCHREALTIME//[.,]/}"
		__shello_n=1
		__shello_hist=
		__shello_last=
		__shello_ran=
		__shello_cwd=$PWD
		__shello_x=()
	fi

	__shello_json() {
		local s=$1
		s=${s//\\/\\\\}
		s=${s//\"/\\\"}
		s=${s//$'\n'/\\n}
		s=${s//$'\t'/\\t}
		s=${s//$'\r'/\\r}
		s=${s//[[:cntrl:]]/}
		printf '%s' "$s"
	}

	__shello_last_history() {
		local h
		h=$(HISTTIMEFORMAT= builtin history 1)
		[[ $h =~ ^\ *[0-9]+\*?\ +(.*)$ ]] && h=${BASH_REMATCH[1]}
		printf '%s' "$h"
	}

	__shello_may_hide() {
		[[ :$HISTCONTROL: == *:ignorespace:* || :$HISTCONTROL: == *:ignoreboth:* ]]
	}

	# Writes the record and end marker for the command that just finished.
	# When history didn't grow, the command was either an ignoredups repeat
	# (same text as the last one) or kept out of history on purpose; if bash
	# can't tell which, it's recorded as hidden so nothing private leaks.
	__shello_record() {
		local ec=$1 text hidden=false ts
		text=$(__shello_last_history)
		if [[ $HISTCMD == "$__shello_hist" ]] && { __shello_may_hide || [[ $text != "$__shello_last" ]]; }; then
			text=
			hidden=true
		fi

		printf -v ts '%(%s)T' -1
		printf '{"seq":"%s","cmd":"%s","cwd":"%s","exit":%d,"hidden":%s,"ts":%s}\n' \
			"$__shello_id.$__shello_n" "$(__shello_json "$text")" "$(__shello_json "$__shello_cwd")" \
			"$ec" "$hidden" "$ts" >> "$SHELLO_DIR/$SHELLO_SESSION.cmds"
		printf '\e]6973;end;%s;%d\a' "$__shello_id.$__shello_n" "$ec"
		[[ $hidden == true ]] || __shello_last=$text
		((__shello_n++))
	}

	__shello_precmd() {
		local ec=$?
		# PS0 sets __shello_ran; bash doesn't show PS0 for empty lines.
		if [[ -n $__shello_ran && -n $__shello_hist ]]; then
			__shello_record "$ec"
		fi

		__shello_ran=
		__shello_hist=$HISTCMD
		__shello_cwd=$PWD
		return $ec
	}

	# Runs first in PROMPT_COMMAND (so $? is still the command's) and is
	# re-added if the rc was sourced again and replaced PROMPT_COMMAND.
	__shello_install() {
		if [[ $(declare -p PROMPT_COMMAND 2>/dev/null) == "declare -a"* ]]; then
			[[ " ${PROMPT_COMMAND[*]} " == *" __shello_precmd "* ]] \
				|| PROMPT_COMMAND=(__shello_precmd "${PROMPT_COMMAND[@]}")
		elif [[ ${PROMPT_COMMAND:-} != *__shello_precmd* ]]; then
			PROMPT_COMMAND="__shello_precmd${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
		fi

		# The array subscript sets __shello_ran without forking and expands
		# to nothing; the escape is the start marker.
		[[ ${PS0:-} == *__shello_ran* ]] \
			|| PS0="${PS0:-}"'${__shello_x[__shello_ran=1]}\e]6973;start;${__shello_id}.${__shello_n}\a'
	}

	__shello_install
fi
