package main

import (
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"strings"
	"syscall"
)

const maxInteractivePrompt = 120000

// BuildArgv fills {prompt} into a copy of tmpl. When no element contains the
// placeholder, the prompt should go on stdin instead.
func BuildArgv(tmpl []string, prompt string) (argv []string, useStdin bool) {
	argv = make([]string, len(tmpl))
	useStdin = true
	for i, a := range tmpl {
		if strings.Contains(a, "{prompt}") {
			a = strings.ReplaceAll(a, "{prompt}", prompt)
			useStdin = false
		}
		argv[i] = a
	}
	return argv, useStdin
}

// RunPrint runs the agent once and returns its exit code (127 if it could
// not be started).
func RunPrint(argv []string, prompt string, useStdin bool, stdout io.Writer) int {
	cmd := exec.Command(argv[0], argv[1:]...)
	cmd.Stdout = stdout
	cmd.Stderr = os.Stderr
	if useStdin {
		cmd.Stdin = strings.NewReader(prompt)
	} else {
		cmd.Stdin = os.Stdin
	}
	err := cmd.Run()
	var exitErr *exec.ExitError
	switch {
	case err == nil:
		return 0
	case errors.As(err, &exitErr):
		return exitErr.ExitCode()
	default:
		fmt.Fprintf(os.Stderr, "shello: %v\n", err)
		return 127
	}
}

// ExecInteractive replaces this process with the agent, so it owns the
// terminal exactly as if the user had started it.
func ExecInteractive(argv []string) error {
	path, err := exec.LookPath(argv[0])
	if err != nil {
		return err
	}
	return syscall.Exec(path, argv, os.Environ())
}

func tooLargeForInteractive(prompt string) bool {
	return len(prompt) > maxInteractivePrompt
}
