package main

import (
	"strings"
	"testing"
)

func TestFormatCommands(t *testing.T) {
	got := FormatCommands([]Command{{Cmd: "pactl list short sinks", Exit: 0, Output: "1\tsink\n", Available: true}})
	want := "$ pactl list short sinks\nexit 0\n```\n1\tsink\n```\n"
	if got != want {
		t.Errorf("got %q, want %q", got, want)
	}
}

func TestFormatCommandsSeparatedAndNewlineAdded(t *testing.T) {
	got := FormatCommands([]Command{
		{Cmd: "a", Output: "x"},
		{Cmd: "b", Exit: 1, Output: ""},
	})
	want := "$ a\nexit 0\n```\nx\n```\n\n$ b\nexit 1\n```\n```\n"
	if got != want {
		t.Errorf("got %q, want %q", got, want)
	}
}

func TestFormatCommandsEmpty(t *testing.T) {
	if got := FormatCommands(nil); got != "(no commands recorded in this terminal yet)\n" {
		t.Errorf("got %q", got)
	}
}

func TestBuildPromptDefaultQuestion(t *testing.T) {
	got := BuildPrompt("Q: {question}", nil, "", PromptEnv{})
	if !strings.Contains(got, defaultQuestion) {
		t.Errorf("got %q", got)
	}
}

func TestBuildPromptSubstitutes(t *testing.T) {
	cmds := []Command{{Cmd: "echo", Output: "{question} {shell}\n", Available: true}}
	got := BuildPrompt("{shell}|{cwd}|{os}|{question}|{commands}", cmds, "why?", PromptEnv{Shell: "bash", Cwd: "/tmp", OS: "Arch Linux"})
	want := "bash|/tmp|Arch Linux|why?|" + FormatCommands(cmds)
	if got != want {
		t.Errorf("got %q, want %q", got, want)
	}
}
