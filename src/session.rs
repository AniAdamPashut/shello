use crate::paths::{env_var, state_dir};
use serde::Deserialize;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub const NOT_RECORDING: &str =
    "shello isn't recording this terminal — run install.sh and open a new terminal";

/// One line of a session's .cmds file, written by the shell hooks.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct CmdRecord {
    pub seq: String,
    pub cmd: String,
    /// The directory the command was typed in.
    #[serde(default)]
    pub cwd: String,
    pub exit: i32,
    /// The command was kept out of shell history (e.g. a leading space), so
    /// neither its text nor its output is shared.
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub ts: i64,
}

/// The SHELLO_* variables the shell hooks export.
#[derive(Debug, Clone, Default)]
pub struct SessionEnv {
    pub session: Option<String>,
    pub tty: Option<String>,
    pub dir: Option<PathBuf>,
}

/// The recording of one terminal: a log of everything it showed, and the
/// commands run in it.
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub dir: PathBuf,
    pub id: String,
}

impl Session {
    pub fn log_path(&self) -> PathBuf {
        self.dir.join(format!("{}.log", self.id))
    }

    pub fn cmds_path(&self) -> PathBuf {
        self.dir.join(format!("{}.cmds", self.id))
    }
}

impl SessionEnv {
    pub fn from_env() -> Self {
        Self {
            session: env_var("SHELLO_SESSION"),
            tty: env_var("SHELLO_TTY"),
            dir: env_var("SHELLO_DIR").map(PathBuf::from),
        }
    }
}

/// Finds the recording for the terminal on current_tty.
pub fn current_session(env: &SessionEnv, current_tty: &str) -> Result<Session, String> {
    let Some(id) = &env.session else {
        return Err(NOT_RECORDING.into());
    };

    if env.tty.as_deref() != Some(current_tty) {
        return Err(NOT_RECORDING.into());
    }

    let dir = env.dir.clone().unwrap_or_else(state_dir);
    let session = Session {
        dir,
        id: id.clone(),
    };
    if !session.cmds_path().is_file() {
        return Err(NOT_RECORDING.into());
    }

    Ok(session)
}

/// Parses a .cmds file, skipping lines that aren't valid records.
pub fn read_cmds(path: &Path) -> Result<Vec<CmdRecord>, String> {
    let bytes = std::fs::read(path).map_err(|e| describe(path, e))?;
    let text = String::from_utf8_lossy(&bytes);

    Ok(text.lines().filter_map(parse_record).collect())
}

fn parse_record(line: &str) -> Option<CmdRecord> {
    serde_json::from_str::<CmdRecord>(line)
        .ok()
        .filter(|r| !r.seq.is_empty())
}

/// Returns at most the last max_bytes bytes of the file at path.
pub fn read_log_tail(path: &Path, max_bytes: u64) -> Result<Vec<u8>, String> {
    tail_bytes(path, max_bytes).map_err(|e| describe(path, e))
}

fn tail_bytes(path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(max_bytes)))?;

    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    Ok(buf)
}

fn describe(path: &Path, e: io::Error) -> String {
    format!("{}: {e}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{temp_dir, temp_file};

    fn env(session: &str, tty: &str, dir: &Path) -> SessionEnv {
        SessionEnv {
            session: Some(session.into()),
            tty: Some(tty.into()),
            dir: Some(dir.into()),
        }
    }

    #[test]
    fn unset_session_is_not_recording() {
        let err = current_session(&SessionEnv::default(), "/dev/pts/3").unwrap_err();

        assert_eq!(err, NOT_RECORDING);
    }

    #[test]
    fn other_tty_is_not_recording() {
        let dir = temp_dir();
        std::fs::write(dir.join("abc.cmds"), "").unwrap();

        assert!(current_session(&env("abc", "/dev/pts/9", &dir), "/dev/pts/3").is_err());
    }

    #[test]
    fn missing_files_are_not_recording() {
        assert!(current_session(&env("abc", "/dev/pts/3", &temp_dir()), "/dev/pts/3").is_err());
    }

    #[test]
    fn finds_session_files() {
        let dir = temp_dir();
        std::fs::write(dir.join("abc.cmds"), "").unwrap();
        let s = current_session(&env("abc", "/dev/pts/3", &dir), "/dev/pts/3").unwrap();

        assert_eq!(s.cmds_path(), dir.join("abc.cmds"));
        assert_eq!(s.log_path(), dir.join("abc.log"));
    }

    #[test]
    fn read_cmds_skips_bad_lines() {
        let data = concat!(
            r#"{"seq":"1.1","cmd":"ls","exit":0,"ts":1}"#,
            "\ngarbage\n",
            r#"{"seq":"1.2","cmd":"q\"x","cwd":"/tmp","exit":2,"hidden":true,"ts":2}"#,
            "\n"
        );
        let recs = read_cmds(&temp_file("s.cmds", data.as_bytes())).unwrap();

        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].cwd, "");
        assert!(!recs[0].hidden);
        assert_eq!(recs[1].cmd, r#"q"x"#);
        assert_eq!(recs[1].exit, 2);
        assert_eq!(recs[1].cwd, "/tmp");
        assert!(recs[1].hidden);
    }

    #[test]
    fn read_log_tail_keeps_last_bytes() {
        let path = temp_file("s.log", b"0123456789");

        assert_eq!(read_log_tail(&path, 4).unwrap(), b"6789");
        assert_eq!(read_log_tail(&path, 100).unwrap(), b"0123456789");
    }
}
