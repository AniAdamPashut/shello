package main

import (
	"strings"
	"testing"
)

func TestParseArgs(t *testing.T) {
	cases := []struct {
		args []string
		want Options
	}{
		{[]string{"-p", "how", "do", "I"}, Options{Print: true, Question: "how do I"}},
		{[]string{"--print", "x"}, Options{Print: true, Question: "x"}},
		{[]string{"-n", "5", "--show"}, Options{N: 5, Show: true}},
		{[]string{"--commands", "2", "q"}, Options{N: 2, Question: "q"}},
		{[]string{"--agent", "codex", "q"}, Options{Agent: "codex", Question: "q"}},
		{[]string{"-a", "gemini"}, Options{Agent: "gemini"}},
		{[]string{}, Options{}},
		{[]string{"why", "-p"}, Options{Question: "why -p"}},
	}
	for _, c := range cases {
		got, err := ParseArgs(c.args)
		if err != nil {
			t.Errorf("ParseArgs(%v): %v", c.args, err)
			continue
		}
		if got != c.want {
			t.Errorf("ParseArgs(%v) = %+v, want %+v", c.args, got, c.want)
		}
	}
}

func TestParseArgsRejectsBadN(t *testing.T) {
	if _, err := ParseArgs([]string{"-n", "0"}); err == nil {
		t.Error("expected error for -n 0")
	}
}

func TestRunNotRecording(t *testing.T) {
	t.Setenv("SHELLO_SESSION", "")
	t.Setenv("XDG_CONFIG_HOME", t.TempDir())
	if code := run([]string{"--show", "hi"}); code != 1 {
		t.Errorf("code = %d, want 1", code)
	}
}

func TestTooLargeForInteractive(t *testing.T) {
	if !tooLargeForInteractive(strings.Repeat("x", 120001)) || tooLargeForInteractive("small") {
		t.Error("size limit check wrong")
	}
}
