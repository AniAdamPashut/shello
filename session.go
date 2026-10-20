package main

import (
	"bufio"
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"path/filepath"
	"strings"
)

const (
	notRecordingMsg  = "shello isn't recording this terminal — run install.sh and open a new terminal"
	unavailableNote  = "[output not available]"
	markerStartPfx   = "\x1b]6973;start;"
	markerEndPfx     = "\x1b]6973;end;"
	maxCmdsLineBytes = 1 << 20
)

// CmdRecord is one line of a session's .cmds file, written by the shell hooks.
type CmdRecord struct {
	Seq  string `json:"seq"`
	Cmd  string `json:"cmd"`
	Exit int    `json:"exit"`
	TS   int64  `json:"ts"`
}

// Command is a past command ready to be put in the prompt.
type Command struct {
	Cmd       string
	Exit      int
	Output    string
	Available bool
}

type Session struct {
	Dir, ID string
}

func (s Session) LogPath() string  { return filepath.Join(s.Dir, s.ID+".log") }
func (s Session) CmdsPath() string { return filepath.Join(s.Dir, s.ID+".cmds") }

// StateDir returns $XDG_STATE_HOME/shello/sessions, falling back to
// ~/.local/state/shello/sessions.
func StateDir() string {
	base := os.Getenv("XDG_STATE_HOME")
	if base == "" {
		home, _ := os.UserHomeDir()
		base = filepath.Join(home, ".local", "state")
	}
	return filepath.Join(base, "shello", "sessions")
}

// CurrentSession finds the recording for the terminal on currentTTY.
func CurrentSession(currentTTY string) (Session, error) {
	id := os.Getenv("SHELLO_SESSION")
	if id == "" || os.Getenv("SHELLO_TTY") != currentTTY {
		return Session{}, errors.New(notRecordingMsg)
	}
	dir := os.Getenv("SHELLO_DIR")
	if dir == "" {
		dir = StateDir()
	}
	s := Session{Dir: dir, ID: id}
	if _, err := os.Stat(s.CmdsPath()); err != nil {
		return Session{}, errors.New(notRecordingMsg)
	}
	return s, nil
}

// ReadCmds parses a .cmds file, skipping lines that aren't valid records.
func ReadCmds(path string) ([]CmdRecord, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	var recs []CmdRecord
	sc := bufio.NewScanner(f)
	sc.Buffer(make([]byte, 64*1024), maxCmdsLineBytes)
	for sc.Scan() {
		var r CmdRecord
		if json.Unmarshal(sc.Bytes(), &r) == nil && r.Seq != "" {
			recs = append(recs, r)
		}
	}
	return recs, sc.Err()
}

// ReadLogTail returns at most the last maxBytes bytes of the file at path.
func ReadLogTail(path string, maxBytes int64) ([]byte, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	info, err := f.Stat()
	if err != nil {
		return nil, err
	}
	if off := info.Size() - maxBytes; off > 0 {
		if _, err := f.Seek(off, io.SeekStart); err != nil {
			return nil, err
		}
	}
	return io.ReadAll(f)
}

// ExtractOutput returns the bytes between the last end marker for seq and
// the last start marker for seq before it. Empty prompt lines can leave a
// stale start marker with the same seq, which is why the last one wins.
func ExtractOutput(log []byte, seq string) ([]byte, bool) {
	e := bytes.LastIndex(log, []byte(markerEndPfx+seq+";"))
	if e < 0 {
		return nil, false
	}
	startMarker := []byte(markerStartPfx + seq + "\x07")
	s := bytes.LastIndex(log[:e], startMarker)
	if s < 0 {
		return nil, false
	}
	return log[s+len(startMarker) : e], true
}

// IsShelloCmd reports whether cmd is an invocation of shello itself.
func IsShelloCmd(cmd string) bool {
	f := strings.Fields(cmd)
	return len(f) > 0 && filepath.Base(f[0]) == "shello"
}

// LastCommands returns the last n non-shello commands, oldest first, with
// their cleaned and truncated output.
func LastCommands(recs []CmdRecord, log []byte, n, head, tail int) []Command {
	var picked []CmdRecord
	for i := len(recs) - 1; i >= 0 && len(picked) < n; i-- {
		if !IsShelloCmd(recs[i].Cmd) {
			picked = append(picked, recs[i])
		}
	}
	cmds := make([]Command, 0, len(picked))
	for i := len(picked) - 1; i >= 0; i-- {
		r := picked[i]
		c := Command{Cmd: r.Cmd, Exit: r.Exit, Output: unavailableNote}
		if raw, ok := ExtractOutput(log, r.Seq); ok {
			c.Output = Truncate(CleanOutput(raw), head, tail)
			c.Available = true
		}
		cmds = append(cmds, c)
	}
	return cmds
}
