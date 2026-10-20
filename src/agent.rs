use std::io::{self, Read, Write};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;

const MAX_INTERACTIVE_PROMPT: usize = 120_000;
const PLACEHOLDER: &str = "{prompt}";

/// Fills {prompt} into the template. When no argument has the placeholder,
/// the prompt goes on stdin instead (the second value is true).
pub fn build_argv(tmpl: &[String], prompt: &str) -> (Vec<String>, bool) {
    let argv = tmpl
        .iter()
        .map(|a| a.replace(PLACEHOLDER, prompt))
        .collect();
    let use_stdin = !tmpl.iter().any(|a| a.contains(PLACEHOLDER));

    (argv, use_stdin)
}

/// Where a print-mode run's output goes. on_output runs before each chunk
/// is written, so a spinner can clear its line first.
pub struct Sinks<'a> {
    pub stdout: &'a mut (dyn Write + Send),
    pub stderr: &'a mut (dyn Write + Send),
    pub on_output: &'a (dyn Fn() + Sync),
}

/// Runs the agent once, relaying its output, and returns its exit code
/// (127 if it couldn't start).
pub fn run_print(argv: &[String], prompt: &str, use_stdin: bool, mut sinks: Sinks) -> i32 {
    match spawn_and_relay(argv, prompt, use_stdin, &mut sinks) {
        Ok(status) => exit_code(status),
        Err(e) => {
            (sinks.on_output)();
            let _ = writeln!(sinks.stderr, "shello: {}: {e}", argv[0]);
            127
        }
    }
}

fn spawn_and_relay(
    argv: &[String],
    prompt: &str,
    use_stdin: bool,
    sinks: &mut Sinks,
) -> io::Result<ExitStatus> {
    let mut child = spawn(argv, use_stdin)?;
    feed_stdin(&mut child, prompt);

    let agent_out = child.stdout.take().expect("stdout is piped");
    let agent_err = child.stderr.take().expect("stderr is piped");
    let on_output = sinks.on_output;
    let (out, err) = (&mut *sinks.stdout, &mut *sinks.stderr);
    thread::scope(|scope| {
        scope.spawn(move || relay(agent_err, err, on_output));
        let _ = relay(agent_out, out, on_output);
    });

    child.wait()
}

fn spawn(argv: &[String], use_stdin: bool) -> io::Result<Child> {
    let stdin = if use_stdin {
        Stdio::piped()
    } else {
        Stdio::inherit()
    };

    Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

/// Writes the prompt from a thread so a large prompt can't deadlock against
/// the agent's output; dropping the pipe sends EOF.
fn feed_stdin(child: &mut Child, prompt: &str) {
    if let Some(mut pipe) = child.stdin.take() {
        let prompt = prompt.to_owned();
        thread::spawn(move || pipe.write_all(prompt.as_bytes()));
    }
}

/// Copies from the agent to a sink as output arrives.
fn relay(mut from: impl Read, to: &mut dyn Write, on_output: &dyn Fn()) -> io::Result<()> {
    let mut buf = [0u8; 8192];
    loop {
        let n = from.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }

        on_output();
        to.write_all(&buf[..n])?;
        to.flush()?;
    }
}

/// The exit code, or 128 + signal number like a shell reports it.
fn exit_code(status: ExitStatus) -> i32 {
    let by_signal = status.signal().map(|s| 128 + s);

    status.code().or(by_signal).unwrap_or(1)
}

/// Replaces this process with the agent, so it owns the terminal exactly as
/// if the user had started it. Returns only on failure.
pub fn exec_interactive(argv: &[String]) -> io::Error {
    Command::new(&argv[0]).args(&argv[1..]).exec()
}

pub fn too_large_for_interactive(prompt: &str) -> bool {
    prompt.len() > MAX_INTERACTIVE_PROMPT
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    struct Captured {
        code: i32,
        stdout: String,
        stderr: String,
        output_calls: usize,
    }

    /// Runs run_print with both streams captured.
    fn capture(argv: &[&str], prompt: &str, use_stdin: bool) -> Captured {
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        let calls = AtomicUsize::new(0);
        let on_output = || {
            calls.fetch_add(1, Ordering::SeqCst);
        };
        let sinks = Sinks {
            stdout: &mut stdout,
            stderr: &mut stderr,
            on_output: &on_output,
        };
        let code = run_print(&strings(argv), prompt, use_stdin, sinks);

        Captured {
            code,
            stdout: String::from_utf8(stdout).unwrap(),
            stderr: String::from_utf8(stderr).unwrap(),
            output_calls: calls.load(Ordering::SeqCst),
        }
    }

    fn run_captured(argv: &[&str], prompt: &str, use_stdin: bool) -> (i32, String) {
        let c = capture(argv, prompt, use_stdin);
        (c.code, c.stdout)
    }

    #[test]
    fn reports_when_output_starts() {
        assert!(capture(&["cat"], "hello", true).output_calls > 0);
        assert_eq!(capture(&["true"], "", true).output_calls, 0);
    }

    #[test]
    fn forwards_agent_stderr() {
        let c = capture(&["sh", "-c", "echo oops >&2; exit 4"], "", true);

        assert_eq!((c.code, c.stderr.as_str()), (4, "oops\n"));
        assert!(c.output_calls > 0);
    }

    #[test]
    fn fills_prompt_placeholder() {
        let cases: &[(&[&str], &[&str], bool)] = &[
            (&["claude", "{prompt}"], &["claude", "hi"], false),
            (&["claude", "-p"], &["claude", "-p"], true),
            (&["x", "--msg={prompt}"], &["x", "--msg=hi"], false),
        ];

        for (tmpl, want, stdin) in cases {
            assert_eq!(build_argv(&strings(tmpl), "hi"), (strings(want), *stdin));
        }
    }

    #[test]
    fn pipes_prompt_on_stdin() {
        assert_eq!(
            run_captured(&["cat"], "hello", true),
            (0, "hello".to_string())
        );
    }

    #[test]
    fn returns_agent_exit_code() {
        assert_eq!(run_captured(&["sh", "-c", "exit 3"], "", true).0, 3);
    }

    #[test]
    fn missing_agent_is_127() {
        assert_eq!(run_captured(&["/nonexistent/agent"], "", true).0, 127);
    }

    #[test]
    fn large_prompts_are_refused_interactively() {
        assert!(too_large_for_interactive(&"x".repeat(120_001)));
        assert!(!too_large_for_interactive("small"));
    }
}
