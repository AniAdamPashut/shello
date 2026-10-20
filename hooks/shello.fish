# shello: installed as ~/.config/fish/conf.d/shello.fish.
#
# Re-runs this interactive shell inside util-linux `script`, which records
# everything the terminal shows, then marks where each command's output
# starts and ends and appends the command's text and exit code to the
# session's .cmds file. If recording can't start, fish carries on as normal.

if status is-interactive; and not set -q SHELLO_DISABLE; and isatty stdin; and isatty stdout
    set -l __shello_tty (tty 2>/dev/null)

    if set -q SHELLO_PENDING; and set -q SHELLO_SESSION
        # Inside the recording: the first shell on the new pty claims the session.
        set -e SHELLO_PENDING
        set -gx SHELLO_TTY $__shello_tty
        command sh -c 'umask 077 && : > "$1"' sh "$SHELLO_DIR/$SHELLO_SESSION.started"
    else if set -q SHELLO_SESSION; and test "$SHELLO_TTY" = "$__shello_tty"
        # This terminal is already recorded (e.g. a subshell).
    else if command -q script
        set -l state $HOME/.local/state
        set -q XDG_STATE_HOME; and set state $XDG_STATE_HOME
        set -l dir $state/shello/sessions
        set -l id (date +%Y%m%d-%H%M%S)-$fish_pid
        if command sh -c 'umask 077 && mkdir -p "$1" && : > "$1/$2.log" && : > "$1/$2.cmds"' sh $dir $id 2>/dev/null
            find $dir -type f -mtime +7 -delete 2>/dev/null
            SHELLO_SESSION=$id SHELLO_DIR=$dir SHELLO_PENDING=1 \
                script -q -e -f -O $dir/$id.log -c "exec fish -i"
            set -l st $status
            if test -f $dir/$id.started
                exit $st
            end
            rm -f $dir/$id.log $dir/$id.cmds
            echo "shello: recording failed to start; continuing without it" >&2
        end
    end

    if set -q SHELLO_SESSION; and not set -q __shello_loaded; and test "$SHELLO_TTY" = "$__shello_tty"
        set -g __shello_loaded 1
        set -g __shello_n 1
        set -g __shello_ran

        function __shello_json
            set -l s (string replace -a '\\' '\\\\' -- $argv[1] | string replace -a '"' '\\"' \
                | string replace -a \t '\\t' | string replace -a \r '\\r')
            string join '\n' -- $s | string replace -ra '[\x00-\x1f]' ''
        end

        function __shello_preexec --on-event fish_preexec
            string match -qr '\S' -- $argv[1]; or return
            set -g __shello_ran 1
            printf '\e]6973;start;%s\a' "$fish_pid.$__shello_n"
        end

        function __shello_postexec --on-event fish_postexec
            set -l ec $status
            test -n "$__shello_ran"; or return
            set -g __shello_ran
            printf '{"seq":"%s","cmd":"%s","exit":%d,"ts":%s}\n' \
                "$fish_pid.$__shello_n" (__shello_json "$argv[1]") $ec (date +%s) \
                >> $SHELLO_DIR/$SHELLO_SESSION.cmds
            printf '\e]6973;end;%s;%d\a' "$fish_pid.$__shello_n" $ec
            set -g __shello_n (math $__shello_n + 1)
        end
    end
end
