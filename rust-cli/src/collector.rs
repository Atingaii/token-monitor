use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use tokscale_core::pricing::PricingService;
use tokscale_core::{
    canonical_model_id, parse_local_clients, LocalParseOptions, ScannerSettings, TokenBreakdown,
};

use crate::model::{DeviceInfo, Ledger, Metrics, UsageRow};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RowKey {
    date: String,
    client: String,
    provider: String,
    model: String,
    tier: Option<String>,
}

#[derive(Default)]
struct RowAccumulator {
    metrics: Metrics,
    sessions: HashSet<String>,
}

fn load_pricing() -> Option<Arc<PricingService>> {
    if let Some(cached) = PricingService::load_cached_any_age() {
        return Some(Arc::new(cached));
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
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

/// Tokscale currently exposes a provider dimension but not a universal service-tier
/// dimension. Keep the schema ready for fast/standard/priority whenever a source
/// exposes it without guessing from model names.
fn detect_service_tier(_client: &str, _provider: &str, _model: &str) -> Option<String> {
    None
}

pub fn collect(device: DeviceInfo, since: Option<String>) -> Result<Ledger> {
    let started = Instant::now();
    let parsed = parse_local_clients(LocalParseOptions {
        home_dir: None,
        use_env_roots: true,
        clients: None,
        since,
        until: None,
        year: None,
        scanner_settings: ScannerSettings::default(),
    })
    .map_err(|error| anyhow::anyhow!(error))
    .context("failed to scan local AI coding-tool sessions")?;

    let pricing = load_pricing();
    let mut grouped: BTreeMap<RowKey, RowAccumulator> = BTreeMap::new();

    for msg in parsed.messages {
        let client = normalize_text(&msg.client, "unknown");
        let provider = normalize_text(&msg.provider_id, "unknown");
        let model = canonical_model_id(&msg.model_id);
        let tier = detect_service_tier(&client, &provider, &model);
        let key = RowKey {
            date: msg.date.clone(),
            client,
            provider,
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
        if !msg.session_id.is_empty() {
            entry.sessions.insert(msg.session_id);
        }
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
            .then_with(|| a.provider.cmp(&b.provider))
            .then_with(|| a.model.cmp(&b.model))
    });
    previous.generated_at = partial.generated_at;
    previous.device = partial.device;
    previous.scan_ms = partial.scan_ms;
    previous.totals = Metrics::default();
    for row in &previous.rows {
        previous.totals.add(&row.metrics);
    }
    previous
}
