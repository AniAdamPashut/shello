package main

import (
	"fmt"
	"regexp"
	"strings"
)

const altScreenNote = "[full-screen program output omitted]"

var (
	// Alternate screen (vim, less, htop...): from enter to leave, or to the
	// end of the output when the program never left it.
	altScreenRe = regexp.MustCompile(`(?s)\x1b\[\?(?:1049|1047|47)h.*?(?:\x1b\[\?(?:1049|1047|47)l|$)`)
	oscRe       = regexp.MustCompile(`\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)`)
	csiRe       = regexp.MustCompile(`\x1b\[[0-?]*[ -/]*[@-~]`)
	escRe       = regexp.MustCompile(`\x1b[@-_]`)
)

// CleanOutput turns raw terminal bytes into plain text: escape sequences are
// removed, carriage returns and backspaces are replayed the way a terminal
// would, and full-screen program output is replaced by a short note.
func CleanOutput(raw []byte) string {
	s := strings.ToValidUTF8(string(raw), "�")
	s = altScreenRe.ReplaceAllString(s, altScreenNote)
	s = oscRe.ReplaceAllString(s, "")
	s = csiRe.ReplaceAllString(s, "")
	s = escRe.ReplaceAllString(s, "")
	s = strings.ReplaceAll(s, "\r\n", "\n")

	var out strings.Builder
	for i, line := range strings.Split(s, "\n") {
		if i > 0 {
			out.WriteByte('\n')
		}
		out.WriteString(replayLine(line))
	}
	return out.String()
}

// replayLine applies \r (back to column 0) and \b (one column left) as
// overwrites, so progress bars collapse to their final state.
func replayLine(line string) string {
	if !strings.ContainsAny(line, "\r\b") {
		return line
	}
	var buf []rune
	col := 0
	for _, r := range line {
		switch r {
		case '\r':
			col = 0
		case '\b':
			if col > 0 {
				col--
			}
		default:
			if col < len(buf) {
				buf[col] = r
			} else {
				buf = append(buf, r)
			}
			col++
		}
	}
	return string(buf)
}

// Truncate keeps the first head and last tail lines of s, replacing the
// middle with a note saying how many lines were dropped.
func Truncate(s string, head, tail int) string {
	body, nl := strings.CutSuffix(s, "\n")
	lines := strings.Split(body, "\n")
	if len(lines) <= head+tail {
		return s
	}
	omitted := len(lines) - head - tail
	kept := append(lines[:head:head], fmt.Sprintf("[... %d lines omitted ...]", omitted))
	kept = append(kept, lines[len(lines)-tail:]...)
	out := strings.Join(kept, "\n")
	if nl {
		out += "\n"
	}
	return out
}
