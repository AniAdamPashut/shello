package main

import (
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

type Options struct {
	Print, Show bool
	N           int
	Agent       string
	Question    string
}

const usage = `usage: shello [options] [question...]

Sends your last commands in this terminal, with their output, to an AI agent.

  -p, --print        single answer, then exit (default: conversation)
  -n, --commands N   how many past commands to include (default 3)
  -a, --agent NAME   pick a configured agent
      --show         print the built prompt and exit; call no agent
`

// ParseArgs parses flags up to the first non-flag; the rest is the question.
func ParseArgs(args []string) (Options, error) {
	var o Options
	fs := flag.NewFlagSet("shello", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	fs.BoolVar(&o.Print, "p", false, "")
	fs.BoolVar(&o.Print, "print", false, "")
	fs.IntVar(&o.N, "n", 0, "")
	fs.IntVar(&o.N, "commands", 0, "")
	fs.StringVar(&o.Agent, "a", "", "")
	fs.StringVar(&o.Agent, "agent", "", "")
	fs.BoolVar(&o.Show, "show", false, "")
	if err := fs.Parse(args); err != nil {
		return o, err
	}
	if o.N < 0 || (o.N == 0 && flagSet(fs, "n", "commands")) {
		return o, errors.New("-n must be at least 1")
	}
	o.Question = strings.Join(fs.Args(), " ")
	return o, nil
}

func flagSet(fs *flag.FlagSet, names ...string) bool {
	found := false
	fs.Visit(func(f *flag.Flag) {
		for _, n := range names {
			if f.Name == n {
				found = true
			}
		}
	})
	return found
}

func currentTTY() string {
	tty, _ := os.Readlink("/proc/self/fd/0")
	return tty
}

func run(args []string) int {
	opts, err := ParseArgs(args)
	if errors.Is(err, flag.ErrHelp) {
		fmt.Print(usage)
		return 0
	}
	if err != nil {
		fmt.Fprintf(os.Stderr, "shello: %v\n\n%s", err, usage)
		return 2
	}
	cfg, err := LoadConfig(filepath.Join(ConfigDir(), "config.toml"))
	if err != nil {
		fmt.Fprintf(os.Stderr, "shello: config: %v\n", err)
		return 1
	}
	tmpl, err := LoadPromptTemplate(filepath.Join(ConfigDir(), "prompt.md"))
	if err != nil {
		fmt.Fprintf(os.Stderr, "shello: prompt template: %v\n", err)
		return 1
	}

	sess, err := CurrentSession(currentTTY())
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	recs, err := ReadCmds(sess.CmdsPath())
	if err != nil {
		fmt.Fprintf(os.Stderr, "shello: %v\n", err)
		return 1
	}
	log, err := ReadLogTail(sess.LogPath(), cfg.Capture.MaxLogBytes)
	if err != nil {
		fmt.Fprintf(os.Stderr, "shello: %v\n", err)
		return 1
	}
	n := opts.N
	if n == 0 {
		n = cfg.Capture.Commands
	}
	cmds := LastCommands(recs, log, n, cfg.Capture.HeadLines, cfg.Capture.TailLines)
	prompt := BuildPrompt(tmpl, cmds, opts.Question, DetectEnv())

	name := opts.Agent
	if name == "" {
		name = cfg.Agent
	}
	agent, ok := cfg.Agents[name]
	tmplArgv := agent.Interactive
	if opts.Print {
		tmplArgv = agent.Print
	}
	if opts.Show {
		fmt.Print(prompt)
		if ok && len(tmplArgv) > 0 {
			fmt.Fprintf(os.Stderr, "agent: %q\n", tmplArgv)
		}
		return 0
	}
	if !ok {
		fmt.Fprintf(os.Stderr, "shello: unknown agent %q\n", name)
		return 2
	}
	if len(tmplArgv) == 0 {
		fmt.Fprintf(os.Stderr, "shello: agent %q has no command configured for this mode\n", name)
		return 2
	}

	argv, useStdin := BuildArgv(tmplArgv, prompt)
	if opts.Print {
		return RunPrint(argv, prompt, useStdin, os.Stdout)
	}
	if tooLargeForInteractive(prompt) {
		fmt.Fprintln(os.Stderr, "prompt too large for interactive mode; use -p or a smaller -n")
		return 1
	}
	if useStdin {
		fmt.Fprintf(os.Stderr, "shello: agent %q interactive command needs a {prompt} placeholder\n", name)
		return 2
	}
	err = ExecInteractive(argv)
	fmt.Fprintf(os.Stderr, "shello: %v\n", err)
	return 127
}

func main() {
	os.Exit(run(os.Args[1:]))
}
