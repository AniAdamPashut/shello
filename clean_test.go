package main

import (
	"fmt"
	"strings"
	"testing"
)

func TestCleanOutput(t *testing.T) {
	cases := []struct{ name, in, want string }{
		{"sgr colors", "a\x1b[31mred\x1b[0m\r\n", "ared\n"},
		{"crlf is newline", "line1\r\nline2\r\n", "line1\nline2\n"},
		{"progress bar", "10%\r50%\r100%\r\n", "100%\n"},
		{"backspace overwrite", "abc\b\bX\r\n", "aXc\n"},
		{"osc title", "x\x1b]0;title\x07y", "xy"},
		{"shello marker", "x\x1b]6973;end;1.1;0\x07y", "xy"},
		{"invalid utf8", "\xffok", "�ok"},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			if got := CleanOutput([]byte(c.in)); got != c.want {
				t.Errorf("CleanOutput(%q) = %q, want %q", c.in, got, c.want)
			}
		})
	}
}

func TestCleanOutputAltScreen(t *testing.T) {
	got := CleanOutput([]byte("before\x1b[?1049hjunk\x1b[?1049lafter"))
	if !strings.Contains(got, altScreenNote) || strings.Contains(got, "junk") {
		t.Errorf("alt screen not replaced: %q", got)
	}
	if !strings.HasPrefix(got, "before") || !strings.HasSuffix(got, "after") {
		t.Errorf("surrounding text lost: %q", got)
	}
}

func TestCleanOutputAltScreenUnclosed(t *testing.T) {
	got := CleanOutput([]byte("before\x1b[?1049hjunk forever"))
	if strings.Contains(got, "junk") || !strings.Contains(got, altScreenNote) {
		t.Errorf("unclosed alt screen not replaced: %q", got)
	}
}

func TestTruncate(t *testing.T) {
	var lines []string
	for i := 0; i < 250; i++ {
		lines = append(lines, fmt.Sprintf("l%d", i))
	}
	got := strings.Split(Truncate(strings.Join(lines, "\n"), 100, 100), "\n")
	if len(got) != 201 {
		t.Fatalf("got %d lines, want 201", len(got))
	}
	if got[100] != "[... 50 lines omitted ...]" {
		t.Errorf("line 101 = %q", got[100])
	}
	if got[200] != "l249" {
		t.Errorf("last line = %q", got[200])
	}
}

func TestTruncateShortUnchanged(t *testing.T) {
	in := "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\n"
	if got := Truncate(in, 100, 100); got != in {
		t.Errorf("Truncate changed short input: %q", got)
	}
}
