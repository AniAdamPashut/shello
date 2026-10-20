use serde::Deserialize;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use toml::{Table, Value};

pub const DEFAULT_CONFIG: &str = include_str!("../config.toml");
pub const DEFAULT_PROMPT: &str = include_str!("../prompt.md");

/// Command templates for one agent; see config.toml.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct AgentConfig {
    #[serde(default)]
    pub interactive: Vec<String>,
    #[serde(default)]
    pub print: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CaptureConfig {
    pub commands: usize,
    pub head_lines: usize,
    pub tail_lines: usize,
    pub max_log_bytes: u64,
}

/// Secrets are scrubbed from the prompt before it's sent; see redact.rs.
#[derive(Debug, Clone, Deserialize)]
pub struct RedactConfig {
    pub enabled: bool,
    /// Extra regexes; each match is replaced whole.
    pub patterns: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub agent: String,
    pub agents: HashMap<String, AgentConfig>,
    pub capture: CaptureConfig,
    pub redact: RedactConfig,
}

/// Loads the built-in config with the user's file at path laid over it.
/// A missing file is not an error.
pub fn load_config(path: &Path) -> Result<Config, String> {
    let mut table = parse_table(DEFAULT_CONFIG, "built-in config")?;
    if let Some(text) = read_optional(path)? {
        merge(&mut table, parse_table(&text, &path.display().to_string())?);
    }

    Value::Table(table)
        .try_into()
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// Reads the prompt template at path, or the built-in one if it's missing.
pub fn load_prompt_template(path: &Path) -> Result<String, String> {
    let text = read_optional(path)?;

    Ok(text.unwrap_or_else(|| DEFAULT_PROMPT.to_string()))
}

fn parse_table(text: &str, origin: &str) -> Result<Table, String> {
    text.parse::<Table>().map_err(|e| format!("{origin}: {e}"))
}

fn read_optional(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Deep-merges over into base: nested tables merge, other values replace.
fn merge(base: &mut Table, over: Table) {
    for (key, value) in over {
        match (base.get_mut(&key), value) {
            (Some(Value::Table(inner)), Value::Table(value)) => merge(inner, value),
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::temp_file;

    #[test]
    fn missing_file_uses_defaults() {
        let cfg = load_config(Path::new("/nonexistent/config.toml")).unwrap();

        assert_eq!(cfg.agent, "claude");
        assert_eq!(cfg.agents["claude"].print, ["claude", "-p"]);
        assert_eq!(cfg.capture.commands, 1);
        assert_eq!(cfg.capture.max_log_bytes, 2_097_152);
        assert!(cfg.redact.enabled);
        assert!(cfg.redact.patterns.is_empty());
    }

    #[test]
    fn redaction_can_be_configured() {
        let path = temp_file(
            "config.toml",
            b"[redact]\nenabled = false\npatterns = ['x+']\n",
        );
        let cfg = load_config(&path).unwrap();

        assert!(!cfg.redact.enabled);
        assert_eq!(cfg.redact.patterns, ["x+"]);
    }

    #[test]
    fn user_file_overlays_defaults() {
        let path = temp_file(
            "config.toml",
            b"agent = \"codex\"\n[capture]\ncommands = 5\n",
        );
        let cfg = load_config(&path).unwrap();

        assert_eq!(cfg.agent, "codex");
        assert_eq!(cfg.capture.commands, 5);
        assert_eq!(cfg.capture.head_lines, 100);
        assert!(cfg.agents.contains_key("claude"));
    }

    #[test]
    fn invalid_file_is_an_error() {
        let path = temp_file("bad.toml", b"agent = [");

        assert!(load_config(&path).is_err());
    }

    #[test]
    fn missing_template_uses_default() {
        let tmpl = load_prompt_template(Path::new("/nonexistent/prompt.md")).unwrap();

        assert!(tmpl.contains("{commands}"));
        assert!(tmpl.contains("{question}"));
    }
}
