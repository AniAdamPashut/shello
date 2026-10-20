use crate::clean::{clean_output, truncate, Trim};
use crate::session::CmdRecord;
use std::path::Path;

pub const UNAVAILABLE: &str = "[output not available]";
const START_MARKER: &str = "\x1b]6973;start;";
const END_MARKER: &str = "\x1b]6973;end;";

/// A past command, ready to be put in the prompt.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Command {
    pub cmd: String,
    pub exit: i32,
    pub output: String,
    pub available: bool,
}

/// Returns the log bytes between the last end marker for seq and the last
/// start marker for seq before it. An empty prompt line can leave a stale
/// start marker with the same seq, which is why the last one wins.
pub fn extract_output<'a>(log: &'a [u8], seq: &str) -> Option<&'a [u8]> {
    let end = rfind(log, format!("{END_MARKER}{seq};").as_bytes())?;
    let start_marker = format!("{START_MARKER}{seq}\x07");
    let start = rfind(&log[..end], start_marker.as_bytes())?;

    Some(&log[start + start_marker.len()..end])
}

fn rfind(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).rposition(|w| w == needle)
}

/// Reports whether cmd is an invocation of shello itself.
pub fn is_shello_cmd(cmd: &str) -> bool {
    let program = cmd.split_whitespace().next().map(Path::new);

    program.and_then(Path::file_name) == Some("shello".as_ref())
}

/// The last n commands that aren't shello itself, oldest first, with their
/// cleaned and truncated output.
pub fn last_commands(recs: &[CmdRecord], log: &[u8], n: usize, trim: Trim) -> Vec<Command> {
    let mut picked: Vec<&CmdRecord> = recs
        .iter()
        .rev()
        .filter(|r| !is_shello_cmd(&r.cmd))
        .take(n)
        .collect();
    picked.reverse();

    picked
        .into_iter()
        .map(|r| to_command(r, log, trim))
        .collect()
}

fn to_command(rec: &CmdRecord, log: &[u8], trim: Trim) -> Command {
    let output = extract_output(log, &rec.seq).map(|raw| truncate(&clean_output(raw), trim));

    Command {
        cmd: rec.cmd.clone(),
        exit: rec.exit,
        available: output.is_some(),
        output: output.unwrap_or_else(|| UNAVAILABLE.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRIM: Trim = Trim {
        head: 100,
        tail: 100,
    };

    fn start(seq: &str) -> String {
        format!("\x1b]6973;start;{seq}\x07")
    }

    fn end(seq: &str, code: i32) -> String {
        format!("\x1b]6973;end;{seq};{code}\x07")
    }

    fn recs(cmds: &[&str]) -> Vec<CmdRecord> {
        let to_rec = |(i, c): (usize, &&str)| CmdRecord {
            seq: format!("1.{}", i + 1),
            cmd: c.to_string(),
            ..Default::default()
        };
        cmds.iter().enumerate().map(to_rec).collect()
    }

    #[test]
    fn extracts_between_markers() {
        let log = format!("$ ls\r\n{}a\r\nb\r\n{}", start("1.1"), end("1.1", 0));

        assert_eq!(
            extract_output(log.as_bytes(), "1.1"),
            Some(&b"a\r\nb\r\n"[..])
        );
    }

    #[test]
    fn last_start_marker_wins() {
        let log = format!("{}stale{}real{}", start("1.2"), start("1.2"), end("1.2", 0));

        assert_eq!(extract_output(log.as_bytes(), "1.2"), Some(&b"real"[..]));
    }

    #[test]
    fn missing_start_marker_is_none() {
        let log = format!("tail-only{}", end("1.3", 1));

        assert_eq!(extract_output(log.as_bytes(), "1.3"), None);
    }

    #[test]
    fn seq_does_not_match_its_prefix() {
        let one = format!("{}one{}", start("1.1"), end("1.1", 0));
        let log = format!("{one}{}ten{}", start("1.10"), end("1.10", 0));

        assert_eq!(extract_output(log.as_bytes(), "1.1"), Some(&b"one"[..]));
    }

    #[test]
    fn excludes_shello_invocations() {
        let r = recs(&[
            "ls",
            "pactl list",
            "shello hi",
            "/home/u/.local/bin/shello -p x",
            "echo",
        ]);
        let got: Vec<String> = last_commands(&r, b"", 3, TRIM)
            .into_iter()
            .map(|c| c.cmd)
            .collect();

        assert_eq!(got, ["ls", "pactl list", "echo"]);
    }

    #[test]
    fn returns_fewer_when_history_is_short() {
        assert_eq!(last_commands(&recs(&["ls"]), b"", 3, TRIM).len(), 1);
    }

    #[test]
    fn marks_missing_output() {
        let got = last_commands(&recs(&["ls"]), b"nothing here", 3, TRIM);

        assert!(!got[0].available);
        assert_eq!(got[0].output, UNAVAILABLE);
    }

    #[test]
    fn keeps_empty_interrupted_command() {
        let r = [CmdRecord {
            seq: "1.5".into(),
            cmd: "sleep 5".into(),
            exit: 130,
            ts: 0,
        }];
        let log = format!("{}{}", start("1.5"), end("1.5", 130));
        let got = last_commands(&r, log.as_bytes(), 3, TRIM);

        assert!(got[0].available);
        assert_eq!(got[0].output, "");
        assert_eq!(got[0].exit, 130);
    }

    #[test]
    fn cleans_and_truncates_output() {
        let log = format!(
            "{}\x1b[1mA\x1b[0m\r\nB\r\nC\r\nD\r\n{}",
            start("1.1"),
            end("1.1", 0)
        );
        let got = last_commands(&recs(&["x"]), log.as_bytes(), 3, Trim { head: 1, tail: 1 });

        assert_eq!(got[0].output, "A\n[... 2 lines omitted ...]\nD\n");
    }
}
