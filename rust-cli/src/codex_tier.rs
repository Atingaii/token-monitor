//! Codex service-tier enhancement.
//!
//! The canonical Codex total remains Tokscale v4.14.0. This module is a
//! deliberately narrow adapter derived from the MIT-licensed request parser and
//! `estimateCost` logic in `falyx6851-byte/codex-monitor` (2026), translated to
//! Rust and stripped of its HTTP/SQLite/UI concerns. Its output is accepted by
//! `collector.rs` only after exact per-day token reconciliation with Tokscale.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokscale_core::{BucketTimezone, ScannerSettings};
use walkdir::WalkDir;

use crate::model::Metrics;

const PRICING_JSON: &str = include_str!("../pricing/codex_tiers.json");

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct RawUsage {
    input: i64,
    cached: i64,
    cache_write: Option<i64>,
    output: i64,
    reasoning: i64,
    total: i64,
}

impl RawUsage {
    fn normalized_metrics(&self) -> Metrics {
        // Match Tokscale's Codex TokenBreakdown semantics: Codex input includes
        // cached input and output includes reasoning, while the normalized
        // buckets persisted by Token Monitor must be additive.
        let cached = self.cached.max(0).min(self.input.max(0));
        let uncached = self.input.max(0).saturating_sub(cached);
        let cache_write = self.cache_write.unwrap_or(0).max(0).min(uncached);
        let reasoning = self.reasoning.max(0).min(self.output.max(0));
        Metrics {
            input: uncached.saturating_sub(cache_write),
            cache_read: cached,
            cache_write,
            output: self.output.max(0).saturating_sub(reasoning),
            reasoning,
            messages: 1,
            duration_ms: 0,
            cost_usd: 0.0,
        }
    }

    fn tuple(&self) -> String {
        format!(
            "{}/{}/{}/{}/{}/{}",
            self.input,
            self.cached,
            self.cache_write.map(|v| v.to_string()).unwrap_or_else(|| "?".into()),
            self.output,
            self.reasoning,
            self.total
        )
    }
}

#[derive(Debug, Clone)]
pub struct EnhancedCodexRow {
    pub date: String,
    pub model: String,
    pub provider: String,
    pub tier: String,
    pub metrics: Metrics,
    pub cost_lower_bound: bool,
}

#[derive(Debug, Clone, Default)]
pub struct EnhancementResult {
    pub rows: Vec<EnhancedCodexRow>,
    /// A day is eligible only when every token-bearing request on that day had
    /// a supported model/tier price and therefore can replace Tokscale without
    /// silently dropping cost coverage.
    pub eligible_days: HashSet<String>,
    pub totals_by_date: HashMap<String, i64>,
}

#[derive(Debug, Deserialize)]
struct PricingConfig {
    long_context_threshold_tokens: i64,
    #[serde(default)]
    aliases: HashMap<String, String>,
    models: HashMap<String, ModelPricing>,
}

#[derive(Debug, Deserialize)]
struct ModelPricing {
    standard: ContextPricing,
    fast: ContextPricing,
}

#[derive(Debug, Deserialize)]
struct ContextPricing {
    short: Rates,
    long: Rates,
}

#[derive(Debug, Clone, Deserialize)]
struct Rates {
    input: f64,
    cached_input: f64,
    cache_write: f64,
    output: f64,
}

#[derive(Debug)]
struct CostEstimate {
    usd: f64,
    lower_bound: bool,
}

fn pricing() -> &'static PricingConfig {
    static PRICING: OnceLock<PricingConfig> = OnceLock::new();
    PRICING.get_or_init(|| {
        serde_json::from_str(PRICING_JSON).expect("embedded Codex tier pricing JSON must be valid")
    })
}

fn number(value: Option<&Value>) -> i64 {
    value
        .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|n| n.min(i64::MAX as u64) as i64)))
        .unwrap_or(0)
        .max(0)
}

fn optional_number(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    value
        .as_i64()
        .or_else(|| value.as_u64().map(|n| n.min(i64::MAX as u64) as i64))
        .map(|n| n.max(0))
}

