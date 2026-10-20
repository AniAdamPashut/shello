package main

import (
	"bytes"
	"reflect"
	"testing"
)

func TestBuildArgv(t *testing.T) {
	cases := []struct {
		tmpl      []string
		want      []string
		wantStdin bool
	}{
		{[]string{"claude", "{prompt}"}, []string{"claude", "hi"}, false},
		{[]string{"claude", "-p"}, []string{"claude", "-p"}, true},
		{[]string{"x", "--msg={prompt}"}, []string{"x", "--msg=hi"}, false},
	}
	for _, c := range cases {
		got, stdin := BuildArgv(c.tmpl, "hi")
		if !reflect.DeepEqual(got, c.want) || stdin != c.wantStdin {
			t.Errorf("BuildArgv(%v) = %v, %v; want %v, %v", c.tmpl, got, stdin, c.want, c.wantStdin)
		}
	}
}

func TestBuildArgvDoesNotModifyTemplate(t *testing.T) {
	tmpl := []string{"claude", "{prompt}"}
	BuildArgv(tmpl, "hi")
	if tmpl[1] != "{prompt}" {
		t.Error("template was modified")
	}
}

func TestRunPrintPipesPrompt(t *testing.T) {
	argv, stdin := BuildArgv([]string{"cat"}, "hello")
	var out bytes.Buffer
	if code := RunPrint(argv, "hello", stdin, &out); code != 0 || out.String() != "hello" {
		t.Errorf("code=%d out=%q", code, out.String())
	}
}

func TestRunPrintExitCode(t *testing.T) {
	var out bytes.Buffer
	if code := RunPrint([]string{"sh", "-c", "exit 3"}, "", true, &out); code != 3 {
		t.Errorf("code = %d, want 3", code)
	}
}

func TestRunPrintMissingAgent(t *testing.T) {
	var out bytes.Buffer
	if code := RunPrint([]string{"/nonexistent/agent"}, "", true, &out); code != 127 {
		t.Errorf("code = %d, want 127", code)
	}
}
