use crate::extract::Command;
use crate::paths::env_var;
use std::path::Path;

pub const DEFAULT_QUESTION: &str =
    "Explain what happened in these commands and whether anything went wrong.";
const NO_COMMANDS: &str = "(no commands recorded in this terminal yet)\n";

/// Facts about the user's environment that the template can mention.
#[derive(Debug, Clone, Default)]
pub struct PromptEnv {
    pub shell: String,
    pub cwd: String,
    pub os: String,
}

/// Renders each command as "$ cmd", "exit N" and a fenced output block,
/// with blank lines between commands.
pub fn format_commands(cmds: &[Command]) -> String {
    if cmds.is_empty() {
        return NO_COMMANDS.to_string();
    }

    cmds.iter()
        .map(format_command)
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_command(c: &Command) -> String {
    let needs_newline = !c.output.is_empty() && !c.output.ends_with('\n');
    let newline = if needs_newline { "\n" } else { "" };
    let cwd = if c.cwd.is_empty() {
        String::new()
    } else {
        format!("cwd: {}\n", c.cwd)
    };

    format!(
        "$ {}\n{cwd}exit {}\n```\n{}{newline}```\n",
        c.cmd, c.exit, c.output
    )
}

/// Fills the template's placeholders.
pub fn build_prompt(tmpl: &str, cmds: &[Command], question: &str, env: &PromptEnv) -> String {
    let question = if question.trim().is_empty() {
        DEFAULT_QUESTION
    } else {
        question
    };

    let commands = format_commands(cmds);
    let vars = [
        ("{commands}", commands.as_str()),
        ("{question}", question),
        ("{shell}", &env.shell),
        ("{cwd}", &env.cwd),
        ("{os}", &env.os),
    ];

    fill_placeholders(tmpl, &vars)
}

/// Replaces placeholders in a single pass, so text that a value brings in
/// (like "{question}" inside command output) is never expanded.
fn fill_placeholders(tmpl: &str, vars: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(tmpl.len());
    let mut rest = tmpl;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        let (text, len) = expand_at(&rest[i..], vars);
        out.push_str(text);
        rest = &rest[i + len..];
    }

    out.push_str(rest);
    out
}

/// The text for the placeholder at the start of rest (or a literal "{"),
/// and how many bytes of rest it consumes.
fn expand_at<'a>(rest: &str, vars: &[(&str, &'a str)]) -> (&'a str, usize) {
    match vars.iter().find(|(key, _)| rest.starts_with(key)) {
        Some((key, value)) => (value, key.len()),
        None => ("{", 1),
    }
}

pub fn detect_env() -> PromptEnv {
    PromptEnv {
        shell: shell_name(),
        cwd: std::env::current_dir()
            .map(|d| d.display().to_string())
            .unwrap_or_default(),
        os: os_name(),
    }
}

fn shell_name() -> String {
    let shell = env_var("SHELL").unwrap_or_default();
    let name = Path::new(&shell).file_name().unwrap_or_default();

    name.to_string_lossy().into_owned()
}

/// PRETTY_NAME from /etc/os-release, or "Linux".
fn os_name() -> String {
    let release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let pretty = release.lines().find_map(|l| l.strip_prefix("PRETTY_NAME="));

    pretty.map_or("Linux".into(), |v| v.trim_matches('"').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(cmd: &str, exit: i32, output: &str) -> Command {
        Command {
            cmd: cmd.into(),
            exit,
            output: output.into(),
            available: true,
            ..Default::default()
        }
    }

    #[test]
    fn formats_one_command() {
        let got = format_commands(&[cmd("pactl list short sinks", 0, "1\tsink\n")]);

        assert_eq!(got, "$ pactl list short sinks\nexit 0\n```\n1\tsink\n```\n");
    }

    #[test]
    fn separates_blocks_and_ends_output_with_newline() {
        let got = format_commands(&[cmd("a", 0, "x"), cmd("b", 1, "")]);

        assert_eq!(got, "$ a\nexit 0\n```\nx\n```\n\n$ b\nexit 1\n```\n```\n");
    }

    #[test]
    fn says_when_no_commands() {
        assert_eq!(format_commands(&[]), NO_COMMANDS);
    }

    #[test]
    fn uses_default_question_when_empty() {
        let got = build_prompt("Q: {question}", &[], "  ", &PromptEnv::default());

        assert_eq!(got, format!("Q: {DEFAULT_QUESTION}"));
    }

    #[test]
    fn substitutes_in_one_pass() {
        let cmds = [cmd("echo", 0, "{question} {shell}\n")];
        let env = PromptEnv {
            shell: "bash".into(),
            cwd: "/tmp".into(),
            os: "Arch Linux".into(),
        };
        let got = build_prompt(
            "{shell}|{cwd}|{os}|{question}|{commands}|{other}",
            &cmds,
            "why?",
            &env,
        );

        assert_eq!(
            got,
            format!(
                "bash|/tmp|Arch Linux|why?|{}|{{other}}",
                format_commands(&cmds)
            )
        );
    }
}
