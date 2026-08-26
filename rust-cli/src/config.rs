use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::crypto::generate_key;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub version: u32,
    pub repo: String,
    pub github_token: String,
    pub dashboard_key: String,
    pub device_id: String,
    pub device_name: String,
    pub interval_minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JoinCode {
    version: u32,
    repo: String,
    dashboard_key: String,
    interval_minutes: u32,
}

pub fn config_dir() -> Result<PathBuf> {
    dirs::config_dir()
        .map(|path| path.join("token-monitor"))
        .context("cannot determine the user configuration directory")
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.json"))
}

pub fn ledger_cache_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("ledger-cache.json"))
}

pub fn normalize_repo(value: &str) -> Result<String> {
    let repo = value
        .trim()
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .strip_prefix("https://github.com/")
        .unwrap_or(value.trim().trim_end_matches('/').trim_end_matches(".git"))
        .to_string();
    let parts: Vec<&str> = repo.split('/').collect();
    if parts.len() != 2 || parts.iter().any(|part| part.trim().is_empty()) {
        bail!("repository must be OWNER/REPO or a github.com repository URL");
    }
    Ok(repo)
}

pub fn default_device_name() -> String {
    hostname::get()
        .ok()
        .map(|value| value.to_string_lossy().trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("{}-device", std::env::consts::OS))
}

pub fn sanitize_device_id(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut last_dash = false;
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() { "device".to_string() } else { trimmed.to_string() }
}

pub fn new_config(repo: &str, token: String, device_name: Option<String>, interval_minutes: u32) -> Result<Config> {
    let name = device_name.filter(|v| !v.trim().is_empty()).unwrap_or_else(default_device_name);
    Ok(Config {
        version: 2,
        repo: normalize_repo(repo)?,
        github_token: token,
        dashboard_key: generate_key(),
        device_id: sanitize_device_id(&name),
        device_name: name,
        interval_minutes: interval_minutes.clamp(5, 1440),
    })
}

pub fn from_join(code: &str, token: String, device_name: Option<String>) -> Result<Config> {
    let bytes = URL_SAFE_NO_PAD.decode(code.trim()).context("invalid join code")?;
    let join: JoinCode = serde_json::from_slice(&bytes).context("invalid join code payload")?;
    if join.version != 2 { bail!("unsupported join-code version {}", join.version); }
    crate::crypto::decode_key(&join.dashboard_key)?;
    let name = device_name.filter(|v| !v.trim().is_empty()).unwrap_or_else(default_device_name);
    Ok(Config {
        version: 2,
        repo: normalize_repo(&join.repo)?,
        github_token: token,
        dashboard_key: join.dashboard_key,
        device_id: sanitize_device_id(&name),
        device_name: name,
        interval_minutes: join.interval_minutes.clamp(5, 1440),
    })
}

pub fn join_code(config: &Config) -> Result<String> {
    let join = JoinCode {
        version: 2,
        repo: config.repo.clone(),
        dashboard_key: config.dashboard_key.clone(),
        interval_minutes: config.interval_minutes,
    };
    Ok(URL_SAFE_NO_PAD.encode(serde_json::to_vec(&join)?))
}

pub fn dashboard_url(config: &Config) -> String {
    let mut parts = config.repo.split('/');
    let owner = parts.next().unwrap_or_default();
    let repo = parts.next().unwrap_or_default();
    if repo.eq_ignore_ascii_case(&format!("{owner}.github.io")) {
        format!("https://{owner}.github.io/usage/#key={}", config.dashboard_key)
    } else {
        format!("https://{owner}.github.io/{repo}/usage/#key={}", config.dashboard_key)
    }
}

pub fn load() -> Result<Config> {
    let path = config_path()?;
    let data = fs::read(&path).with_context(|| format!("configuration not found at {}. Run `token-monitor setup` first", path.display()))?;
    let config: Config = serde_json::from_slice(&data).context("invalid token-monitor configuration")?;
    if config.version != 2 { bail!("unsupported config version {}", config.version); }
    crate::crypto::decode_key(&config.dashboard_key)?;
    Ok(config)
}

pub fn save(config: &Config) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
    write_private(&path, &serde_json::to_vec_pretty(config)?)?;
    Ok(())
}

pub fn read_cached_ledger() -> Result<Option<crate::model::Ledger>> {
    let path = ledger_cache_path()?;
    match fs::read(&path) {
        Ok(data) => Ok(Some(serde_json::from_slice(&data).context("invalid local ledger cache")?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub fn write_cached_ledger(ledger: &crate::model::Ledger) -> Result<()> {
    let path = ledger_cache_path()?;
    if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
    write_private(&path, &serde_json::to_vec(ledger)?)
}

fn write_private(path: &Path, data: &[u8]) -> Result<()> {
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(data)?;
        file.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        fs::write(&tmp, data)?;
    }
    fs::rename(&tmp, path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}
