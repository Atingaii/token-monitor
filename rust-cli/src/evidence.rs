use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::Value;
use walkdir::WalkDir;

#[derive(Debug, Clone, Default)]
pub struct SessionEvidence {
    pub explicit_provider: Option<String>,
    pub tier: Option<String>,
}

pub type EvidenceMap = HashMap<(String, String), SessionEvidence>;

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

fn scan_codex_file(path: &Path, map: &mut EvidenceMap) {
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
    let fallback_id = path.file_stem().and_then(|v| v.to_str()).map(str::to_string);
    let id = session_id.or(fallback_id);
    if let Some(id) = id {
        map.insert(("codex".into(), id), SessionEvidence { explicit_provider: provider, tier: normalize_tiers(tiers) });
    }
}

fn scan_claude_file(path: &Path, map: &mut EvidenceMap) {
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
        map.insert(("claude".into(), id), SessionEvidence { explicit_provider: provider.clone(), tier: None });
    }
}

pub fn scan(incremental: bool) -> EvidenceMap {
    let mut map = EvidenceMap::new();
    let Some(home) = home() else { return map; };

    for root in [home.join(".codex/sessions"), home.join(".codex/archived_sessions")] {
        if root.exists() { for path in jsonl_files(&root, incremental) { scan_codex_file(&path, &mut map); } }
    }
    let claude_root = std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from).unwrap_or_else(|| home.join(".claude")).join("projects");
    if claude_root.exists() { for path in jsonl_files(&claude_root, incremental) { scan_claude_file(&path, &mut map); } }
    map
}

pub fn for_message<'a>(map: &'a EvidenceMap, client: &str, session_id: &str) -> Option<&'a SessionEvidence> {
    map.get(&(client.to_string(), session_id.to_string()))
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
