use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::Value;
use walkdir::WalkDir;

use crate::provider::{route_hint_from_base_url, RouteHint};

#[derive(Debug, Clone, Default)]
pub struct SessionEvidence {
    pub explicit_provider: Option<String>,
    pub route_hint: Option<RouteHint>,
    /// Service-tier evidence keyed by the same YYYY-MM-DD day key used by the
    /// normalized ledger. This avoids turning a long session into `mixed` merely
    /// because the user changed tiers on another day.
    pub tiers_by_date: HashMap<String, String>,
    /// Used only for tier-bearing records that genuinely have no usable date.
    pub fallback_tier: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct EvidenceBundle {
    pub sessions: HashMap<(String, String), SessionEvidence>,
    pub provider_hints: HashMap<String, RouteHint>,
}

fn home() -> Option<PathBuf> { dirs::home_dir() }

fn recent_enough(path: &Path, incremental: bool) -> bool {
    if !incremental { return true; }
    let Ok(modified) = path.metadata().and_then(|m| m.modified()) else { return true; };
    modified >= SystemTime::now()
        .checked_sub(Duration::from_secs(5 * 24 * 3600))
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

fn jsonl_files(root: &Path, incremental: bool) -> impl Iterator<Item = PathBuf> + '_ {
    WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(move |path| path.extension().and_then(|v| v.to_str()) == Some("jsonl") && recent_enough(path, incremental))
}

fn get_string<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .filter(|v| !v.trim().is_empty())
}

fn collect_keyed_strings(value: &Value, keys: &[&str], out: &mut HashSet<String>, depth: usize) {
    if depth > 6 { return; }
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if keys.iter().any(|candidate| key == candidate) {
                    if let Some(text) = child.as_str().filter(|v| !v.trim().is_empty()) {
                        out.insert(text.trim().to_ascii_lowercase());
                    }
                }
                collect_keyed_strings(child, keys, out, depth + 1);
            }
        }
        Value::Array(items) => {
            for child in items { collect_keyed_strings(child, keys, out, depth + 1); }
        }
        _ => {}
    }
}

fn canonical_tier(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "priority" | "fast" => "fast".to_string(),
        "default" | "standard" => "standard".to_string(),
        "flex" => "flex".to_string(),
        other => other.to_string(),
    }
}

fn normalize_tiers(tiers: HashSet<String>) -> Option<String> {
    let canonical: HashSet<String> = tiers.into_iter().map(|tier| canonical_tier(&tier)).collect();
    if canonical.is_empty() { return None; }
    if canonical.len() > 1 { return Some("mixed".to_string()); }
    canonical.into_iter().next()
}

fn date_from_timestamp(value: &str) -> Option<String> {
    let date = value.get(..10)?;
    let bytes = date.as_bytes();
    let valid = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes.iter().enumerate().all(|(index, byte)| {
            matches!(index, 4 | 7) || byte.is_ascii_digit()
        });
    valid.then(|| date.to_string())
}

fn codex_provider_hints(home: &Path) -> HashMap<String, RouteHint> {
    let codex_home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".codex"));
    let Ok(text) = std::fs::read_to_string(codex_home.join("config.toml")) else { return HashMap::new(); };
    let Ok(value) = text.parse::<toml::Value>() else { return HashMap::new(); };
    let Some(table) = value.get("model_providers").and_then(toml::Value::as_table) else { return HashMap::new(); };
    table.iter().filter_map(|(id, config)| {
        let base_url = config.get("base_url").and_then(toml::Value::as_str)
            .or_else(|| config.get("baseUrl").and_then(toml::Value::as_str))?;
        Some((id.to_ascii_lowercase(), route_hint_from_base_url(id, base_url)))
    }).collect()
}

