# shello: installed as ~/.config/fish/conf.d/shello.fish. Safe to source again.
#
# Re-runs this interactive shell inside util-linux `script`, which records
# everything the terminal shows, then marks where each command's output
# starts and ends and appends the command's text, directory and exit code to
# the session's .cmds file. If recording can't start, fish carries on as normal.

# True when fish was started with a command (-c), not as a session.
function __shello_has_command_arg
    for arg in (string split0 < /proc/$fish_pid/cmdline)[2..-1]
        string match -qr -- '^(-[a-zA-Z]*c[a-zA-Z]*|--command(=.*)?)$' $arg; and return 0
    end
    return 1
end

function __shello_wanted
    status is-interactive; or return 1
    not set -q SHELLO_DISABLE; or return 1
    isatty stdin; and isatty stdout; or return 1
    not __shello_has_command_arg; or return 1
    # The Linux console, where login profiles often start a desktop session.
    test "$TERM" != linux
end

# Runs fish again inside `script`. When that session ends, its files are
# deleted (they're only useful while the terminal is open) and this shell
# exits with the same status.
function __shello_record
    set -l state $HOME/.local/state
    set -q XDG_STATE_HOME; and set state $XDG_STATE_HOME
    set -l dir $state/shello/sessions
    set -l id (date +%Y%m%d-%H%M%S)-$fish_pid
    command sh -c 'umask 077 && mkdir -p "$1" && : > "$1/$2.log" && : > "$1/$2.cmds"' sh $dir $id 2>/dev/null
    or return
    sh $HOME/.local/share/shello/hooks/sweep.sh $dir 2>/dev/null

    set -l flags -i
    status is-login; and set flags -l -i
    SHELLO_SESSION=$id SHELLO_DIR=$dir SHELLO_PENDING=1 \
        script -q -e -f -O $dir/$id.log -c "exec fish $flags"
    set -l st $status

    set -l started no
    test -f $dir/$id.started; and set started yes
    rm -f $dir/$id.log $dir/$id.cmds $dir/$id.started
    # `exit` in a config file only stops sourcing it, so replace this shell
    # with one that exits with the recorded shell's status.
    test $started = yes; and exec sh -c "exit $st"

    echo "shello: recording failed to start; continuing without it" >&2
end

function __shello_wrap
    __shello_wanted; or return
    set -l tty (tty 2>/dev/null)

    if set -q SHELLO_PENDING; and set -q SHELLO_SESSION
        # Inside the recording: the first shell on the new pty claims the session.
        set -e SHELLO_PENDING
        set -gx SHELLO_TTY $tty
        command sh -c 'umask 077 && : > "$1"' sh "$SHELLO_DIR/$SHELLO_SESSION.started"
        return
    end

    # This terminal is already recorded (e.g. a subshell).
    if set -q SHELLO_SESSION; and test "$SHELLO_TTY" = "$tty"
        return
    end

    command -q script; and __shello_record
end

function __shello_json
    set -l s (string replace -a '\\' '\\\\' -- $argv[1] | string replace -a '"' '\\"' \
        | string replace -a \t '\\t' | string replace -a \r '\\r')
    string join '\n' -- $s | string replace -ra '[\x00-\x1f]' ''
end

function __shello_hooks
    set -q SHELLO_SESSION; and test "$SHELLO_TTY" = (tty 2>/dev/null); and test -w "$SHELLO_DIR"
    or return

    # A fresh id per shell start (not exported), so `exec fish` never reuses
    # a seq from the shell it replaced.
    if not set -q __shello_id
        set -g __shello_id $fish_pid-(random)(random)
        set -g __shello_n 1
        set -g __shello_ran
    end

    function __shello_preexec --on-event fish_preexec
        string match -qr '\S' -- $argv[1]; or return
        set -g __shello_ran 1
        set -g __shello_cwd $PWD
        set -g __shello_cmd $argv[1]
        set -g __shello_hidden false
        # fish keeps space-prefixed commands out of history; keep them private.
        if string match -q ' *' -- $argv[1]
            set -g __shello_cmd ''
            set -g __shello_hidden true
        end

        printf '\e]6973;start;%s\a' "$__shello_id.$__shello_n"
    end

    function __shello_postexec --on-event fish_postexec
        set -l ec $status
        test -n "$__shello_ran"; or return
        set -g __shello_ran
        printf '{"seq":"%s","cmd":"%s","cwd":"%s","exit":%d,"hidden":%s,"ts":%s}\n' \
            "$__shello_id.$__shello_n" (__shello_json "$__shello_cmd") (__shello_json "$__shello_cwd") \
            $ec $__shello_hidden (date +%s) >> $SHELLO_DIR/$SHELLO_SESSION.cmds
        printf '\e]6973;end;%s;%d\a' "$__shello_id.$__shello_n" $ec
        set -g __shello_n (math $__shello_n + 1)
    end
end

__shello_wrap
__shello_hooks
