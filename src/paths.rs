use std::path::PathBuf;

/// Reads an environment variable, treating an empty value as unset.
pub fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

pub fn home_dir() -> PathBuf {
    PathBuf::from(env_var("HOME").unwrap_or_default())
}

/// $XDG_CONFIG_HOME/shello, falling back to ~/.config/shello.
pub fn config_dir() -> PathBuf {
    let base = env_var("XDG_CONFIG_HOME").map(PathBuf::from);
    base.unwrap_or_else(|| home_dir().join(".config"))
        .join("shello")
}

/// $XDG_STATE_HOME/shello/sessions, falling back to ~/.local/state/shello/sessions.
pub fn state_dir() -> PathBuf {
    let base = env_var("XDG_STATE_HOME").map(PathBuf::from);
    base.unwrap_or_else(|| home_dir().join(".local/state"))
        .join("shello/sessions")
}
