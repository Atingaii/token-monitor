//! Codex service-tier enhancement.
//!
//! Canonical Codex token accounting remains Tokscale v4.14.0. This module is a
//! narrow adapter derived from the MIT-licensed request/tier parsing approach in
//! `falyx6851-byte/codex-monitor`. Its rows may replace canonical Codex rows only
//! after exact day-level token + message reconciliation in `collector.rs`.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use anyhow::Result;
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
            cost_usd: 0.0,
            plan_cost_usd: 0.0,
        }
    }

    fn tuple(&self) -> String {
        format!(
            "{}/{}/{}/{}/{}/{}",
            self.input,
            self.cached,
            self.cache_write
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".into()),
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
    /// At least one request in this grouped row matched the explicit Codex plan
    /// rate table. If false, the dashboard must show plan cost as unavailable,
    /// not as an exact $0.
    pub plan_cost_available: bool,
}

#[derive(Debug, Clone, Default)]
pub struct DayReconciliation {
    pub tokens: i64,
    pub messages: i32,
    /// Diagnostic only. Price coverage is not a condition for accepting tier
    /// evidence; unknown prices are surfaced as a lower bound.
    pub all_priced: bool,
}

#[derive(Debug, Clone, Default)]
pub struct EnhancementResult {
    pub rows: Vec<EnhancedCodexRow>,
    pub by_date: HashMap<String, DayReconciliation>,
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
    api: BillingPricing,
    plan: BillingPricing,
}

#[derive(Debug, Deserialize)]
struct BillingPricing {
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
    api_usd: f64,
    plan_usd: f64,
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
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_u64().map(|n| n.min(i64::MAX as u64) as i64))
        })
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

fn normalize_usage(value: Option<&Value>) -> RawUsage {
    let Some(usage) = value.and_then(Value::as_object) else {
        return RawUsage::default();
    };
    let details = usage.get("input_tokens_details").and_then(Value::as_object);
    let output_details = usage.get("output_tokens_details").and_then(Value::as_object);
    let input = number(usage.get("input_tokens"));
    let cached = number(
        usage
            .get("cached_input_tokens")
            .or_else(|| details.and_then(|details| details.get("cached_tokens")))
            .or_else(|| usage.get("cache_read_input_tokens")),
    );
    let cache_write = optional_number(
        usage
            .get("cache_write_input_tokens")
            .or_else(|| usage.get("cache_write_tokens"))
            .or_else(|| details.and_then(|details| details.get("cache_write_tokens"))),
    );
    let output = number(usage.get("output_tokens"));
    let reasoning = number(
        usage
            .get("reasoning_output_tokens")
            .or_else(|| output_details.and_then(|details| details.get("reasoning_tokens"))),
    );
    let total = optional_number(usage.get("total_tokens"))
        .unwrap_or_else(|| input.saturating_add(output));
    RawUsage {
        input,
        cached,
        cache_write,
        output,
        reasoning,
        total,
    }
}

fn normalize_tier(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "priority" | "fast" => "fast".into(),
        "default" | "standard" | "" => "standard".into(),
        other => other.to_string(),
    }
}

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
        value
            .as_str()
            .map(normalize_tier)
            .filter(|tier| !tier.is_empty())
    })
}

fn resolve_model<'a>(
    model: &'a str,
    config: &'a PricingConfig,
) -> Option<(&'a str, &'a ModelPricing)> {
    let raw = model.trim().to_ascii_lowercase();
    if let Some((key, rates)) = config.models.get_key_value(&raw) {
        return Some((key.as_str(), rates));
    }
    let alias = config.aliases.get(&raw)?;
    let (key, rates) = config.models.get_key_value(alias)?;
    Some((key.as_str(), rates))
}

fn rates_for<'a>(pricing: &'a BillingPricing, tier: &str, long: bool) -> Option<&'a Rates> {
    match (normalize_tier(tier).as_str(), long) {
        ("standard", false) => Some(&pricing.standard.short),
        ("standard", true) => Some(&pricing.standard.long),
        ("fast", false) => Some(&pricing.fast.short),
        ("fast", true) => Some(&pricing.fast.long),
        _ => None,
    }
}

