package main

import (
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

func TestLoadConfigMissingUsesDefaults(t *testing.T) {
	cfg, err := LoadConfig("/nonexistent/config.toml")
	if err != nil {
		t.Fatal(err)
	}
	if cfg.Agent != "claude" {
		t.Errorf("Agent = %q, want claude", cfg.Agent)
	}
	if got := cfg.Agents["claude"].Print; !reflect.DeepEqual(got, []string{"claude", "-p"}) {
		t.Errorf("claude print = %v", got)
	}
	if cfg.Capture.Commands != 3 {
		t.Errorf("Commands = %d, want 3", cfg.Capture.Commands)
	}
	if cfg.Capture.MaxLogBytes != 2097152 {
		t.Errorf("MaxLogBytes = %d, want 2097152", cfg.Capture.MaxLogBytes)
	}
}

func TestLoadConfigOverlay(t *testing.T) {
	path := filepath.Join(t.TempDir(), "config.toml")
	if err := os.WriteFile(path, []byte("agent = \"codex\"\n[capture]\ncommands = 5\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	cfg, err := LoadConfig(path)
	if err != nil {
		t.Fatal(err)
	}
	if cfg.Agent != "codex" || cfg.Capture.Commands != 5 || cfg.Capture.HeadLines != 100 {
		t.Errorf("overlay wrong: %+v", cfg)
	}
	if _, ok := cfg.Agents["claude"]; !ok {
		t.Error("default claude agent lost after overlay")
	}
}

func TestLoadPromptTemplateDefault(t *testing.T) {
	tmpl, err := LoadPromptTemplate("/nonexistent/prompt.md")
	if err != nil {
		t.Fatal(err)
	}
	for _, p := range []string{"{commands}", "{question}"} {
		if !strings.Contains(tmpl, p) {
			t.Errorf("default template missing %s", p)
		}
	}
}
