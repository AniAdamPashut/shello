use crate::agent;
use crate::clean::Trim;
use crate::cli::{self, Options};
use crate::config::{self, AgentConfig, Config};
use crate::extract::{self, Command};
use crate::paths::config_dir;
use crate::prompt;
use crate::redact::{self, Redactor};
use crate::session::{self, SessionEnv};
use std::process::Stdio;

/// What ends a run early: a message for stderr and the exit code.
struct Failure {
    code: i32,
    message: String,
}

impl Failure {
    fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Wraps an error message as an exit-1 failure prefixed with "shello: ".
fn failed(message: String) -> Failure {
    Failure::new(1, format!("shello: {message}"))
}

pub fn run(args: &[String]) -> i32 {
    match cli::parse_args(args) {
        Ok(opts) => finish(execute(&opts)),
        Err(e) => {
            // clap prints help/version to stdout (exit 0) and errors to stderr (exit 2).
            let _ = e.print();
            if let Some(hint) = cli::hint(&e) {
                eprintln!("\n{hint}");
            }

            e.exit_code()
        }
    }
}

fn finish(result: Result<i32, Failure>) -> i32 {
    result.unwrap_or_else(|f| {
        eprintln!("{}", f.message);
        f.code
    })
}

fn execute(opts: &Options) -> Result<i32, Failure> {
    let cfg = config::load_config(&config_dir().join("config.toml")).map_err(failed)?;
    let prompt = build_prompt(opts, &cfg)?;
    let name = opts.agent.as_deref().unwrap_or(&cfg.agent);
    let agent = cfg.agents.get(name);
    if opts.show {
        show(&prompt, agent, opts.print);
        return Ok(0);
    }

    let agent =
        agent.ok_or_else(|| Failure::new(2, format!("shello: unknown agent \"{name}\"")))?;
    if opts.print {
        return run_print(name, agent, &prompt);
    }

    run_interactive(name, agent, &prompt)
}

fn build_prompt(opts: &Options, cfg: &Config) -> Result<String, Failure> {
    let tmpl = config::load_prompt_template(&config_dir().join("prompt.md")).map_err(failed)?;
    let cmds = recent_commands(opts, cfg)?;

    Ok(prompt::build_prompt(
        &tmpl,
        &cmds,
        &opts.question_text(),
        &prompt::detect_env(),
    ))
}

fn recent_commands(opts: &Options, cfg: &Config) -> Result<Vec<Command>, Failure> {
    let session = session::current_session(&SessionEnv::from_env(), &current_tty())
        .map_err(|e| Failure::new(1, e))?;
    let recs = session::read_cmds(&session.cmds_path()).map_err(failed)?;
    let log =
        session::read_log_tail(&session.log_path(), cfg.capture.max_log_bytes).map_err(failed)?;
    let n = opts.commands.unwrap_or(cfg.capture.commands);
    let trim = Trim {
        head: cfg.capture.head_lines,
        tail: cfg.capture.tail_lines,
    };

    let mut cmds = extract::last_commands(&recs, &log, n, trim);
    redact_secrets(&mut cmds, cfg)?;

    Ok(cmds)
}

/// Scrubs secrets from the commands and says how many were removed.
fn redact_secrets(cmds: &mut [Command], cfg: &Config) -> Result<(), Failure> {
    if !cfg.redact.enabled {
        return Ok(());
    }

    let redactor = Redactor::new(&cfg.redact.patterns).map_err(failed)?;
    let count = redact::redact_commands(cmds, &redactor);
    if count > 0 {
        let noun = if count == 1 { "secret" } else { "secrets" };
        eprintln!("shello: redacted {count} {noun} before sending (see --show)");
    }

    Ok(())
}

fn current_tty() -> String {
    let tty = std::fs::read_link("/proc/self/fd/0");

    tty.map(|p| p.display().to_string()).unwrap_or_default()
}

/// --show: the prompt on stdout, and which command would run on stderr.
fn show(prompt: &str, agent: Option<&AgentConfig>, print: bool) {
    print!("{prompt}");
    if let Some(agent) = agent {
        eprintln!("agent: {:?}", mode_template(agent, print));
    }
}

fn mode_template(agent: &AgentConfig, print: bool) -> &[String] {
    if print {
        &agent.print
    } else {
        &agent.interactive
    }
}

fn run_print(name: &str, agent: &AgentConfig, prompt: &str) -> Result<i32, Failure> {
    let tmpl = require_template(name, &agent.print)?;
    let (argv, use_stdin) = agent::build_argv(tmpl, prompt);

    Ok(agent::run_print(&argv, prompt, use_stdin, Stdio::inherit()))
}

fn run_interactive(name: &str, agent: &AgentConfig, prompt: &str) -> Result<i32, Failure> {
    let tmpl = require_template(name, &agent.interactive)?;
    if agent::too_large_for_interactive(prompt) {
        return Err(Failure::new(
            1,
            "prompt too large for interactive mode; use -p or a smaller -n",
        ));
    }

    let (argv, use_stdin) = agent::build_argv(tmpl, prompt);
    if use_stdin {
        let msg = format!(
            "shello: agent \"{name}\" needs a {{prompt}} placeholder in its interactive command"
        );
        return Err(Failure::new(2, msg));
    }

    let err = agent::exec_interactive(&argv);
    Err(Failure::new(127, format!("shello: {}: {err}", argv[0])))
}

fn require_template<'a>(name: &str, tmpl: &'a [String]) -> Result<&'a [String], Failure> {
    if tmpl.is_empty() {
        return Err(Failure::new(
            2,
            format!("shello: agent \"{name}\" has no command for this mode"),
        ));
    }

    Ok(tmpl)
}