fn estimate_with_rates(usage: &RawUsage, rates: &Rates) -> f64 {
    let cached = usage.cached.max(0).min(usage.input.max(0));
    let uncached = usage.input.max(0).saturating_sub(cached);
    let cache_write = usage
        .cache_write
        .map(|value| value.max(0).min(uncached))
        .unwrap_or(0);
    let regular_input = uncached.saturating_sub(cache_write);
    (regular_input as f64 / 1_000_000.0) * rates.input
        + (cached as f64 / 1_000_000.0) * rates.cached_input
        + (cache_write as f64 / 1_000_000.0) * rates.cache_write
        + (usage.output.max(0) as f64 / 1_000_000.0) * rates.output
}

fn estimate_cost(usage: &RawUsage, model: &str, tier: &str) -> Option<CostEstimate> {
    let config = pricing();
    let (_, model_pricing) = resolve_model(model, config)?;
    let long = usage.input > config.long_context_threshold_tokens;
    let api_rates = rates_for(&model_pricing.api, tier, long)?;
    let plan_rates = rates_for(&model_pricing.plan, tier, long)?;
    let cached = usage.cached.max(0).min(usage.input.max(0));
    let uncached = usage.input.max(0).saturating_sub(cached);
    let lower_bound = usage.cache_write.is_none()
        && uncached > 0
        && (api_rates.cache_write > 0.0 || plan_rates.cache_write > 0.0);
    Some(CostEstimate {
        api_usd: estimate_with_rates(usage, api_rates),
        plan_usd: estimate_with_rates(usage, plan_rates),
        lower_bound,
    })
}

fn timestamp_ms(value: Option<&Value>) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value?.as_str()?)
        .ok()
        .map(|date| date.timestamp_millis())
}

fn string_at<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
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
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    let mut session_id = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown")
        .to_string();
    let mut provider = String::new();
    let mut current_model = String::new();
    let mut current_tier = "standard".to_string();
    let mut records = Vec::new();

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(row) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let payload = row.get("payload").unwrap_or(&Value::Null);
        if let Some(tier) = extract_service_tier(payload) {
            current_tier = tier;
        }

        match string_at(&row, "type").unwrap_or_default() {
            "session_meta" => {
                if let Some(id) = string_at(payload, "id") {
                    session_id = id.to_string();
                }
                if let Some(model_provider) = string_at(payload, "model_provider") {
                    provider = model_provider.to_string();
                }
            }
            "turn_context" => {
                if let Some(model) = string_at(payload, "model") {
                    current_model = model.to_string();
                }
            }
            "event_msg" if string_at(payload, "type") == Some("token_count") => {
                let Some(ms) = timestamp_ms(row.get("timestamp")) else {
                    continue;
                };
                let info = payload.get("info").unwrap_or(&Value::Null);
                let usage = normalize_usage(info.get("last_token_usage"));
                if usage.total <= 0
                    || (usage.input == 0 && usage.cached == 0 && usage.output == 0)
                {
                    continue;
                }
                let total = normalize_usage(info.get("total_token_usage"));
                let tier = current_tier.clone();
                records.push(RequestRecord {
                    dedupe: usage_dedupe_key(&session_id, &usage, &total),
                    date: bucket_timezone.day_key(ms),
                    model: current_model.clone(),
                    provider: provider.clone(),
                    tier: tier.clone(),
                    cost: estimate_cost(&usage, &current_model, &tier),
                    usage,
                });
            }
            _ => {}
        }
    }

    if session_id == "unknown" {
        let suffix = short_file_id(path);
        for record in &mut records {
            record.dedupe.push_str(&format!("|file={suffix}"));
        }
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
    if !incremental {
        return true;
    }
    let Ok(modified) = path.metadata().and_then(|metadata| metadata.modified()) else {
        return true;
    };
    modified
        >= SystemTime::now()
            .checked_sub(Duration::from_secs(5 * 24 * 3600))
            .unwrap_or(SystemTime::UNIX_EPOCH)
}

