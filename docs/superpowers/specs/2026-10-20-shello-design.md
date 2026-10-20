# shello — design

## Intent

When you're stuck at the terminal, you've usually just run a few commands
whose output says exactly what your system looks like. The motivating case
was trying to switch to headphones with `pactl`: you run `pactl list sinks`
and `pactl list cards`, get walls of output, and want to ask an AI "how do I
switch to my headphones?" without copy-pasting all of it.

`shello` is a CLI that takes the **last N (default 3) commands you ran in this
terminal, with their output and exit codes**, wraps them in a specialized
prompt along with your question (argv), and hands that to an AI agent CLI.
The agent defaults to the local `claude`.

```
$ pactl list short sinks
$ pactl list cards
$ shello how do I switch to my headphones?
```

## Goals

- Capture command + output + exit code for every command in an interactive
  terminal, transparently. Colors, pagers, and TTY detection must keep working.
- One-command install (`./install.sh`) and a clean uninstall
  (`./install.sh --uninstall`).
- Shell-agnostic capture. Ship hooks for **bash, zsh, and fish**. bash is the
  user's shell and the one that must work. zsh and fish are best-effort, since
  neither is installed on the dev machine.
- Agent-agnostic. The agent is configured as command templates, with `claude`
  as the default.
- Two modes:
  - **Conversation (default):** start the agent's own interactive tool with
    the built prompt as the first message, and the user continues there.
  - **Single answer (`-p` / `--print`):** pipe the prompt into the agent's
    non-interactive mode, print the answer, and exit.

## Non-goals

- macOS / BSD (their `script` has different flags). Linux with util-linux
  `script` only, for now.
- Pulling commands from other terminals. Only the current terminal session counts.
- Shells other than bash, zsh, and fish.
- Redacting secrets automatically.

## Anti-goals (failure even if it "works")

- Anything that noticeably slows the prompt or breaks normal terminal behavior
  (colors, `less`, `vim`, ssh, ctrl-c, job control, closing the terminal).
- Sending anything to an AI without the user explicitly running `shello`.
- Recursion: nested `script` sessions when opening a subshell or tmux pane
  inside an already-recorded terminal.

## Decided how

### Language and layout

- **Go**, compiled to a single static binary (`CGO_ENABLED=0`), installed to
  `~/.local/bin/shello`. The user asked for a simple executable.
- Standard library plus one small TOML library (`github.com/BurntSushi/toml`).
  No other dependencies.
- Interactive mode uses `syscall.Exec` so the agent replaces the process.
- Tests use stdlib `testing`.
- Repo layout (builder may adjust):
  ```
  go.mod
  main.go, ...            # the CLI (package main, split into files as needed)
  Makefile                # `make` builds ./shello, `make test`
  hooks/shello.bash
  hooks/shello.zsh
  hooks/shello.fish
  prompt.md               # default specialized prompt template
  config.toml             # default config
  install.sh              # POSIX sh
  tests/
  ```

### Capture

1. **Recording.** The rc snippet runs only in an interactive shell on a TTY.
   It wraps the shell unless **this TTY** is already recorded. A session is
   identified by `SHELLO_SESSION=<id>` together with `SHELLO_TTY=<tty
   device>`. If `SHELLO_SESSION` is unset, or `SHELLO_TTY` differs from the
   current `tty`, the snippet:
   - generates a new id
   - exports both variables
   - `exec`s `script -q -f -O ~/.local/state/shello/sessions/<id>.log`,
     which runs the current shell interactively inside it

   *(My call, after the review.)* Comparing the TTY means a tmux/screen pane,
   or a new terminal window launched from a recorded shell, gets its own
   recording instead of writing commands into someone else's session. That
   isn't the nesting the anti-goal forbids, because each recording covers a
   different terminal. A subshell on the same TTY isn't re-wrapped.
   - Because of `exec`, closing the shell closes the terminal as before.
   - If `script` is missing or fails to start, the snippet must fall through
     to a normal shell. A broken install can never lock the user out.
   - Setting `SHELLO_DISABLE=1` skips everything.
   - Session files are created with mode 0600 inside a 0700 directory.
   - **rc placement (my call, after the review):** the installer writes two
     marked blocks.
     - The wrapper goes at the **top** of the rc file, so the outer shell
       `exec`s `script` before running the rest of the rc. The rc then runs
       only once, in the inner shell, with no doubled startup time or
       repeated banners.
     - The hooks go at the **bottom**, after other tools have set
       `PROMPT_COMMAND`/`PS0`.

     For fish, both go in `conf.d/shello.fish`, which is fine because it
     loads early.
2. **Markers.** Per-shell hooks write invisible OSC escape sequences to the
   terminal, so they land in the `script` log in the right order relative to
   the output:
   - before a command runs: `ESC ] 6973 ; start ; <seq> BEL`
   - after it finishes, before the next prompt: `ESC ] 6973 ; end ; <seq> ; <exit> BEL`

   Terminals ignore unknown OSC sequences.
