use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use tokscale_core::pricing::PricingService;
use tokscale_core::{canonical_model_id, parse_local_clients, ClientId, LocalParseOptions, ScannerSettings, TokenBreakdown};

use crate::evidence;
use crate::model::{DeviceInfo, Ledger, Metrics, UsageRow};
use crate::provider;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RowKey {
    date: String,
    client: String,
    provider: String,
    upstream_vendor: String,
    route_provider: String,
    route_type: String,
    model: String,
    tier: Option<String>,
}

#[derive(Default)]
struct RowAccumulator {
    metrics: Metrics,
    sessions: HashSet<String>,
}

fn load_pricing() -> Option<Arc<PricingService>> {
    if let Some(cached) = PricingService::load_cached_any_age() { return Some(Arc::new(cached)); }
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().ok()?;
    runtime.block_on(async { PricingService::get_or_init().await.ok() })
}

fn message_cost(msg: &tokscale_core::ParsedMessage, pricing: Option<&PricingService>) -> f64 {
    let Some(pricing) = pricing else { return 0.0; };
    pricing.calculate_cost_with_provider(
        &msg.model_id,
        Some(&msg.provider_id),
        &TokenBreakdown {
            input: msg.input,
            output: msg.output,
            cache_read: msg.cache_read,
            cache_write: msg.cache_write,
            reasoning: msg.reasoning,
        },
    )
}

fn normalize_text(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() { fallback.to_string() } else { value.to_string() }
}

fn load_scanner_settings() -> ScannerSettings {
    let Some(config_dir) = dirs::config_dir() else { return ScannerSettings::default(); };
    let Ok(data) = std::fs::read(config_dir.join("tokscale/settings.json")) else { return ScannerSettings::default(); };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&data) else { return ScannerSettings::default(); };
    value.get("scanner").cloned().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
}

/// Some Tokscale clients preserve a provider identifier directly from their
/// source record. Only mark the ones whose upstream parser contract proves this.
/// Other clients stay conservative unless our evidence scanner confirms the
/// provider from the raw session/config.
fn parser_provider_is_explicit(client: &str, raw_provider: &str) -> bool {
    !raw_provider.trim().is_empty()
        && raw_provider != "unknown"
        && matches!(client, "opencode" | "micode")
}

pub fn supported_clients() -> Vec<String> {
    let mut clients: Vec<String> = ClientId::iter().map(|id| id.as_str().to_string()).collect();
    clients.sort();
    clients
}

pub fn collect(device: DeviceInfo, since: Option<String>) -> Result<Ledger> {
    let started = Instant::now();
    let incremental = since.is_some();
    let scanner_settings = load_scanner_settings();
    let parsed = parse_local_clients(LocalParseOptions {
        home_dir: None,
        use_env_roots: true,
        clients: None,
        since,
        until: None,
        year: None,
        scanner_settings,
    })
    .map_err(|error| anyhow::anyhow!(error))
    .context("failed to scan local AI coding-tool sessions")?;

    let evidence = evidence::scan(incremental);
    let pricing = load_pricing();
    let mut grouped: BTreeMap<RowKey, RowAccumulator> = BTreeMap::new();

    for msg in parsed.messages {
        let client = normalize_text(&msg.client, "unknown");
        let model = canonical_model_id(&msg.model_id);
        let session_evidence = evidence::for_message(&evidence, &client, &msg.session_id);
        let raw_provider = session_evidence
            .and_then(|e| e.explicit_provider.as_deref())
            .unwrap_or_else(|| msg.provider_id.as_str());
        let explicit_provider = session_evidence.and_then(|e| e.explicit_provider.as_ref()).is_some()
            || parser_provider_is_explicit(&client, raw_provider);
        let identity = provider::classify(
            Some(raw_provider),
            &model,
            explicit_provider,
            session_evidence.and_then(|e| e.route_hint.as_ref()),
        );
        let tier = evidence::tier_for_message(&evidence, &client, &msg.session_id, &msg.date);
        let key = RowKey {
            date: msg.date.clone(),
            client,
            provider: normalize_text(raw_provider, "unknown"),
            upstream_vendor: identity.upstream_vendor,
            route_provider: identity.route_provider,
            route_type: identity.route_type,
            model,
            tier,
        };
        let entry = grouped.entry(key).or_default();
        entry.metrics.input = entry.metrics.input.saturating_add(msg.input.max(0));
        entry.metrics.output = entry.metrics.output.saturating_add(msg.output.max(0));
        entry.metrics.cache_read = entry.metrics.cache_read.saturating_add(msg.cache_read.max(0));
        entry.metrics.cache_write = entry.metrics.cache_write.saturating_add(msg.cache_write.max(0));
        entry.metrics.reasoning = entry.metrics.reasoning.saturating_add(msg.reasoning.max(0));
        entry.metrics.messages = entry.metrics.messages.saturating_add(msg.message_count.max(0));
        entry.metrics.duration_ms = entry.metrics.duration_ms.saturating_add(msg.duration_ms.unwrap_or(0).max(0));
        entry.metrics.cost_usd += message_cost(&msg, pricing.as_deref()).max(0.0);
        if !msg.session_id.is_empty() { entry.sessions.insert(msg.session_id); }
    }

    let mut rows = Vec::with_capacity(grouped.len());
    let mut totals = Metrics::default();
    for (key, mut value) in grouped {
        value.metrics.sessions = value.sessions.len().min(i32::MAX as usize) as i32;
        totals.add(&value.metrics);
        rows.push(UsageRow {
            date: key.date,
            client: key.client,
            provider: key.provider,
            upstream_vendor: key.upstream_vendor,
            route_provider: key.route_provider,
            route_type: key.route_type,
            model: key.model,
            tier: key.tier,
            metrics: value.metrics,
        });
    }

    Ok(Ledger {
        schema_version: 2,
        generated_at: chrono::Utc::now().to_rfc3339(),
        device,
        rows,
        totals,
        scan_ms: started.elapsed().as_millis().min(u64::MAX as u128) as u64,
    })
}

pub fn merge_incremental(mut previous: Ledger, partial: Ledger, since: &str) -> Ledger {
    previous.rows.retain(|row| row.date.as_str() < since);
    previous.rows.extend(partial.rows);
    previous.rows.sort_by(|a, b| {
        a.date.cmp(&b.date)
            .then_with(|| a.client.cmp(&b.client))
            .then_with(|| a.route_provider.cmp(&b.route_provider))
            .then_with(|| a.model.cmp(&b.model))
            .then_with(|| a.tier.cmp(&b.tier))
    });
    previous.generated_at = partial.generated_at;
    previous.device = partial.device;
    previous.scan_ms = partial.scan_ms;
    previous.totals = Metrics::default();
    for row in &previous.rows { previous.totals.add(&row.metrics); }
    previous
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tokscale_client_is_exposed() {
        let clients = supported_clients();
        assert!(clients.contains(&"codex".to_string()));
        assert!(clients.contains(&"claude".to_string()));
        assert!(clients.contains(&"opencode".to_string()));
        assert!(clients.contains(&"gemini".to_string()));
        assert!(clients.len() >= 20, "expected broad Tokscale client coverage, got {}", clients.len());
    }

    #[test]
    fn only_proven_schema_providers_are_marked_explicit() {
        assert!(parser_provider_is_explicit("opencode", "openrouter"));
        assert!(parser_provider_is_explicit("micode", "openai"));
        assert!(!parser_provider_is_explicit("kimi", "moonshot"));
        assert!(!parser_provider_is_explicit("gemini", "google"));
        assert!(!parser_provider_is_explicit("opencode", "unknown"));
    }
}
