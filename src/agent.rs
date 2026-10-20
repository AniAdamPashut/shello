use std::io::{self, Write};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Command, ExitStatus, Stdio};

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

/// Runs the agent once and returns its exit code (127 if it couldn't start).
pub fn run_print(argv: &[String], prompt: &str, use_stdin: bool, stdout: Stdio) -> i32 {
    match spawn_and_wait(argv, prompt, use_stdin, stdout) {
        Ok(status) => exit_code(status),
        Err(e) => {
            eprintln!("shello: {}: {e}", argv[0]);
            127
        }
    }
}

fn spawn_and_wait(
    argv: &[String],
    prompt: &str,
    use_stdin: bool,
    stdout: Stdio,
) -> io::Result<ExitStatus> {
    let stdin = if use_stdin {
        Stdio::piped()
    } else {
        Stdio::inherit()
    };

    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(stdin)
        .stdout(stdout)
        .spawn()?;

    // Write from a thread so a large prompt can't deadlock against the
    // agent's output; dropping the pipe sends EOF.
    if let Some(mut pipe) = child.stdin.take() {
        let prompt = prompt.to_owned();
        std::thread::spawn(move || pipe.write_all(prompt.as_bytes()));
    }

    child.wait()
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
    use crate::test_util::temp_dir;
    use std::fs::File;

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// Runs run_print with stdout sent to a file, returning (exit code, output).
    fn run_captured(argv: &[&str], prompt: &str, use_stdin: bool) -> (i32, String) {
        let path = temp_dir().join("out");
        let code = run_print(
            &strings(argv),
            prompt,
            use_stdin,
            File::create(&path).unwrap().into(),
        );
        (code, std::fs::read_to_string(&path).unwrap())
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
