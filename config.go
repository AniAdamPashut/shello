package main

import (
	_ "embed"
	"errors"
	"io/fs"
	"os"
	"path/filepath"

	"github.com/BurntSushi/toml"
)

//go:embed config.toml
var defaultConfigTOML string

//go:embed prompt.md
var defaultPromptMD string

type AgentConfig struct {
	Interactive []string `toml:"interactive"`
	Print       []string `toml:"print"`
}

type CaptureConfig struct {
	Commands    int   `toml:"commands"`
	HeadLines   int   `toml:"head_lines"`
	TailLines   int   `toml:"tail_lines"`
	MaxLogBytes int64 `toml:"max_log_bytes"`
}

type Config struct {
	Agent   string                 `toml:"agent"`
	Agents  map[string]AgentConfig `toml:"agents"`
	Capture CaptureConfig          `toml:"capture"`
}

// ConfigDir returns $XDG_CONFIG_HOME/shello, falling back to ~/.config/shello.
func ConfigDir() string {
	if d := os.Getenv("XDG_CONFIG_HOME"); d != "" {
		return filepath.Join(d, "shello")
	}
	home, _ := os.UserHomeDir()
	return filepath.Join(home, ".config", "shello")
}

// LoadConfig returns the embedded defaults overlaid with the file at path.
// A missing file is not an error.
func LoadConfig(path string) (Config, error) {
	var cfg Config
	if err := toml.Unmarshal([]byte(defaultConfigTOML), &cfg); err != nil {
		return cfg, err
	}
	if _, err := toml.DecodeFile(path, &cfg); err != nil && !errors.Is(err, fs.ErrNotExist) {
		return cfg, err
	}
	return cfg, nil
}

// LoadPromptTemplate reads the template at path, or the embedded default if
// the file does not exist.
func LoadPromptTemplate(path string) (string, error) {
	b, err := os.ReadFile(path)
	if errors.Is(err, fs.ErrNotExist) {
		return defaultPromptMD, nil
	}
	return string(b), err
}
