use crate::extract::Command;
use regex::{Captures, Regex};

pub const REDACTED: &str = "[redacted]";
pub const REDACTED_KEY: &str = "[redacted private key]";

/// PEM private keys (ssh, TLS, PGP), up to their END line or the end of the
/// output when it was cut off.
const PRIVATE_KEY: &str = r"(?s)-----BEGIN [A-Z0-9 ]*PRIVATE KEY(?: BLOCK)?-----.*?(?:-----END [A-Z0-9 ]*PRIVATE KEY(?: BLOCK)?-----|\z)";

/// Credentials with a recognizable shape, replaced wherever they appear.
const TOKENS: &[&str] = &[
    r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b",          // AWS access key id
    r"\bgh[pousr]_[A-Za-z0-9]{36,}\b",         // GitHub token
    r"\bgithub_pat_[A-Za-z0-9_]{22,}",         // GitHub fine-grained token
    r"\bglpat-[A-Za-z0-9_-]{20,}",             // GitLab token
    r"\bxox[abposr]-[A-Za-z0-9-]{10,}",        // Slack token
    r"\bsk-[A-Za-z0-9_-]{20,}",                // OpenAI / Anthropic key
    r"\b[rs]k_(?:live|test)_[A-Za-z0-9]{16,}", // Stripe key
    r"\bAIza[0-9A-Za-z_-]{35}",                // Google API key
    r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}", // JWT
];

/// "Authorization: Bearer <secret>": keeps the header and scheme.
const AUTH_HEADER: &str =
    r"(?i)(\bauthorization[ \t]*[:=][ \t]*(?:(?:bearer|basic|token|digest)[ \t]+)?)[^\s'\x22]+";

/// "scheme://user:<secret>@host": keeps everything but the password.
const URL_PASSWORD: &str = r"(\b[a-zA-Z][a-zA-Z0-9+.-]*://[^/\s:@]+:)[^@\s/]+@";

/// "SOME_TOKEN=<secret>", "password: <secret>", "api_key": "<secret>": keeps
/// the name. A value starting with "[" is already redacted.
const SECRET_ASSIGNMENT: &str = r#"(?i)\b([a-z0-9_.-]*(?:password|passwd|passphrase|secret|token|api[_-]?key|access[_-]?key|private[_-]?key|auth[_-]?key|credentials?)[a-z0-9_.-]*)(["']?[ \t]*[=:][ \t]*)("[^"\n]*"|'[^'\n]*'|[^\s\["',;]+)"#;

/// A pattern and what each match becomes ($1-style groups allowed).
struct Rule {
    re: Regex,
    replacement: String,
}

impl Rule {
    fn new(pattern: &str, replacement: &str) -> Result<Self, regex::Error> {
        let re = Regex::new(pattern)?;

        Ok(Self {
            re,
            replacement: replacement.to_string(),
        })
    }

    fn builtin(pattern: &str, replacement: &str) -> Self {
        Self::new(pattern, replacement).expect("built-in redaction pattern")
    }

    /// Replaces every match, adding the number of matches to count.
    fn apply(&self, text: &str, count: &mut usize) -> String {
        let replace = |caps: &Captures| {
            *count += 1;
            let mut out = String::new();
            caps.expand(&self.replacement, &mut out);
            out
        };

        self.re.replace_all(text, replace).into_owned()
    }
}

/// Finds secrets in text and replaces them with a placeholder.
pub struct Redactor {
    rules: Vec<Rule>,
}

impl Redactor {
    /// The built-in rules plus the user's extra regexes (matched whole).
    pub fn new(extra_patterns: &[String]) -> Result<Self, String> {
        let mut rules = builtin_rules();
        for pattern in extra_patterns {
            let rule = Rule::new(pattern, REDACTED)
                .map_err(|e| format!("redact pattern {pattern:?}: {e}"))?;
            rules.push(rule);
        }

        Ok(Self { rules })
    }

    /// Returns the redacted text and how many secrets were replaced.
    pub fn redact(&self, text: &str) -> (String, usize) {
        let mut count = 0;
        let mut text = text.to_string();
        for rule in &self.rules {
            text = rule.apply(&text, &mut count);
        }

        (text, count)
    }

    fn redact_in_place(&self, text: &mut String) -> usize {
        let (redacted, count) = self.redact(text);
        *text = redacted;
        count
    }
}

/// Specific shapes first, so a generic rule never re-matches their placeholder.
fn builtin_rules() -> Vec<Rule> {
    let mut rules = vec![Rule::builtin(PRIVATE_KEY, REDACTED_KEY)];
    rules.extend(TOKENS.iter().map(|t| Rule::builtin(t, REDACTED)));
    rules.push(Rule::builtin(AUTH_HEADER, "${1}[redacted]"));
    rules.push(Rule::builtin(URL_PASSWORD, "${1}[redacted]@"));
    rules.push(Rule::builtin(SECRET_ASSIGNMENT, "${1}${2}[redacted]"));
    rules
}

