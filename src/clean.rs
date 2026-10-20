use regex::{Captures, NoExpand, Regex};
use std::borrow::Cow;
use std::ops::Range;
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

/// Erase-line sequences (ESC[K, ESC[1K, ESC[2K). They're kept as
/// placeholder characters until the line is replayed, since what they erase
/// depends on where the cursor is.
static ERASE_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\x1b\[([012]?)K").unwrap());
const ERASE_TO_END: char = '\u{E000}';
const ERASE_TO_START: char = '\u{E001}';
const ERASE_ALL: char = '\u{E002}';
const CURSOR_CONTROLS: [char; 5] = ['\r', '\x08', ERASE_TO_END, ERASE_TO_START, ERASE_ALL];

/// Turns raw terminal bytes into the plain text a person would have seen.
pub fn clean_output(raw: &[u8]) -> String {
    let text = String::from_utf8_lossy(raw);
    let text = ALT_SCREEN.replace_all(&text, NoExpand(ALT_SCREEN_NOTE));
    let text = mark_erases(&text);
    let text = ESCAPES.replace_all(&text, "").replace("\r\n", "\n");

    let lines: Vec<String> = text.split('\n').map(replay_line).collect();

    drop_padding(&lines)
}

/// Removes the trailing spaces terminals pad lines with, and blank lines at
/// the end (zsh's partial-line marker leaves one after every command).
fn drop_padding(lines: &[String]) -> String {
    let trimmed: Vec<&str> = lines.iter().map(|l| l.trim_end()).collect();
    let mut text = trimmed.join("\n");
    while text.ends_with("\n\n") {
        text.pop();
    }

    text
}

fn mark_erases(text: &str) -> Cow<'_, str> {
    let mark = |caps: &Captures| match &caps[1] {
        "1" => ERASE_TO_START.to_string(),
        "2" => ERASE_ALL.to_string(),
        _ => ERASE_TO_END.to_string(),
    };

    ERASE_LINE.replace_all(text, mark)
}

/// Redraws a line the way a terminal would, so progress bars collapse to
/// their final state.
fn replay_line(line: &str) -> String {
    if !line.contains(CURSOR_CONTROLS) {
        return line.to_string();
    }

    let mut cursor = Cursor::default();
    line.chars().for_each(|c| cursor.apply(c));

    cursor.buf.into_iter().collect()
}

/// One terminal line and the cursor's column on it.
#[derive(Default)]
struct Cursor {
    buf: Vec<char>,
    col: usize,
}

impl Cursor {
    fn apply(&mut self, c: char) {
        match c {
            '\r' => self.col = 0,
            '\x08' => self.col = self.col.saturating_sub(1),
            ERASE_TO_END => self.buf.truncate(self.col),
            ERASE_TO_START => self.blank(0..(self.col + 1).min(self.buf.len())),
            ERASE_ALL => self.blank(0..self.buf.len()),
            _ => self.put(c),
        }
    }

    fn blank(&mut self, range: Range<usize>) {
        self.buf[range].fill(' ');
    }

    fn put(&mut self, c: char) {
        if self.col < self.buf.len() {
            self.buf[self.col] = c;
        } else {
            self.buf.resize(self.col, ' ');
            self.buf.push(c);
        }

        self.col += 1;
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
    fn applies_erase_line() {
        let cases: &[(&[u8], &str)] = &[
            (b"Downloading foo 50%\r\x1b[KDone\r\n", "Done\n"),
            (b"Downloading foo 50%\r\x1b[0KDone\r\n", "Done\n"),
            (b"abcdef\r\x1b[2Kxy\r\n", "xy\n"),
            (b"abcdef\x08\x08\x08\x1b[K\r\n", "abc\n"),
            (b"abcdef\x08\x08\x1b[1KX\r\n", "    Xf\n"),
        ];

        for (input, want) in cases {
            assert_eq!(clean_output(input), *want, "input {:?}", input);
        }
    }

    #[test]
    fn drops_trailing_padding() {
        let zsh_prompt_sp = b"first-run\r\n\x1b[1m\x1b[7m%\x1b[27m\x1b[1m\x1b[0m        \r \r";

        assert_eq!(clean_output(zsh_prompt_sp), "first-run\n");
        assert_eq!(clean_output(b"a  \r\nb\r\n\r\n\r\n"), "a\nb\n");
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
