use regex::{NoExpand, Regex};
use std::sync::LazyLock;

pub const ALT_SCREEN_NOTE: &str = "[full-screen program output omitted]";

/// How many lines to keep from the start and the end of an output.
#[derive(Debug, Clone, Copy)]
pub struct Trim {
    pub head: usize,
    pub tail: usize,
}

/// Full-screen programs (vim, less, htop...): from entering the alternate
/// screen to leaving it, or to the end when the program never left.
static ALT_SCREEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)\x1b\[\?(?:1049|1047|47)h.*?(?:\x1b\[\?(?:1049|1047|47)l|\z)").unwrap()
});

/// OSC sequences (titles, shello markers), CSI sequences (colors, cursor
/// movement) and other two-byte escapes.
static ESCAPES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b\[[0-?]*[ -/]*[@-~]|\x1b[@-_]").unwrap()
});

/// Turns raw terminal bytes into the plain text a person would have seen.
pub fn clean_output(raw: &[u8]) -> String {
    let text = String::from_utf8_lossy(raw);
    let text = ALT_SCREEN.replace_all(&text, NoExpand(ALT_SCREEN_NOTE));
    let text = ESCAPES.replace_all(&text, "").replace("\r\n", "\n");

    text.split('\n')
        .map(replay_line)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Applies \r (back to column 0) and backspace (one column left) as
/// overwrites, so progress bars collapse to their final state.
fn replay_line(line: &str) -> String {
    if !line.contains(['\r', '\x08']) {
        return line.to_string();
    }

    let mut buf: Vec<char> = Vec::new();
    let mut col: usize = 0;
    for c in line.chars() {
        match c {
            '\r' => col = 0,
            '\x08' => col = col.saturating_sub(1),
            _ => {
                overwrite(&mut buf, col, c);
                col += 1;
            }
        }
    }

    buf.into_iter().collect()
}

fn overwrite(buf: &mut Vec<char>, col: usize, c: char) {
    match buf.get_mut(col) {
        Some(slot) => *slot = c,
        None => buf.push(c),
    }
}

/// Keeps the first and last lines of s, replacing the middle with a note.
pub fn truncate(s: &str, trim: Trim) -> String {
    let (body, newline) = split_final_newline(s);
    let lines: Vec<&str> = body.split('\n').collect();
    if lines.len() <= trim.head + trim.tail {
        return s.to_string();
    }

    let omitted = lines.len() - trim.head - trim.tail;
    let note = format!("[... {omitted} lines omitted ...]");
    let mut kept = lines[..trim.head].to_vec();
    kept.push(&note);
    kept.extend(&lines[lines.len() - trim.tail..]);

    kept.join("\n") + newline
}

fn split_final_newline(s: &str) -> (&str, &str) {
    match s.strip_suffix('\n') {
        Some(body) => (body, "\n"),
        None => (s, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_escapes_and_replays_overwrites() {
        let cases: &[(&[u8], &str)] = &[
            (b"a\x1b[31mred\x1b[0m\r\n", "ared\n"),
            (b"line1\r\nline2\r\n", "line1\nline2\n"),
            (b"10%\r50%\r100%\r\n", "100%\n"),
            (b"abc\x08\x08X\r\n", "aXc\n"),
            (b"x\x1b]0;title\x07y", "xy"),
            (b"x\x1b]6973;end;1.1;0\x07y", "xy"),
            (b"\xffok", "\u{FFFD}ok"),
        ];

        for (input, want) in cases {
            assert_eq!(clean_output(input), *want, "input {:?}", input);
        }
    }

    #[test]
    fn replaces_alt_screen() {
        let got = clean_output(b"before\x1b[?1049hjunk\x1b[?1049lafter");

        assert_eq!(got, format!("before{ALT_SCREEN_NOTE}after"));
    }

    #[test]
    fn replaces_unclosed_alt_screen() {
        let got = clean_output(b"before\x1b[?1049hjunk forever");

        assert_eq!(got, format!("before{ALT_SCREEN_NOTE}"));
    }

    #[test]
    fn truncates_middle_lines() {
        let input: Vec<String> = (0..250).map(|i| format!("l{i}")).collect();
        let got = truncate(
            &input.join("\n"),
            Trim {
                head: 100,
                tail: 100,
            },
        );
        let lines: Vec<&str> = got.split('\n').collect();

        assert_eq!(lines.len(), 201);
        assert_eq!(lines[100], "[... 50 lines omitted ...]");
        assert_eq!(lines[200], "l249");
    }

    #[test]
    fn keeps_short_output() {
        let input = "a\nb\nc\n";

        assert_eq!(truncate(input, Trim { head: 1, tail: 2 }), input);
    }
}