/// Redacts each command's text and output; returns how many secrets were replaced.
pub fn redact_commands(cmds: &mut [Command], redactor: &Redactor) -> usize {
    let mut total = 0;
    for c in cmds.iter_mut() {
        total += redactor.redact_in_place(&mut c.cmd);
        total += redactor.redact_in_place(&mut c.output);
    }

    total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn redact(text: &str) -> (String, usize) {
        Redactor::new(&[]).unwrap().redact(text)
    }

    fn assert_redacted(input: &str, want: &str) {
        let (got, count) = redact(input);

        assert_eq!(got, want, "input {input:?}");
        assert_eq!(count, 1, "input {input:?}");
    }

    #[test]
    fn redacts_private_key_blocks() {
        let key = "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAA\nAAAABG5vbmUAAAAEbm9u\n-----END OPENSSH PRIVATE KEY-----";

        assert_redacted(
            &format!("$ cat id\n{key}\ndone"),
            &format!("$ cat id\n{REDACTED_KEY}\ndone"),
        );
        assert_redacted(
            "-----BEGIN RSA PRIVATE KEY-----\nMIIEow\n(cut off",
            REDACTED_KEY,
        );
        assert_redacted(
            "-----BEGIN PGP PRIVATE KEY BLOCK-----\nlQ\n-----END PGP PRIVATE KEY BLOCK-----",
            REDACTED_KEY,
        );
    }

    #[test]
    fn redacts_known_token_formats() {
        let tokens = [
            "AKIAIOSFODNN7EXAMPLE",
            "ghp_abcdefghijklmnopqrstuvwxyzABCDEF0123",
            "github_pat_11ABCDEFG0123456789abcdefghij",
            "glpat-abcdefghij0123456789",
            "xoxb-1234567890-abcdefghij",
            "sk-ant-api03-abcdefghijklmnopqrstuvwxyz",
            "sk_live_abcdefghijklmnop1234",
            "AIzaSyA1234567890abcdefghijklmnopqrstuv",
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U",
        ];

        for token in tokens {
            assert_redacted(
                &format!("value {token} end"),
                &format!("value {REDACTED} end"),
            );
        }
    }

    #[test]
    fn redacts_authorization_headers() {
        assert_redacted(
            "Authorization: Bearer abc.def-123",
            "Authorization: Bearer [redacted]",
        );
        assert_redacted(
            "curl -H 'authorization: Basic dXNlcjpwYXNz'",
            "curl -H 'authorization: Basic [redacted]'",
        );
    }

    #[test]
    fn redacts_url_passwords() {
        assert_redacted(
            "https://bob:hunter2@example.com/x",
            "https://bob:[redacted]@example.com/x",
        );
        assert_redacted(
            "postgres://app:s3cr3t@db:5432/app",
            "postgres://app:[redacted]@db:5432/app",
        );
    }

    #[test]
    fn redacts_secret_assignments() {
        assert_redacted(
            "export GITHUB_TOKEN=abc123",
            "export GITHUB_TOKEN=[redacted]",
        );
        assert_redacted("DB_PASSWORD='p w'", "DB_PASSWORD=[redacted]");
        assert_redacted(r#""api_key": "xyz789","#, r#""api_key": [redacted],"#);
        assert_redacted("client_secret: abc", "client_secret: [redacted]");
    }

    #[test]
    fn counts_a_secret_once() {
        assert_redacted(
            "token=ghp_abcdefghijklmnopqrstuvwxyzABCDEF0123",
            "token=[redacted]",
        );
    }

    #[test]
    fn leaves_ordinary_output_alone() {
        let ordinary = [
            "Sink #52\n\tState: RUNNING\n\tName: alsa_output.pci-0000_00_1f.3.analog-stereo",
            "PWD=/home/u\nOLDPWD=/tmp",
            "Password:\nnext line",
            "sudo: a password is required",
            "https://example.com/path:8080",
            "tokens used: 1234",
        ];

        for text in ordinary {
            assert_eq!(redact(text), (text.to_string(), 0), "input {text:?}");
        }
    }

    #[test]
    fn applies_extra_patterns() {
        let r = Redactor::new(&["hunter\\d".to_string()]).unwrap();

        assert_eq!(
            r.redact("pw is hunter2"),
            ("pw is [redacted]".to_string(), 1)
        );
    }

    #[test]
    fn rejects_invalid_extra_patterns() {
        assert!(Redactor::new(&["(".to_string()]).is_err());
    }

    #[test]
    fn redacts_command_text_and_output() {
        let mut cmds = [
            Command {
                cmd: "echo $API_KEY".into(),
                output: "API_KEY=abc\n".into(),
                ..Default::default()
            },
            Command {
                cmd: "curl -u bob https://bob:pw@x.io".into(),
                ..Default::default()
            },
        ];
        let r = Redactor::new(&[]).unwrap();

        assert_eq!(redact_commands(&mut cmds, &r), 2);
        assert_eq!(cmds[0].output, "API_KEY=[redacted]\n");
        assert_eq!(cmds[1].cmd, "curl -u bob https://bob:[redacted]@x.io");
    }
}