/// Port of codex-monitor's `normalizeUsage`, preserving the distinction between
/// an absent cache-write field and an explicit zero.
fn normalize_usage(value: Option<&Value>) -> RawUsage {
    let Some(usage) = value.and_then(Value::as_object) else { return RawUsage::default(); };
    let details = usage.get("input_tokens_details").and_then(Value::as_object);
    let output_details = usage.get("output_tokens_details").and_then(Value::as_object);
    let input = number(usage.get("input_tokens"));
    let cached = number(
        usage.get("cached_input_tokens")
            .or_else(|| details.and_then(|d| d.get("cached_tokens")))
            .or_else(|| usage.get("cache_read_input_tokens")),
    );
    let cache_write = optional_number(
        usage.get("cache_write_input_tokens")
            .or_else(|| usage.get("cache_write_tokens"))
            .or_else(|| details.and_then(|d| d.get("cache_write_tokens"))),
    );
    let output = number(usage.get("output_tokens"));
    let reasoning = number(
        usage.get("reasoning_output_tokens")
            .or_else(|| output_details.and_then(|d| d.get("reasoning_tokens"))),
    );
    let total = optional_number(usage.get("total_tokens")).unwrap_or_else(|| input.saturating_add(output));
    RawUsage { input, cached, cache_write, output, reasoning, total }
}

fn normalize_tier(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "priority" | "fast" => "fast".into(),
        "default" | "standard" | "" => "standard".into(),
        other => other.to_string(),
    }
}

/// Port of codex-monitor's explicit candidate list. Do not recursively search
/// arbitrary JSON: a field named `service_tier` in unrelated tool output must
/// never affect billing state.
fn extract_service_tier(payload: &Value) -> Option<String> {
    let candidates = [
        payload.pointer("/thread_settings/service_tier"),
        payload.pointer("/thread_settings/serviceTier"),
        payload.get("service_tier"),
        payload.get("serviceTier"),
        payload.pointer("/info/service_tier"),
        payload.pointer("/info/serviceTier"),
        payload.pointer("/request/service_tier"),
        payload.pointer("/request/serviceTier"),
        payload.pointer("/response/service_tier"),
        payload.pointer("/response/serviceTier"),
    ];
    candidates.into_iter().flatten().find_map(|value| {
        value.as_str().map(normalize_tier).filter(|tier| !tier.is_empty())
    })
}

fn resolve_model<'a>(model: &'a str, config: &'a PricingConfig) -> Option<(&'a str, &'a ModelPricing)> {
    let raw = model.trim().to_ascii_lowercase();
    if let Some((key, pricing)) = config.models.get_key_value(&raw) {
        return Some((key.as_str(), pricing));
    }
    let alias = config.aliases.get(&raw)?;
    let (key, pricing) = config.models.get_key_value(alias)?;
    Some((key.as_str(), pricing))
}

/// Port of codex-monitor's tier-aware cost formula, with the rate table refreshed
/// from current official OpenAI pricing. `input` is the raw Codex input count,
/// which includes cached tokens; long-context selection is request-level.
fn estimate_cost(usage: &RawUsage, model: &str, tier: &str) -> Option<CostEstimate> {
    let config = pricing();
    let (_, model_pricing) = resolve_model(model, config)?;
    let tier = normalize_tier(tier);
    let context = if usage.input > config.long_context_threshold_tokens { "long" } else { "short" };
    let rates = match tier.as_str() {
        "standard" => if context == "long" { &model_pricing.standard.long } else { &model_pricing.standard.short },
        "fast" => if context == "long" { &model_pricing.fast.long } else { &model_pricing.fast.short },
        _ => return None,
    };

    let cached = usage.cached.max(0).min(usage.input.max(0));
    let uncached = usage.input.max(0).saturating_sub(cached);
    let reported_cache_write = usage.cache_write.map(|v| v.max(0).min(uncached));
    let cache_write = reported_cache_write.unwrap_or(0);
    let regular_input = uncached.saturating_sub(cache_write);
    let lower_bound = usage.cache_write.is_none() && uncached > 0 && rates.cache_write > 0.0;

    let usd = (regular_input as f64 / 1_000_000.0) * rates.input
        + (cached as f64 / 1_000_000.0) * rates.cached_input
        + (cache_write as f64 / 1_000_000.0) * rates.cache_write
        + (usage.output.max(0) as f64 / 1_000_000.0) * rates.output;
    Some(CostEstimate { usd, lower_bound })
}

