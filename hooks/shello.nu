# shello: installed as ~/.config/nushell/autoload/shello.nu. Safe to source again.
#
# Re-runs this interactive shell inside util-linux `script`, which records
# everything the terminal shows, then marks where each command's output
# starts and ends and appends the command's text, directory and exit code to
# the session's .cmds file. If recording can't start, nu carries on as normal.

# True when nu was started with a command (-c), not as a session.
def __shello_has_command_arg [] {
    open --raw $"/proc/($nu.pid)/cmdline"
    | split row (char nul)
    | skip 1
    | any {|arg| $arg =~ '^(-[a-zA-Z]*c[a-zA-Z]*|--commands?(=.*)?)$' }
}

def __shello_wanted [] {
    if not $nu.is-interactive { return false }
    if ($env.SHELLO_DISABLE? | is-not-empty) { return false }
    if not ((is-terminal --stdin) and (is-terminal --stdout)) { return false }
    if (__shello_has_command_arg) { return false }
    # The Linux console, where login profiles often start a desktop session.
    ($env.TERM? | default "") != "linux"
}

def __shello_tty [] {
    let res = (^tty | complete)
    if $res.exit_code == 0 { $res.stdout | str trim } else { "" }
}

# Runs nu again inside `script`. When that session ends, its files are
# deleted (they're only useful while the terminal is open) and this shell
# exits with the same status.
def __shello_record [] {
    let state = ($env.XDG_STATE_HOME? | default $"($env.HOME)/.local/state")
    let dir = $"($state)/shello/sessions"
    let id = $"(date now | format date '%Y%m%d-%H%M%S')-($nu.pid)"
    let made = (^sh -c 'umask 077 && mkdir -p "$1" && : > "$1/$2.log" && : > "$1/$2.cmds"' sh $dir $id | complete)
    if $made.exit_code != 0 { return }
    ^sh $"($env.HOME)/.local/share/shello/hooks/sweep.sh" $dir | complete | ignore

    let flags = if $nu.is-login { "-l -i" } else { "-i" }
    ^env $"SHELLO_SESSION=($id)" $"SHELLO_DIR=($dir)" SHELLO_PENDING=1 script -q -e -f -O $"($dir)/($id).log" -c $"exec '($nu.current-exe)' ($flags)"
    let st = $env.LAST_EXIT_CODE

    let started = ($"($dir)/($id).started" | path exists)
    ^rm -f $"($dir)/($id).log" $"($dir)/($id).cmds" $"($dir)/($id).started"
    if $started { exit $st }

    print -e "shello: recording failed to start; continuing without it"
}

def --env __shello_wrap [] {
    if not (__shello_wanted) { return }
    let tty = (__shello_tty)

    if ($env.SHELLO_PENDING? | is-not-empty) and ($env.SHELLO_SESSION? | is-not-empty) {
        # Inside the recording: the first shell on the new pty claims the session.
        hide-env SHELLO_PENDING
        $env.SHELLO_TTY = $tty
        ^sh -c 'umask 077 && : > "$1"' sh $"($env.SHELLO_DIR)/($env.SHELLO_SESSION).started"
        return
    }

    # This terminal is already recorded (e.g. a subshell).
    if ($env.SHELLO_SESSION? | is-not-empty) and ($env.SHELLO_TTY? == $tty) { return }

    if (which script | is-not-empty) { __shello_record }
}

def --env __shello_preexec [] {
    let cmd = (commandline)
    if ($cmd | str trim | is-empty) { return }

    # Space-prefixed commands are kept private, as in the other shells.
    let hidden = ($cmd | str starts-with ' ')
    $env.__shello = ($env.__shello | merge {
        ran: true
        cwd: $env.PWD
        cmd: (if $hidden { "" } else { $cmd })
        hidden: $hidden
    })

    print -n $"\u{1b}]6973;start;($env.__shello.id).($env.__shello.n)\u{7}"
}

def --env __shello_postexec [] {
    let ec = $env.LAST_EXIT_CODE
    let s = $env.__shello
    if not $s.ran { return }

    let seq = $"($s.id).($s.n)"
    {seq: $seq, cmd: $s.cmd, cwd: $s.cwd, exit: $ec, hidden: $s.hidden, ts: ((date now | into int) // 1_000_000_000)}
    | to json --raw
    | $"($in)\n"
    | save --append $"($env.SHELLO_DIR)/($env.SHELLO_SESSION).cmds"
    print -n $"\u{1b}]6973;end;($seq);($ec)\u{7}"

    $env.__shello = ($s | merge {ran: false, n: ($s.n + 1)})
}

# Adds a hook by name, replacing an earlier copy so re-sourcing is harmless.
def __shello_add_hook [current: any, name: string] {
    let kept = match ($current | describe | str replace -r '<.*' '') {
        "nothing" => []
        "list" => $current
        _ => [$current]
    }
    $kept | where {|h| $h != $name } | append $name
}

def --env __shello_hooks [] {
    if ($env.SHELLO_SESSION? | is-empty) or ($env.SHELLO_TTY? != (__shello_tty)) or not ($env.SHELLO_DIR? | default "" | path exists) {
        return
    }

    # A fresh id per shell start, so `exec nu` never reuses a seq from the
    # shell it replaced.
    $env.__shello = {id: $"($nu.pid)-(random int)", n: 1, ran: false, cwd: "", cmd: "", hidden: false}

    $env.config.hooks.pre_execution = (__shello_add_hook ($env.config.hooks?.pre_execution? | default null) "__shello_preexec")
    $env.config.hooks.pre_prompt = (__shello_add_hook ($env.config.hooks?.pre_prompt? | default null) "__shello_postexec")
}

__shello_wrap
__shello_hooks
