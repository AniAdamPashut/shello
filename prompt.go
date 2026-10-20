package main

import (
	"bufio"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

const defaultQuestion = "Explain what happened in these commands and whether anything went wrong."

type PromptEnv struct {
	Shell, Cwd, OS string
}

// FormatCommands renders commands as "$ cmd", "exit N" and a fenced output
// block each, separated by blank lines.
func FormatCommands(cmds []Command) string {
	if len(cmds) == 0 {
		return "(no commands recorded in this terminal yet)\n"
	}
	blocks := make([]string, len(cmds))
	for i, c := range cmds {
		out := c.Output
		if out != "" && !strings.HasSuffix(out, "\n") {
			out += "\n"
		}
		blocks[i] = fmt.Sprintf("$ %s\nexit %d\n```\n%s```\n", c.Cmd, c.Exit, out)
	}
	return strings.Join(blocks, "\n")
}

// BuildPrompt fills the template's placeholders in a single pass, so text
// inside command output is never expanded.
func BuildPrompt(tmpl string, cmds []Command, question string, env PromptEnv) string {
	if strings.TrimSpace(question) == "" {
		question = defaultQuestion
	}
	return strings.NewReplacer(
		"{commands}", FormatCommands(cmds),
		"{question}", question,
		"{shell}", env.Shell,
		"{cwd}", env.Cwd,
		"{os}", env.OS,
	).Replace(tmpl)
}

// DetectEnv describes the user's environment for the prompt.
func DetectEnv() PromptEnv {
	env := PromptEnv{Shell: filepath.Base(os.Getenv("SHELL")), OS: "Linux"}
	env.Cwd, _ = os.Getwd()
	if f, err := os.Open("/etc/os-release"); err == nil {
		defer f.Close()
		sc := bufio.NewScanner(f)
		for sc.Scan() {
			if v, ok := strings.CutPrefix(sc.Text(), "PRETTY_NAME="); ok {
				env.OS = strings.Trim(v, `"`)
			}
		}
	}
	return env
}
