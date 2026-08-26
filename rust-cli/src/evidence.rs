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
    pub tier: Option<String>,
    pub route_hint: Option<RouteHint>,
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
    modified >= SystemTime::now().checked_sub(Duration::from_secs(5 * 24 * 3600)).unwrap_or(SystemTime::UNIX_EPOCH)
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
    keys.iter().find_map(|key| value.get(*key).and_then(Value::as_str)).filter(|v| !v.trim().is_empty())
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
        Value::Array(items) => for child in items { collect_keyed_strings(child, keys, out, depth + 1); },
        _ => {}
    }
}

fn normalize_tiers(tiers: HashSet<String>) -> Option<String> {
    if tiers.is_empty() { return None; }
    if tiers.len() > 1 { return Some("mixed".to_string()); }
    let value = tiers.into_iter().next().unwrap();
    let canonical = match value.as_str() {
        "priority" | "fast" => "fast",
        "default" | "standard" => "standard",
        "flex" => "flex",
        other => other,
    };
    Some(canonical.to_string())
}

fn codex_provider_hints(home: &Path) -> HashMap<String, RouteHint> {
    let codex_home = std::env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".codex"));
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
    let mut tiers = HashSet::new();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if !line.contains("session_meta") && !line.contains("service_tier") && !line.contains("serviceTier") { continue; }
        let Ok(value) = serde_json::from_str::<Value>(&line) else { continue; };
        let entry_type = value.get("type").and_then(Value::as_str).unwrap_or_default();
        let payload = value.get("payload").unwrap_or(&value);
        if entry_type == "session_meta" {
            if let Some(id) = get_string(payload, &["id"]) { session_id = Some(id.to_string()); }
            if let Some(route) = get_string(payload, &["model_provider", "modelProvider"]) { provider = Some(route.to_string()); }
        }
        collect_keyed_strings(payload, &["service_tier", "serviceTier"], &mut tiers, 0);
    }
    let id = session_id.or_else(|| path.file_stem().and_then(|v| v.to_str()).map(str::to_string));
    if let Some(id) = id {
        let route_hint = provider.as_ref().and_then(|p| bundle.provider_hints.get(&p.to_ascii_lowercase())).cloned();
        bundle.sessions.insert(("codex".into(), id), SessionEvidence { explicit_provider: provider, tier: normalize_tiers(tiers), route_hint });
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
        if let Some(route) = get_string(&value, &["providerId", "provider_id", "provider"]) { providers.insert(route.to_string()); }
        if let Some(message) = value.get("message") {
            if let Some(route) = get_string(message, &["providerId", "provider_id", "provider"]) { providers.insert(route.to_string()); }
        }
    }
    let provider = if providers.len() == 1 { providers.into_iter().next() } else { None };
    for id in ids {
        bundle.sessions.insert(("claude".into(), id), SessionEvidence { explicit_provider: provider.clone(), tier: None, route_hint: None });
    }
}

pub fn scan(incremental: bool) -> EvidenceBundle {
    let mut bundle = EvidenceBundle::default();
    let Some(home) = home() else { return bundle; };
    bundle.provider_hints = codex_provider_hints(&home);
    for root in [home.join(".codex/sessions"), home.join(".codex/archived_sessions")] {
        if root.exists() { for path in jsonl_files(&root, incremental) { scan_codex_file(&path, &mut bundle); } }
    }
    let claude_root = std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from).unwrap_or_else(|| home.join(".claude")).join("projects");
    if claude_root.exists() { for path in jsonl_files(&claude_root, incremental) { scan_claude_file(&path, &mut bundle); } }
    bundle
}

pub fn for_message<'a>(bundle: &'a EvidenceBundle, client: &str, session_id: &str) -> Option<&'a SessionEvidence> {
    bundle.sessions.get(&(client.to_string(), session_id.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_fast_and_standard_tiers() {
        assert_eq!(normalize_tiers(HashSet::from(["priority".into()])), Some("fast".into()));
        assert_eq!(normalize_tiers(HashSet::from(["default".into()])), Some("standard".into()));
        assert_eq!(normalize_tiers(HashSet::from(["fast".into(), "standard".into()])), Some("mixed".into()));
    }
}
