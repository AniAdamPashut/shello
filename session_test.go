package main

import (
	"os"
	"path/filepath"
	"reflect"
	"strconv"
	"testing"
)

func start(seq string) string         { return "\x1b]6973;start;" + seq + "\x07" }
func end(seq string, code int) string { return "\x1b]6973;end;" + seq + ";" + itoa(code) + "\x07" }
func itoa(i int) string               { return strconv.Itoa(i) }

func TestExtractOutput(t *testing.T) {
	log := "$ ls\r\n" + start("1.1") + "a\r\nb\r\n" + end("1.1", 0)
	got, ok := ExtractOutput([]byte(log), "1.1")
	if !ok || string(got) != "a\r\nb\r\n" {
		t.Errorf("got %q, %v", got, ok)
	}
}

func TestExtractOutputLastStartWins(t *testing.T) {
	log := start("1.2") + "stale" + start("1.2") + "real" + end("1.2", 0)
	got, ok := ExtractOutput([]byte(log), "1.2")
	if !ok || string(got) != "real" {
		t.Errorf("got %q, %v", got, ok)
	}
}

func TestExtractOutputMissingStart(t *testing.T) {
	if _, ok := ExtractOutput([]byte("tail-only"+end("1.3", 1)), "1.3"); ok {
		t.Error("expected ok=false when start marker is missing")
	}
}

func TestExtractOutputDoesNotMatchSeqPrefix(t *testing.T) {
	log := start("1.1") + "one" + end("1.1", 0) + start("1.10") + "ten" + end("1.10", 0)
	got, ok := ExtractOutput([]byte(log), "1.1")
	if !ok || string(got) != "one" {
		t.Errorf("got %q, %v", got, ok)
	}
}

func recs(cmds ...string) []CmdRecord {
	var r []CmdRecord
	for i, c := range cmds {
		r = append(r, CmdRecord{Seq: "1." + itoa(i+1), Cmd: c})
	}
	return r
}

func TestLastCommandsExcludesShello(t *testing.T) {
	r := recs("ls", "pactl list", "shello hi", "/home/u/.local/bin/shello -p x", "echo")
	var got []string
	for _, c := range LastCommands(r, nil, 3, 100, 100) {
		got = append(got, c.Cmd)
	}
	if want := []string{"ls", "pactl list", "echo"}; !reflect.DeepEqual(got, want) {
		t.Errorf("got %v, want %v", got, want)
	}
}

func TestLastCommandsFewer(t *testing.T) {
	if got := LastCommands(recs("ls"), nil, 3, 100, 100); len(got) != 1 {
		t.Errorf("got %d commands, want 1", len(got))
	}
}

func TestLastCommandsMissingOutput(t *testing.T) {
	got := LastCommands(recs("ls"), []byte("nothing here"), 3, 100, 100)
	if got[0].Available || got[0].Output != "[output not available]" {
		t.Errorf("got %+v", got[0])
	}
}

func TestLastCommandsEmptyInterrupted(t *testing.T) {
	r := []CmdRecord{{Seq: "1.5", Cmd: "sleep 5", Exit: 130}}
	got := LastCommands(r, []byte(start("1.5")+end("1.5", 130)), 3, 100, 100)
	if !got[0].Available || got[0].Output != "" || got[0].Exit != 130 {
		t.Errorf("got %+v", got[0])
	}
}

func TestLastCommandsCleansAndTruncates(t *testing.T) {
	r := []CmdRecord{{Seq: "1.1", Cmd: "x"}}
	log := start("1.1") + "\x1b[1mA\x1b[0m\r\nB\r\nC\r\nD\r\n" + end("1.1", 0)
	got := LastCommands(r, []byte(log), 3, 1, 1)
	if want := "A\n[... 2 lines omitted ...]\nD\n"; got[0].Output != want {
		t.Errorf("got %q, want %q", got[0].Output, want)
	}
}

func TestReadCmdsSkipsBadLines(t *testing.T) {
	path := filepath.Join(t.TempDir(), "s.cmds")
	data := `{"seq":"1.1","cmd":"ls","exit":0,"ts":1}` + "\ngarbage\n" + `{"seq":"1.2","cmd":"q\"x","exit":2,"ts":2}` + "\n"
	os.WriteFile(path, []byte(data), 0o600)
	got, err := ReadCmds(path)
	if err != nil {
		t.Fatal(err)
	}
	if len(got) != 2 || got[1].Cmd != `q"x` || got[1].Exit != 2 {
		t.Errorf("got %+v", got)
	}
}

func TestReadLogTail(t *testing.T) {
	path := filepath.Join(t.TempDir(), "s.log")
	os.WriteFile(path, []byte("0123456789"), 0o600)
	got, err := ReadLogTail(path, 4)
	if err != nil || string(got) != "6789" {
		t.Errorf("got %q, %v", got, err)
	}
	got, _ = ReadLogTail(path, 100)
	if string(got) != "0123456789" {
		t.Errorf("small file: got %q", got)
	}
}

func TestCurrentSessionUnset(t *testing.T) {
	t.Setenv("SHELLO_SESSION", "")
	_, err := CurrentSession("/dev/pts/3")
	if err == nil || err.Error() != notRecordingMsg {
		t.Errorf("err = %v", err)
	}
}

func TestCurrentSessionOtherTTY(t *testing.T) {
	t.Setenv("SHELLO_SESSION", "abc")
	t.Setenv("SHELLO_TTY", "/dev/pts/9")
	if _, err := CurrentSession("/dev/pts/3"); err == nil {
		t.Error("expected error for a different tty")
	}
}

func TestCurrentSessionOK(t *testing.T) {
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "abc.cmds"), nil, 0o600)
	t.Setenv("SHELLO_SESSION", "abc")
	t.Setenv("SHELLO_TTY", "/dev/pts/3")
	t.Setenv("SHELLO_DIR", dir)
	s, err := CurrentSession("/dev/pts/3")
	if err != nil {
		t.Fatal(err)
	}
	if s.CmdsPath() != filepath.Join(dir, "abc.cmds") || s.LogPath() != filepath.Join(dir, "abc.log") {
		t.Errorf("paths: %s %s", s.CmdsPath(), s.LogPath())
	}
}