pub fn collect(since: Option<&str>, scanner_settings: &ScannerSettings) -> Result<EnhancementResult> {
    let incremental = since.is_some();
    let bucket_timezone = BucketTimezone::from_scanner_settings(scanner_settings);
    let mut seen = HashSet::new();
    let mut requests = Vec::new();

    for root in codex_roots() {
        if !root.exists() {
            continue;
        }
        for entry in WalkDir::new(&root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("jsonl")
                || !recent_enough(path, incremental)
            {
                continue;
            }
            for record in parse_session_file(path, &bucket_timezone) {
                if since.is_some_and(|start| record.date.as_str() < start) {
                    continue;
                }
                if seen.insert(record.dedupe.clone()) {
                    requests.push(record);
                }
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    struct Key(String, String, String, String); // date, model, provider, tier

    #[derive(Default)]
    struct Acc {
        metrics: Metrics,
        lower_bound: bool,
        priced_any: bool,
    }

    let mut grouped: BTreeMap<Key, Acc> = BTreeMap::new();
    let mut by_date: HashMap<String, DayReconciliation> = HashMap::new();

    for request in requests {
        let mut metrics = request.usage.normalized_metrics();
        let priced = request.cost.is_some();
        let lower_bound = request
            .cost
            .as_ref()
            .map(|cost| cost.lower_bound)
            .unwrap_or(true);
        metrics.cost_usd = request.cost.as_ref().map(|cost| cost.api_usd).unwrap_or(0.0);
        metrics.plan_cost_usd = request
            .cost
            .as_ref()
            .map(|cost| cost.plan_usd)
            .unwrap_or(0.0);

        let day = by_date
            .entry(request.date.clone())
            .or_insert_with(|| DayReconciliation {
                tokens: 0,
                messages: 0,
                all_priced: true,
            });
        day.tokens = day.tokens.saturating_add(metrics.total_tokens());
        day.messages = day.messages.saturating_add(metrics.messages);
        day.all_priced &= priced;

        let entry = grouped
            .entry(Key(
                request.date,
                request.model,
                request.provider,
                request.tier,
            ))
            .or_default();
        entry.metrics.add(&metrics);
        entry.lower_bound |= lower_bound;
        entry.priced_any |= priced;
    }

    let rows = grouped
        .into_iter()
        .map(|(Key(date, model, provider, tier), acc)| EnhancedCodexRow {
            date,
            model,
            provider,
            tier,
            metrics: acc.metrics,
            cost_lower_bound: acc.lower_bound,
            plan_cost_available: acc.priced_any,
        })
        .collect();

    Ok(EnhancementResult { rows, by_date })
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
    fn sol_fast_has_separate_api_and_plan_equivalents() {
        let usage = RawUsage {
            input: 100_000,
            cached: 50_000,
            cache_write: Some(10_000),
            output: 10_000,
            reasoning: 0,
            total: 110_000,
        };
        let cost = estimate_cost(&usage, "gpt-5.6-sol", "priority").unwrap();
        // API Fast: 40K*$8 + 50K*$0.8 + 10K*$10 + 10K*$40 = $0.86.
        assert!((cost.api_usd - 0.86).abs() < 1e-12);
        // Plan/legacy basis: 2.5x of launch-plan inputs/output/cache buckets.
        assert!((cost.plan_usd - 1.46875).abs() < 1e-12);
        assert!(!cost.lower_bound);
    }

    #[test]
    fn sol_standard_plan_preserves_launch_basis() {
        let usage = RawUsage {
            input: 100_000,
            cached: 50_000,
            cache_write: Some(0),
            output: 10_000,
            reasoning: 0,
            total: 110_000,
        };
        let cost = estimate_cost(&usage, "gpt-5.6-sol", "standard").unwrap();
        // Current API: 50K*$4 + 50K*$0.4 + 10K*$20 = $0.42.
        assert!((cost.api_usd - 0.42).abs() < 1e-12);
        // Plan basis: 50K*$5 + 50K*$0.5 + 10K*$30 = $0.575.
        assert!((cost.plan_usd - 0.575).abs() < 1e-12);
    }

    #[test]
    fn missing_cache_write_is_marked_lower_bound() {
        let usage = RawUsage {
            input: 1000,
            cached: 500,
            cache_write: None,
            output: 100,
            reasoning: 20,
            total: 1100,
        };
        let cost = estimate_cost(&usage, "gpt-5.6-sol", "fast").unwrap();
        assert!(cost.lower_bound);
        assert_eq!(usage.normalized_metrics().total_tokens(), 1100);
    }

    #[test]
    fn unsupported_tier_is_not_guessed() {
        let usage = RawUsage {
            input: 1000,
            cached: 0,
            cache_write: Some(0),
            output: 100,
            reasoning: 0,
            total: 1100,
        };
        assert!(estimate_cost(&usage, "gpt-5.6-sol", "mystery").is_none());
    }
}