3. **Command text.** The after-command hook appends one JSON line to
   `~/.local/state/shello/sessions/<id>.cmds`: `{"seq", "cmd", "exit", "ts"}`.
   `seq` is a per-session counter incremented per executed command, and it
   matches the markers. Empty prompt lines (Enter with no command) record nothing.
   - bash: start marker via `PS0`, end marker and command text via
     `PROMPT_COMMAND`, which reads the last history entry. Prepend to existing
     `PROMPT_COMMAND`/`PS0` values, don't clobber them. A known caveat is
     commands that `HISTCONTROL=ignorespace` drops. Detecting "no new history
     entry" and skipping is acceptable.
   - zsh: `preexec` / `precmd` via `add-zsh-hook`.
   - fish: `fish_preexec` / `fish_postexec` events.
4. **Cleanup.** At session start, delete session files older than 7 days.
   `shello` reads only the tail of the log (the last ~2 MB, configurable)
   so long sessions stay fast.

### Extracting the last N commands

- Read the `.cmds` file and take the last N **completed** entries. Exclude
  `shello` invocations themselves, matching on the command's first word, so
  `shello` runs never count toward N. The currently running `shello` has no
  end marker and is ignored anyway.
- For each entry, the output is the log bytes between its start and end markers.
- Clean the output:
  - strip ANSI/OSC sequences
  - apply `\r` and backspace overwrites so progress bars collapse to their
    final state
  - if the output entered the alternate screen (`ESC[?1049h`, used by vim,
    less, htop, and the like), replace it with
    `[full-screen program output omitted]`
- Truncate each output to the first 100 and last 100 lines (configurable),
  with a `[... N lines omitted ...]` note.
- If `SHELLO_SESSION` is unset or the files are missing, print a clear message
  ("shello isn't recording this terminal — run install.sh and open a new
  terminal") and exit non-zero.

### Prompt

- The template lives in `~/.config/shello/prompt.md`, copied from the repo's
  `prompt.md` on install if missing, and the user can edit it. Placeholders:
  `{commands}`, `{question}`, `{shell}`, `{cwd}`, `{os}`.
- The intent of the default text: "You are helping someone at their Linux
  terminal. Below are the last N commands they ran, with exit codes and
  output. Treat them as evidence about their actual system and setup. Answer
  their question concretely for *their* setup, giving exact commands. If
  something in the output looks like the cause of a problem, say so."
- Each command block shows `$ <cmd>`, `exit <code>`, and the cleaned output
  in a fenced block.
- With no question (`shello` with no args), the question becomes "Explain what
  happened in these commands and whether anything went wrong."

### Agent backend

`~/.config/shello/config.toml`:

```toml
agent = "claude"

[agents.claude]
interactive = ["claude", "{prompt}"]   # prompt passed as an argument
print       = ["claude", "-p"]         # prompt passed on stdin

[agents.codex]
interactive = ["codex", "{prompt}"]
print       = ["codex", "exec", "-"]

[agents.gemini]
interactive = ["gemini", "-i", "{prompt}"]
print       = ["gemini", "-p"]

[capture]
commands = 3
head_lines = 100
tail_lines = 100
```

- The templates are argv lists, and no shell is involved. `{prompt}` is
  substituted in an argv element. If the template has no `{prompt}`, the
  prompt goes on stdin.
- Interactive mode `exec`s the agent (`os.execvp`) so it owns the terminal fully.
- The default config is written on install only if one doesn't already exist.

### CLI

```
shello [options] [question...]
  -p, --print        single answer, then exit (default: conversation)
  -n, --commands N   how many past commands to include (default 3)
  -a, --agent NAME   pick a configured agent
      --show         print the built prompt and exit; call no agent
```

The question is the remaining argv joined with spaces, so quoting isn't needed.

### Installer (`install.sh`, POSIX sh)

- Checks for util-linux `script`, and fails with a clear
  message otherwise. Uses a `shello` binary if one is next to the script
  (a prebuilt release), otherwise builds it with `go build` (fails clearly
  if Go is missing).
- Copies the `shello` binary to `~/.local/bin/` and the hooks to `~/.local/share/shello/hooks/`.
  Writes the config and prompt if they're missing.
- Detects the shell from `$SHELL`, or `--shell bash|zsh|fish` to override.
  Adds the two marked blocks (wrapper at top, hooks at bottom; see Capture) to the right rc file
  (`~/.bashrc`, `~/.zshrc`, `~/.config/fish/conf.d/shello.fish`).
- Idempotent: running it twice doesn't duplicate either block.
- `--uninstall` removes the files and both marked rc blocks, and leaves logs
  and config alone unless `--purge` is passed.
- Warns if `~/.local/bin` isn't on `PATH`. Tells the user to open a new terminal.

## Left to the builder

- Session id format, exact JSON escaping in shell hooks, and how bash gets the
  last history entry robustly.
- Internal package/file structure of the Go code and the test fixtures. Build a sample
  `script` log by hand or by recording a real one.
- Exact wording of the default prompt, within the intent above.
- Whether `--show` also prints which agent command would run.
- Prompts larger than the per-argument limit (~128 KB) in interactive mode.
  Truncation normally keeps prompts far below this, but the builder picks
  the fallback.
