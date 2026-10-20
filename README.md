# shello

Ask an AI about what just happened in your terminal. `shello` sends your
last few commands, with their output and exit codes, plus your question to
an AI agent CLI (`claude` by default).

```console
$ pactl list short sinks
$ pactl list cards
$ shello how do I switch to my headphones?
```

The agent sees your actual device names and errors, so it can answer for
your setup instead of in general.

## Install

Requires Linux with util-linux `script`, and bash, zsh or fish. Building
from source needs Rust (`cargo`).

```sh
./install.sh                 # uses $SHELL; or --shell bash|zsh|fish
```

Then open a new terminal. Recording starts automatically.

To remove it:

```sh
./install.sh --uninstall     # keeps config and logs
./install.sh --uninstall --purge
```

## Usage

```
shello [OPTIONS] [QUESTION]...
  -p, --print           single answer, then exit (default: open a conversation)
      --show            print the built prompt and exit; call no agent
  -n, --commands <N>    how many past commands to include (default 3)
  -a, --agent <NAME>    which configured agent to use
  -e, --empty           send no question; the agent explains what happened
  -h, --help / -V, --version
```

- `shello why did that fail?` opens a conversation in the agent's own tool,
  with the prompt as the first message.
- `shello -p what does this error mean` prints one answer and returns. While
  it waits, a spinner shows on stderr (only when stderr is a terminal).
- `shello -e` sends just the commands; the agent explains what happened and
  whether anything went wrong.
- `shello` on its own prints this help.
- `shello --show -e` (or `--show` with a question) prints exactly what would be sent.

No quoting is needed, because everything after the options is the question.
A question is required unless you pass `-e`.
Use `--` before a question that starts with `-`.

Each command in the prompt comes with the directory it ran in, its exit
code, and its output, with colors, progress-bar redraws and long middles
stripped.

## Configuration

`~/.config/shello/config.toml` picks the agent, how much output to send, and
extra redaction patterns.
Each agent has two command templates. `{prompt}` is replaced by the prompt.
If a template has no `{prompt}`, the prompt goes on stdin:

```toml
agent = "claude"

[agents.ollama]
interactive = ["ollama", "run", "llama3", "{prompt}"]
print       = ["ollama", "run", "llama3"]
```

`~/.config/shello/prompt.md` is the prompt template. It supports the
placeholders `{commands}`, `{question}`, `{shell}`, `{cwd}` and `{os}`.

## How it works

- **Recording:** the installer adds a block at the top of your rc file that
  re-runs the shell inside `script`, which logs everything the terminal shows
  to `~/.local/state/shello/sessions/`.
- **Command boundaries:** a block at the bottom installs hooks. They mark
  where each command's output starts and ends, and record the command text,
  its directory and its exit code.
- **One recording per terminal:** each terminal (including each tmux pane)
  gets its own recording.
- **What's not recorded:** the Linux console (`TERM=linux`) and `bash -c`-style
  one-off shells. Output from full-screen programs like vim and less is left
  out of the prompt.
- **Login shells** stay login shells inside the recording, so your whole
  profile runs.
- **Turning it off:** set `SHELLO_DISABLE=1` before a shell starts to skip
  recording.

## Privacy and security

- **Nothing is sent unless you run `shello`.** Only the last few commands of
  the terminal you run it in are sent. `--show` prints exactly what would go.
- **Secrets are redacted before sending.** These are replaced with
  `[redacted]` in both the command text and the output:
  - private-key blocks
  - common token formats (AWS, GitHub, GitLab, Slack, OpenAI/Anthropic,
    Stripe, Google, JWTs)
  - `Authorization:` headers
  - passwords in URLs
  - `PASSWORD=`/`token:`/`api_key`-style values

  `shello` tells you on stderr when it redacted something. You can add your
  own patterns, or turn redaction off, under `[redact]` in `config.toml`.
  Pattern matching can't catch every secret, so check with `--show` when it
  matters.
- **Hidden commands stay hidden.** A command your shell keeps out of history
  (typed with a leading space, with bash `ignorespace`/`ignoreboth`, zsh
  `HIST_IGNORE_SPACE`, or in fish) is sent only as "(hidden command)", with
  no text or output.
- **The on-disk log is not redacted.** It holds everything the terminal
  showed, is readable only by you, and is deleted when the terminal closes.
  Leftovers from crashed or killed terminals are deleted after 7 days.
- **Interactive mode passes the prompt as a command-line argument.** Other
  local users can read it from `/proc` while the agent runs. Use `-p`
  (stdin) on shared machines.

## Development

```sh
make test                 # unit tests
make integration          # installer + end-to-end tests with bash
make integration-docker   # end-to-end with bash, zsh and fish (Arch container)
```
