# shello: sourced at the end of ~/.zshrc. Safe to source again.
#
# Marks where each command's output starts and ends in the terminal log, and
# appends the command's text, directory and exit code to the session's .cmds
# file.

if [[ -n ${SHELLO_SESSION:-} && ${SHELLO_TTY:-} == "$(tty 2>/dev/null)" && -w ${SHELLO_DIR:-/nonexistent} ]]; then
	zmodload -F zsh/datetime p:EPOCHSECONDS p:EPOCHREALTIME 2>/dev/null

	# A fresh id per shell start (not exported), so `exec zsh` never reuses
	# a seq from the shell it replaced.
	if [[ -z ${__shello_id:-} ]]; then
		typeset -g __shello_id="$$-${EPOCHREALTIME//./}" __shello_n=1
		typeset -g __shello_cmd= __shello_cwd= __shello_hidden=false __shello_ran=
	fi

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
		__shello_cmd=$1
		__shello_cwd=$PWD
		__shello_hidden=false
		__shello_ran=1
		# With HIST_IGNORE_SPACE, space-prefixed commands stay private.
		if [[ -o histignorespace && $1 == ' '* ]]; then
			__shello_cmd=
			__shello_hidden=true
		fi

		printf '\e]6973;start;%s\a' "$__shello_id.$__shello_n"
	}

	__shello_precmd() {
		local ec=$?
		if [[ -n $__shello_ran ]]; then
			__shello_record $ec
		fi

		__shello_ran=
		return $ec
	}

	__shello_record() {
		printf '{"seq":"%s","cmd":"%s","cwd":"%s","exit":%d,"hidden":%s,"ts":%s}\n' \
			"$__shello_id.$__shello_n" "$(__shello_json "$__shello_cmd")" "$(__shello_json "$__shello_cwd")" \
			"$1" "$__shello_hidden" "${EPOCHSECONDS:-0}" >> "$SHELLO_DIR/$SHELLO_SESSION.cmds"
		printf '\e]6973;end;%s;%d\a' "$__shello_id.$__shello_n" "$1"
		(( __shello_n++ ))
	}

	autoload -Uz add-zsh-hook
	add-zsh-hook preexec __shello_preexec
	# First in line, so $? is still the command's exit status.
	precmd_functions=(__shello_precmd ${precmd_functions:#__shello_precmd})
fi
