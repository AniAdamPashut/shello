use std::iter::Peekable;
use std::slice::Iter;

pub const USAGE: &str = "usage: shello [options] [question...]

Sends your last commands in this terminal, with their output, to an AI agent.

  -p, --print        single answer, then exit (default: conversation)
  -n, --commands N   how many past commands to include (default 3)
  -a, --agent NAME   pick a configured agent
      --show         print the built prompt and exit; call no agent
  -h, --help         show this help
";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Options {
    pub print: bool,
    pub show: bool,
    pub commands: Option<usize>,
    pub agent: Option<String>,
    pub question: String,
}

#[derive(Debug, PartialEq)]
pub enum Parsed {
    Help,
    Run(Options),
}

type Args<'a> = Peekable<Iter<'a, String>>;

/// Parses flags up to the first non-flag (or "--"); the rest is the question,
/// so it never needs quoting.
pub fn parse_args(args: &[String]) -> Result<Parsed, String> {
    let mut opts = Options::default();
    let mut it = args.iter().peekable();
    while let Some(arg) = it.next_if(|a| a.starts_with('-')) {
        if arg == "--" {
            break;
        }

        if apply_flag(arg, &mut it, &mut opts)? {
            return Ok(Parsed::Help);
        }
    }

    opts.question = it.cloned().collect::<Vec<_>>().join(" ");
    Ok(Parsed::Run(opts))
}

/// Applies one flag to opts; returns true when it asks for help.
fn apply_flag(arg: &str, it: &mut Args, opts: &mut Options) -> Result<bool, String> {
    let (flag, inline) = split_inline_value(arg);
    match flag {
        "-h" | "--help" => return Ok(true),
        "-p" | "--print" => opts.print = true,
        "--show" => opts.show = true,
        "-n" | "--commands" => opts.commands = Some(parse_count(&value(flag, inline, it)?)?),
        "-a" | "--agent" => opts.agent = Some(value(flag, inline, it)?),
        _ => return Err(format!("unknown option {arg}")),
    }

    Ok(false)
}

/// Splits "--flag=value" into the flag and its value.
fn split_inline_value(arg: &str) -> (&str, Option<&str>) {
    match arg.split_once('=') {
        Some((flag, value)) if arg.starts_with("--") => (flag, Some(value)),
        _ => (arg, None),
    }
}

fn value(flag: &str, inline: Option<&str>, it: &mut Args) -> Result<String, String> {
    let value = inline.map(str::to_string).or_else(|| it.next().cloned());

    value.ok_or_else(|| format!("{flag} needs a value"))
}

fn parse_count(s: &str) -> Result<usize, String> {
    let n = s.parse::<usize>().ok().filter(|n| *n > 0);

    n.ok_or_else(|| format!("invalid number of commands: {s}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Parsed, String> {
        parse_args(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    fn opts(args: &[&str]) -> Options {
        match parse(args) {
            Ok(Parsed::Run(o)) => o,
            other => panic!("parse({args:?}) = {other:?}"),
        }
    }

    #[test]
    fn parses_flags_and_question() {
        let q = |s: &str| s.to_string();
        let cases: &[(&[&str], Options)] = &[
            (
                &["-p", "how", "do", "I"],
                Options {
                    print: true,
                    question: q("how do I"),
                    ..Default::default()
                },
            ),
            (
                &["--print", "x"],
                Options {
                    print: true,
                    question: q("x"),
                    ..Default::default()
                },
            ),
            (
                &["-n", "5", "--show"],
                Options {
                    commands: Some(5),
                    show: true,
                    ..Default::default()
                },
            ),
            (
                &["--commands=2", "q"],
                Options {
                    commands: Some(2),
                    question: q("q"),
                    ..Default::default()
                },
            ),
            (
                &["--agent", "codex", "q"],
                Options {
                    agent: Some(q("codex")),
                    question: q("q"),
                    ..Default::default()
                },
            ),
            (
                &["-a", "gemini"],
                Options {
                    agent: Some(q("gemini")),
                    ..Default::default()
                },
            ),
            (&[], Options::default()),
            (
                &["why", "-p"],
                Options {
                    question: q("why -p"),
                    ..Default::default()
                },
            ),
            (
                &["--", "-p", "is", "a", "flag?"],
                Options {
                    question: q("-p is a flag?"),
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
        assert_eq!(parse(&["--help"]), Ok(Parsed::Help));
        assert_eq!(parse(&["-h"]), Ok(Parsed::Help));
    }

    #[test]
    fn rejects_bad_input() {
        for args in [&["-n", "0"][..], &["-n", "x"], &["-n"], &["--bogus"]] {
            assert!(parse(args).is_err(), "args {args:?}");
        }
    }
}
