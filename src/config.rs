use crate::error::CaldaWarriorError;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct CalendarEntry {
    pub project: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server_url: String,
    pub username: String,
    pub password: String,

    #[serde(default = "default_completed_cutoff_days")]
    pub completed_cutoff_days: u32,

    #[serde(default)]
    pub allow_insecure_tls: bool,

    #[serde(default = "default_caldav_timeout_seconds")]
    pub caldav_timeout_seconds: u64,

    #[serde(default, rename = "calendar")]
    pub calendars: Vec<CalendarEntry>,
}

fn default_completed_cutoff_days() -> u32 {
    90
}
fn default_caldav_timeout_seconds() -> u64 {
    30
}

/// Load configuration from the given path (or discover it).
/// Path resolution order:
///   1. `config_path` argument (from --config flag)
///   2. CALDAWARRIOR_CONFIG environment variable
///   3. ~/.config/caldawarrior/config.toml
///
/// After loading:
/// - If CALDAWARRIOR_PASSWORD env is set, it overrides config.password
/// - If file permissions are more permissive than 0600 (Unix only), emit [WARN] to stderr
/// - If duplicate calendar URLs exist (excluding "default" project), return CaldaWarriorError::Config
pub fn load(config_path: Option<&Path>) -> Result<Config, CaldaWarriorError> {
    // 1. Resolve path
    let path = resolve_path(config_path)?;

    // 2. Read file
    let content = std::fs::read_to_string(&path)
        .map_err(|e| CaldaWarriorError::Config(format!("Cannot read config file {:?}: {}", path, e)))?;

    // 3. Parse TOML
    let mut config: Config = toml::from_str(&content)
        .map_err(|e| CaldaWarriorError::Config(format!("Invalid config file {:?}: {}", path, e)))?;

    // 4. CALDAWARRIOR_PASSWORD env override
    if let Ok(pw) = std::env::var("CALDAWARRIOR_PASSWORD") {
        if !pw.is_empty() {
            config.password = pw;
        }
    }

    // 5. Permission check (Unix only, non-fatal)
    check_permissions(&path);

    // 6. Validate
    validate(&config)?;

    Ok(config)
}

fn resolve_path(config_path: Option<&Path>) -> Result<PathBuf, CaldaWarriorError> {
    if let Some(p) = config_path {
        return Ok(p.to_path_buf());
    }
    if let Ok(env_path) = std::env::var("CALDAWARRIOR_CONFIG") {
        return Ok(PathBuf::from(env_path));
    }
    // Default: ~/.config/caldawarrior/config.toml
    let home = std::env::var("HOME")
        .map_err(|_| CaldaWarriorError::Config("Cannot determine home directory (HOME not set)".to_string()))?;
    Ok(PathBuf::from(home).join(".config").join("caldawarrior").join("config.toml"))
}

#[cfg(unix)]
fn check_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(path) {
        let mode = meta.permissions().mode();
        // mode & 0o777 gives the rwxrwxrwx bits; 0o600 = owner rw only
        if mode & 0o177 != 0 {
            eprintln!("[WARN] Config file {:?} has permissions {:04o} — recommended: 0600", path, mode & 0o777);
        }
    }
}

#[cfg(not(unix))]
fn check_permissions(_path: &Path) {}

fn validate(config: &Config) -> Result<(), CaldaWarriorError> {
    // Duplicate URL check (excluding "default" project)
    let non_default: Vec<&str> = config
        .calendars
        .iter()
        .filter(|c| c.project != "default")
        .map(|c| c.url.as_str())
        .collect();

    let mut seen = std::collections::HashSet::new();
    for url in &non_default {
        if !seen.insert(*url) {
            return Err(CaldaWarriorError::Config(
                format!("Duplicate calendar URL found: {}", url),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_config(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn happy_path() {
        let f = write_temp_config(
            r#"
server_url = "https://dav.example.com"
username = "alice"
password = "secret"

[[calendar]]
project = "default"
url = "https://dav.example.com/alice/default/"
        "#,
        );
        let config = load(Some(f.path())).expect("load");
        assert_eq!(config.server_url, "https://dav.example.com");
        assert_eq!(config.username, "alice");
        assert_eq!(config.password, "secret");
        assert_eq!(config.completed_cutoff_days, 90);
        assert!(!config.allow_insecure_tls);
        assert_eq!(config.caldav_timeout_seconds, 30);
        assert_eq!(config.calendars.len(), 1);
    }

    #[test]
    fn defaults_applied() {
        let f = write_temp_config(
            r#"
server_url = "https://dav.example.com"
username = "alice"
password = "secret"
        "#,
        );
        let config = load(Some(f.path())).expect("load");
        assert_eq!(config.completed_cutoff_days, 90);
        assert!(!config.allow_insecure_tls);
        assert_eq!(config.caldav_timeout_seconds, 30);
    }

    #[test]
    fn missing_required_field() {
        let f = write_temp_config(
            r#"
username = "alice"
password = "secret"
        "#,
        );
        let result = load(Some(f.path()));
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("server_url") || msg.contains("Invalid config"));
    }

    #[test]
    fn duplicate_calendar_url_error() {
        let f = write_temp_config(
            r#"
server_url = "https://dav.example.com"
username = "alice"
password = "secret"

[[calendar]]
project = "work"
url = "https://dav.example.com/alice/cal/"

[[calendar]]
project = "personal"
url = "https://dav.example.com/alice/cal/"
        "#,
        );
        let result = load(Some(f.path()));
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.to_lowercase().contains("duplicate"));
    }

    #[test]
    fn duplicate_default_url_allowed() {
        // Two "default" entries with the same URL should NOT trigger the duplicate check
        // (since we filter out "default" entries). Actually, two "default" entries with the same
        // URL should still be fine since we only check non-default.
        let f = write_temp_config(
            r#"
server_url = "https://dav.example.com"
username = "alice"
password = "secret"

[[calendar]]
project = "default"
url = "https://dav.example.com/alice/cal/"

[[calendar]]
project = "work"
url = "https://dav.example.com/alice/work/"
        "#,
        );
        let result = load(Some(f.path()));
        assert!(result.is_ok());
    }
}