fn timestamp_ms(value: Option<&Value>) -> Option<i64> {
    let raw = value?.as_str()?;
    chrono::DateTime::parse_from_rfc3339(raw).ok().map(|dt| dt.timestamp_millis())
}

fn string_at<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str).filter(|s| !s.trim().is_empty())
}

fn usage_dedupe_key(session_id: &str, usage: &RawUsage, total: &RawUsage) -> String {
    format!("{session_id}|last={}|total={}", usage.tuple(), total.tuple())
}

fn short_file_id(path: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    hex::encode(hasher.finalize())[..12].to_string()
}

#[derive(Debug)]
struct RequestRecord {
    dedupe: String,
    date: String,
    model: String,
    provider: String,
    tier: String,
    usage: RawUsage,
    cost: Option<CostEstimate>,
}

fn parse_session_file(path: &Path, bucket_timezone: &BucketTimezone) -> Vec<RequestRecord> {
    let Ok(file) = File::open(path) else { return Vec::new(); };
    let mut session_id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();
    let mut provider = String::new();
    let mut current_model = String::new();
    let mut current_tier = "standard".to_string();
    let mut records = Vec::new();

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if line.trim().is_empty() { continue; }
        let Ok(row) = serde_json::from_str::<Value>(&line) else { continue; };
        let payload = row.get("payload").unwrap_or(&Value::Null);
        if let Some(tier) = extract_service_tier(payload) { current_tier = tier; }

        match string_at(&row, "type").unwrap_or_default() {
            "session_meta" => {
                if let Some(id) = string_at(payload, "id") { session_id = id.to_string(); }
                if let Some(model_provider) = string_at(payload, "model_provider") {
                    provider = model_provider.to_string();
                }
            }
            "turn_context" => {
                if let Some(model) = string_at(payload, "model") { current_model = model.to_string(); }
            }
            "event_msg" if string_at(payload, "type") == Some("token_count") => {
                let Some(ms) = timestamp_ms(row.get("timestamp")) else { continue; };
                let info = payload.get("info").unwrap_or(&Value::Null);
                let usage = normalize_usage(info.get("last_token_usage"));
                if usage.total <= 0 { continue; }
                // Same exclusion as codex-monitor: a total-only record has no
                // billable/request breakdown and is commonly compaction/system bookkeeping.
                if usage.total > 0 && usage.input == 0 && usage.cached == 0 && usage.output == 0 {
                    continue;
                }
                let total = normalize_usage(info.get("total_token_usage"));
                let model = current_model.clone();
                let tier = current_tier.clone();
                let cost = estimate_cost(&usage, &model, &tier);
                records.push(RequestRecord {
                    dedupe: usage_dedupe_key(&session_id, &usage, &total),
                    date: bucket_timezone.day_key(ms),
                    model,
                    provider: provider.clone(),
                    tier,
                    usage,
                    cost,
                });
            }
            _ => {}
        }
    }

    // If session_meta was absent, make fallback dedupe keys path-specific rather
    // than allowing two unrelated malformed files to collapse into one.
    if session_id == "unknown" {
        let suffix = short_file_id(path);
        for record in &mut records { record.dedupe.push_str(&format!("|file={suffix}")); }
    }
    records
}

fn codex_roots() -> Vec<PathBuf> {
    let base = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".codex")));
    base.map(|root| vec![root.join("sessions"), root.join("archived_sessions")])
        .unwrap_or_default()
}