fn scan_codex_file(path: &Path, bundle: &mut EvidenceBundle) {
    let Ok(file) = File::open(path) else { return; };
    let mut session_id: Option<String> = None;
    let mut provider: Option<String> = None;
    let mut tiers_by_date: HashMap<String, HashSet<String>> = HashMap::new();
    let mut undated_tiers = HashSet::new();

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if !line.contains("session_meta") && !line.contains("service_tier") && !line.contains("serviceTier") {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(&line) else { continue; };
        let entry_type = value.get("type").and_then(Value::as_str).unwrap_or_default();
        let payload = value.get("payload").unwrap_or(&value);
        if entry_type == "session_meta" {
            if let Some(id) = get_string(payload, &["id"]) { session_id = Some(id.to_string()); }
            if let Some(route) = get_string(payload, &["model_provider", "modelProvider"]) {
                provider = Some(route.to_string());
            }
        }

        let mut line_tiers = HashSet::new();
        collect_keyed_strings(payload, &["service_tier", "serviceTier"], &mut line_tiers, 0);
        if !line_tiers.is_empty() {
            let date = get_string(&value, &["timestamp"])
                .and_then(date_from_timestamp)
                .or_else(|| get_string(payload, &["timestamp"]).and_then(date_from_timestamp));
            if let Some(date) = date {
                tiers_by_date.entry(date).or_default().extend(line_tiers);
            } else {
                undated_tiers.extend(line_tiers);
            }
        }
    }

    let id = session_id.or_else(|| path.file_stem().and_then(|v| v.to_str()).map(str::to_string));
    if let Some(id) = id {
        let route_hint = provider.as_ref()
            .and_then(|p| bundle.provider_hints.get(&p.to_ascii_lowercase()))
            .cloned();
        let tiers_by_date = tiers_by_date.into_iter()
            .filter_map(|(date, tiers)| normalize_tiers(tiers).map(|tier| (date, tier)))
            .collect();
        bundle.sessions.insert(
            ("codex".into(), id),
            SessionEvidence {
                explicit_provider: provider,
                route_hint,
                tiers_by_date,
                fallback_tier: normalize_tiers(undated_tiers),
            },
        );
    }
}

fn scan_claude_file(path: &Path, bundle: &mut EvidenceBundle) {
    let Ok(file) = File::open(path) else { return; };
    let fallback_id = path.file_stem().and_then(|v| v.to_str()).unwrap_or_default().to_string();
    let mut ids = HashSet::new();
    if !fallback_id.is_empty() { ids.insert(fallback_id); }
    let mut providers = HashSet::new();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if !line.contains("provider") && !line.contains("sessionId") { continue; }
        let Ok(value) = serde_json::from_str::<Value>(&line) else { continue; };
        if let Some(id) = get_string(&value, &["sessionId", "session_id"]) { ids.insert(id.to_string()); }
        if let Some(route) = get_string(&value, &["providerId", "provider_id", "provider"]) {
            providers.insert(route.to_string());
        }
        if let Some(message) = value.get("message") {
            if let Some(route) = get_string(message, &["providerId", "provider_id", "provider"]) {
                providers.insert(route.to_string());
            }
        }
    }
    let provider = if providers.len() == 1 { providers.into_iter().next() } else { None };
    for id in ids {
        bundle.sessions.insert(
            ("claude".into(), id),
            SessionEvidence {
                explicit_provider: provider.clone(),
                route_hint: None,
                tiers_by_date: HashMap::new(),
                fallback_tier: None,
            },
        );
    }
}

pub fn scan(incremental: bool) -> EvidenceBundle {
    let mut bundle = EvidenceBundle::default();
    let Some(home) = home() else { return bundle; };
    bundle.provider_hints = codex_provider_hints(&home);
    for root in [home.join(".codex/sessions"), home.join(".codex/archived_sessions")] {
        if root.exists() {
            for path in jsonl_files(&root, incremental) { scan_codex_file(&path, &mut bundle); }
        }
    }
    let claude_root = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".claude"))
        .join("projects");
    if claude_root.exists() {
        for path in jsonl_files(&claude_root, incremental) { scan_claude_file(&path, &mut bundle); }
    }
    bundle
}

pub fn for_message<'a>(
    bundle: &'a EvidenceBundle,
    client: &str,
    session_id: &str,
) -> Option<&'a SessionEvidence> {
    bundle.sessions.get(&(client.to_string(), session_id.to_string()))
}

pub fn tier_for_message(
    bundle: &EvidenceBundle,
    client: &str,
    session_id: &str,
    date: &str,
) -> Option<String> {
    let evidence = for_message(bundle, client, session_id)?;
    evidence.tiers_by_date.get(date).cloned().or_else(|| evidence.fallback_tier.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_fast_and_standard_tiers() {
        assert_eq!(normalize_tiers(HashSet::from(["priority".into()])), Some("fast".into()));
        assert_eq!(normalize_tiers(HashSet::from(["default".into()])), Some("standard".into()));
        assert_eq!(normalize_tiers(HashSet::from(["priority".into(), "fast".into()])), Some("fast".into()));
        assert_eq!(normalize_tiers(HashSet::from(["fast".into(), "standard".into()])), Some("mixed".into()));
    }

    #[test]
    fn extracts_only_iso_style_day_prefixes() {
        assert_eq!(date_from_timestamp("2026-08-26T10:20:30Z"), Some("2026-08-26".into()));
        assert_eq!(date_from_timestamp("bad-time"), None);
    }
}
