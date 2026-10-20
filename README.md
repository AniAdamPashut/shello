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
shello [options] [question...]
  -p, --print        single answer, then exit (default: conversation)
  -n, --commands N   how many past commands to include (default 3)
  -a, --agent NAME   pick a configured agent
      --show         print the built prompt and exit; call no agent
```

- `shello why did that fail?` opens a conversation in the agent's own tool,
  with the prompt as the first message.
- `shello -p what does this error mean` prints one answer and returns.
- `shello` with no question asks what happened and whether anything went wrong.
- `shello --show` prints exactly what would be sent.

No quoting is needed, because everything after the options is the question.
Use `--` before a question that starts with `-`.

## Configuration

`~/.config/shello/config.toml` picks the agent and how much output to send.
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
  where each command's output starts and ends, and record the command text
  and exit code.
- **Exceptions:** each terminal (including each tmux pane) gets its own
  recording. Output from full-screen programs like vim and less is left out.
- **Cleanup:** logs are readable only by you and are deleted after 7 days.
- **Turning it off:** set `SHELLO_DISABLE=1` before a shell starts to skip
  recording. bash and zsh commands typed with a leading space (when your
  history ignores those) are not recorded.

**Privacy:** the output of the commands you include is sent to the agent.
Nothing is sent unless you run `shello`. Check with `--show` first if the
output might contain secrets.

## Development

```sh
make test                 # unit tests
make integration          # installer + end-to-end tests with bash
make integration-docker   # end-to-end with bash, zsh and fish (Arch container)
```
