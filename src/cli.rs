use clap::Parser;

/// Ask an AI about what just happened in your terminal.
///
/// Sends your last commands in this terminal, with their output and exit
/// codes, plus your question to an AI agent (claude by default).
#[derive(Debug, Clone, Default, PartialEq, Parser)]
#[command(name = "shello", version, arg_required_else_help = true)]
pub struct Options {
    /// Single answer, then exit (default: open a conversation)
    #[arg(short, long)]
    pub print: bool,

    /// Print the built prompt and exit; call no agent
    #[arg(long)]
    pub show: bool,

    /// How many past commands to include [default reads from config]
    #[arg(short = 'n', long, value_name = "N", value_parser = parse_count)]
    pub commands: Option<usize>,

    /// Which configured agent to use
    #[arg(short, long, value_name = "NAME")]
    pub agent: Option<String>,

    /// Send no question; the agent explains what happened in the commands
    #[arg(short, long, conflicts_with = "question")]
    pub empty: bool,

    /// Your question; no quoting needed. Use -- before one starting with "-"
    #[arg(
        value_name = "QUESTION",
        trailing_var_arg = true,
        required_unless_present = "empty"
    )]
    pub question: Vec<String>,
}

impl Options {
    pub fn question_text(&self) -> String {
        self.question.join(" ")
    }
}

/// Parses shello's arguments (without the program name).
pub fn parse_args(args: &[String]) -> Result<Options, clap::Error> {
    let argv = std::iter::once("shello").chain(args.iter().map(String::as_str));

    Options::try_parse_from(argv)
}

pub const EMPTY_HINT: &str =
    "tip: ask a question, or pass -e/--empty to send the commands without one";

/// Extra advice to print after a parse error, if any.
pub fn hint(err: &clap::Error) -> Option<&'static str> {
    let missing_question = err.kind() == clap::error::ErrorKind::MissingRequiredArgument;

    missing_question.then_some(EMPTY_HINT)
}

fn parse_count(s: &str) -> Result<usize, String> {
    let n = s.parse::<usize>().ok().filter(|n| *n > 0);

    n.ok_or_else(|| format!("expected a whole number of at least 1, got '{s}'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    use clap::error::ErrorKind;

    fn parse(args: &[&str]) -> Result<Options, clap::Error> {
        parse_args(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    fn opts(args: &[&str]) -> Options {
        parse(args).unwrap_or_else(|e| panic!("parse({args:?}) failed: {e}"))
    }

    fn words(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn parses_flags_and_question() {
        let q = |s: &str| s.to_string();
        let w = words;
        let cases: &[(&[&str], Options)] = &[
            (
                &["-p", "how", "do", "I"],
                Options {
                    print: true,
                    question: w("how do I"),
                    ..Default::default()
                },
            ),
            (
                &["--print", "x"],
                Options {
                    print: true,
                    question: w("x"),
                    ..Default::default()
                },
            ),
            (
                &["-n", "5", "--show", "-e"],
                Options {
                    commands: Some(5),
                    show: true,
                    empty: true,
                    ..Default::default()
                },
            ),
            (
                &["--commands=2", "q"],
                Options {
                    commands: Some(2),
                    question: w("q"),
                    ..Default::default()
                },
            ),
            (
                &["--agent", "codex", "q"],
                Options {
                    agent: Some(q("codex")),
                    question: w("q"),
                    ..Default::default()
                },
            ),
            (
                &["-a", "gemini", "--empty"],
                Options {
                    agent: Some(q("gemini")),
                    empty: true,
                    ..Default::default()
                },
            ),
            (
                &["-e"],
                Options {
                    empty: true,
                    ..Default::default()
                },
            ),
            (
                &["why", "-p"],
                Options {
                    question: w("why -p"),
                    ..Default::default()
                },
            ),
            (
                &["--", "-p", "is", "a", "flag?"],
                Options {
                    question: w("-p is a flag?"),
                    ..Default::default()
                },
            ),
        ];

        for (args, want) in cases {
            assert_eq!(&opts(args), want, "args {args:?}");
        }
    }

    #[test]
    fn help_flag() {
        for flag in ["--help", "-h"] {
            assert_eq!(parse(&[flag]).unwrap_err().kind(), ErrorKind::DisplayHelp);
        }
    }

    #[test]
    fn no_arguments_shows_help() {
        let err = parse(&[]).unwrap_err();

        assert_eq!(
            err.kind(),
            ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        );
    }

    #[test]
    fn missing_question_needs_empty_flag() {
        for args in [&["-p"][..], &["--show"], &["-n", "2"]] {
            assert_eq!(
                parse(args).unwrap_err().kind(),
                ErrorKind::MissingRequiredArgument,
                "args {args:?}"
            );
        }
    }

    #[test]
    fn hints_at_empty_flag_when_question_missing() {
        assert_eq!(hint(&parse(&["-p"]).unwrap_err()), Some(EMPTY_HINT));
        assert_eq!(hint(&parse(&["--bogus"]).unwrap_err()), None);
    }

    #[test]
    fn empty_flag_conflicts_with_question() {
        assert_eq!(
            parse(&["-e", "why"]).unwrap_err().kind(),
            ErrorKind::ArgumentConflict
        );
    }

    #[test]
    fn joins_question_words() {
        assert_eq!(opts(&["how", "do", "I"]).question_text(), "how do I");
    }

    #[test]
    fn rejects_bad_input() {
        for args in [&["-n", "0"][..], &["-n", "x"], &["-n"], &["--bogus"]] {
            let err = parse(args).expect_err(&format!("args {args:?}"));
            assert_ne!(err.kind(), ErrorKind::DisplayHelp);
        }
    }
}