fn recent_enough(path: &Path, incremental: bool) -> bool {
    if !incremental { return true; }
    let Ok(modified) = path.metadata().and_then(|m| m.modified()) else { return true; };
    modified >= SystemTime::now()
        .checked_sub(Duration::from_secs(5 * 24 * 3600))
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

pub fn collect(since: Option<&str>, scanner_settings: &ScannerSettings) -> Result<EnhancementResult> {
    let incremental = since.is_some();
    let bucket_timezone = BucketTimezone::from_scanner_settings(scanner_settings);
    let mut seen = HashSet::new();
    let mut requests = Vec::new();

    for root in codex_roots() {
        if !root.exists() { continue; }
        for entry in WalkDir::new(&root).follow_links(false).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() { continue; }
            let path = entry.path();
            if path.extension().and_then(|v| v.to_str()) != Some("jsonl") || !recent_enough(path, incremental) {
                continue;
            }
            for record in parse_session_file(path, &bucket_timezone) {
                if since.is_some_and(|start| record.date.as_str() < start) { continue; }
                if seen.insert(record.dedupe.clone()) { requests.push(record); }
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    struct Key(String, String, String, String); // date, model, provider, tier
    #[derive(Default)]
    struct Acc { metrics: Metrics, lower_bound: bool, all_priced: bool }

    let mut grouped: BTreeMap<Key, Acc> = BTreeMap::new();
    let mut totals_by_date: HashMap<String, i64> = HashMap::new();
    let mut all_priced_by_date: HashMap<String, bool> = HashMap::new();

    for request in requests {
        let mut metrics = request.usage.normalized_metrics();
        let priced = request.cost.is_some();
        let lower_bound = request.cost.as_ref().is_some_and(|cost| cost.lower_bound);
        metrics.cost_usd = request.cost.as_ref().map(|cost| cost.usd).unwrap_or(0.0);
        let token_total = metrics.total_tokens();
        *totals_by_date.entry(request.date.clone()).or_default() = totals_by_date
            .get(&request.date)
            .copied()
            .unwrap_or(0)
            .saturating_add(token_total);
        all_priced_by_date
            .entry(request.date.clone())
            .and_modify(|value| *value &= priced)
            .or_insert(priced);

        let entry = grouped.entry(Key(request.date, request.model, request.provider, request.tier)).or_insert_with(|| Acc {
            metrics: Metrics::default(),
            lower_bound: false,
            all_priced: true,
        });
        entry.metrics.add(&metrics);
        entry.lower_bound |= lower_bound;
        entry.all_priced &= priced;
    }

    let rows = grouped.into_iter().map(|(Key(date, model, provider, tier), acc)| EnhancedCodexRow {
        date,
        model,
        provider,
        tier,
        metrics: acc.metrics,
        cost_lower_bound: acc.lower_bound || !acc.all_priced,
    }).collect();
    let eligible_days = all_priced_by_date.into_iter()
        .filter_map(|(date, all_priced)| all_priced.then_some(date))
        .collect();

    Ok(EnhancementResult { rows, eligible_days, totals_by_date })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_thread_settings_tier_like_upstream() {
        let payload = serde_json::json!({"thread_settings":{"service_tier":"priority"}});
        assert_eq!(extract_service_tier(&payload).as_deref(), Some("fast"));
    }

    #[test]
    fn current_sol_fast_short_context_example() {
        let usage = RawUsage {
            input: 100_000,
            cached: 50_000,
            cache_write: Some(10_000),
            output: 10_000,
            reasoning: 0,
            total: 110_000,
        };
        let cost = estimate_cost(&usage, "gpt-5.6-sol", "priority").unwrap();
        // 40K regular input * $8/M + 50K cached * $0.8/M +
        // 10K cache write * $10/M + 10K output * $40/M = $0.86.
        assert!((cost.usd - 0.86).abs() < 1e-12);
        assert!(!cost.lower_bound);
    }

    #[test]
    fn missing_cache_write_is_marked_lower_bound() {
        let usage = RawUsage { input: 1000, cached: 500, cache_write: None, output: 100, reasoning: 20, total: 1100 };
        let cost = estimate_cost(&usage, "gpt-5.6-sol", "fast").unwrap();
        assert!(cost.lower_bound);
        assert_eq!(usage.normalized_metrics().total_tokens(), 1100);
    }

    #[test]
    fn unsupported_tier_is_not_guessed() {
        let usage = RawUsage { input: 1000, cached: 0, cache_write: Some(0), output: 100, reasoning: 0, total: 1100 };
        assert!(estimate_cost(&usage, "gpt-5.6-sol", "mystery").is_none());
    }
}
