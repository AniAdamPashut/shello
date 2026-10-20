# shello: sourced at the end of ~/.bashrc.
#
# Marks where each command's output starts and ends in the terminal log, and
# appends the command's text and exit code to the session's .cmds file.

if [[ -n ${SHELLO_SESSION:-} && -z ${__shello_loaded:-} && ${SHELLO_TTY:-} == "$(tty 2>/dev/null)" ]]; then
	__shello_loaded=1
	__shello_n=1
	__shello_hist=

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

	__shello_precmd() {
		local ec=$? h ts
		# HISTCMD only advances when a command was added to history, so empty
		# lines and space-prefixed (ignorespace) commands are never recorded.
		if [[ -n $__shello_hist && $HISTCMD != "$__shello_hist" ]]; then
			h=$(HISTTIMEFORMAT= builtin history 1)
			[[ $h =~ ^\ *[0-9]+\*?\ +(.*)$ ]] && h=${BASH_REMATCH[1]}
			printf -v ts '%(%s)T' -1
			printf '{"seq":"%s","cmd":"%s","exit":%d,"ts":%s}\n' \
				"$$.$__shello_n" "$(__shello_json "$h")" "$ec" "$ts" \
				>> "$SHELLO_DIR/$SHELLO_SESSION.cmds"
			printf '\e]6973;end;%s;%d\a' "$$.$__shello_n" "$ec"
			((__shello_n++))
		fi
		__shello_hist=$HISTCMD
		return $ec
	}

	if [[ $(declare -p PROMPT_COMMAND 2>/dev/null) == "declare -a"* ]]; then
		PROMPT_COMMAND=(__shello_precmd "${PROMPT_COMMAND[@]}")
	else
		PROMPT_COMMAND="__shello_precmd${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
	fi
	PS0="${PS0:-}"'\e]6973;start;'"$$"'.${__shello_n}\a'
fi
