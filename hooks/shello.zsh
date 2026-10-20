# shello: sourced at the end of ~/.zshrc.
#
# Marks where each command's output starts and ends in the terminal log, and
# appends the command's text and exit code to the session's .cmds file.

if [[ -n ${SHELLO_SESSION:-} && -z ${__shello_loaded:-} && ${SHELLO_TTY:-} == "$(tty 2>/dev/null)" ]]; then
	typeset -g __shello_loaded=1 __shello_n=1 __shello_cmd= __shello_ran=

	__shello_json() {
		local s=$1
		s=${s//\\/\\\\}
		s=${s//\"/\\\"}
		s=${s//$'\n'/\\n}
		s=${s//$'\t'/\\t}
		s=${s//$'\r'/\\r}
		s=${s//[[:cntrl:]]/}
		print -rn -- "$s"
	}

	__shello_preexec() {
		# Respect HIST_IGNORE_SPACE: space-prefixed commands stay private.
		[[ -o histignorespace && $1 == ' '* ]] && return
		__shello_cmd=$1
		__shello_ran=1
		printf '\e]6973;start;%s\a' "$$.$__shello_n"
	}

	__shello_precmd() {
		local ec=$?
		if [[ -n $__shello_ran ]]; then
			printf '{"seq":"%s","cmd":"%s","exit":%d,"ts":%s}\n' \
				"$$.$__shello_n" "$(__shello_json "$__shello_cmd")" "$ec" "${EPOCHSECONDS:-$(date +%s)}" \
				>> "$SHELLO_DIR/$SHELLO_SESSION.cmds"
			printf '\e]6973;end;%s;%d\a' "$$.$__shello_n" "$ec"
			(( __shello_n++ ))
			__shello_ran=
		fi
		return $ec
	}

	zmodload -F zsh/datetime p:EPOCHSECONDS 2>/dev/null
	autoload -Uz add-zsh-hook
	add-zsh-hook preexec __shello_preexec
	# Run first, so $? is still the command's exit status.
	precmd_functions=(__shello_precmd ${precmd_functions:#__shello_precmd})
fi
