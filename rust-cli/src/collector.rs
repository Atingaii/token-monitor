use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use tokscale_core::pricing::PricingService;
use tokscale_core::{
    canonical_model_id, parse_local_clients, ClientId, LocalParseOptions, ParsedMessage,
    ScannerSettings, TokenBreakdown,
};

use crate::codex_tier;
use crate::evidence::{self, EvidenceBundle};
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
    cost_lower_bound: bool,
}

fn load_pricing() -> Option<Arc<PricingService>> {
    // Same fallback posture used by Tokscale's local reports: prefer an existing
    // cache for offline operation, otherwise initialize the canonical pricing
    // service. Token Monitor owns no general-purpose model price table.
    if let Some(cached) = PricingService::load_cached_any_age() {
        return Some(Arc::new(cached));
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
    runtime.block_on(async { PricingService::get_or_init().await.ok() })
}

/// Thin copy of Tokscale CLI's `compute_msg_cost`: all actual model/provider
/// matching and rate arithmetic stays inside PricingService.
fn message_cost(message: &ParsedMessage, pricing: Option<&PricingService>) -> f64 {
    let Some(pricing) = pricing else {
        return 0.0;
    };
    pricing.calculate_cost_with_provider(
        &message.model_id,
        Some(&message.provider_id),
        &TokenBreakdown {
            input: message.input,
            output: message.output,
            cache_read: message.cache_read,
            cache_write: message.cache_write,
            reasoning: message.reasoning,
        },
    )
}

fn normalize_text(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

fn load_scanner_settings() -> ScannerSettings {
    // Reuse Tokscale's persisted scanner schema when present. The file is
    // optional; ScannerSettings::default is the upstream-compatible baseline.
    let Some(config_dir) = dirs::config_dir() else {
        return ScannerSettings::default();
    };
    let Ok(data) = std::fs::read(config_dir.join("tokscale/settings.json")) else {
        return ScannerSettings::default();
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&data) else {
        return ScannerSettings::default();
    };
    value
        .get("scanner")
        .cloned()
        .and_then(|scanner| serde_json::from_value(scanner).ok())
        .unwrap_or_default()
}

/// Only claim parser-level provider evidence for clients whose Tokscale source
/// schema explicitly carries provider identity. Other clients remain unknown
/// unless the route-evidence adapter can prove it independently.
fn parser_provider_is_explicit(client: &str, raw_provider: &str) -> bool {
    !raw_provider.trim().is_empty()
        && raw_provider != "unknown"
        && matches!(client, "opencode" | "micode")
}

fn metrics_from_message(message: &ParsedMessage, pricing: Option<&PricingService>) -> Metrics {
    // ParsedMessage already contains Tokscale's normalized buckets. Do not
    // reinterpret raw client token fields here.
    Metrics {
        input: message.input.max(0),
        output: message.output.max(0),
        cache_read: message.cache_read.max(0),
        cache_write: message.cache_write.max(0),
        reasoning: message.reasoning.max(0),
        messages: message.message_count.max(0),
        cost_usd: message_cost(message, pricing).max(0.0),
    }
}

fn add_grouped(
    grouped: &mut BTreeMap<RowKey, RowAccumulator>,
    key: RowKey,
    metrics: Metrics,
    cost_lower_bound: bool,
) {
    // This is a direct analogue of Tokscale's DayAccumulator merge: additive
    // token/message buckets use saturating arithmetic; cost is summed.
    let entry = grouped.entry(key).or_default();
    entry.metrics.add(&metrics);
    entry.cost_lower_bound |= cost_lower_bound;
}

fn route_for_message(
    evidence: &EvidenceBundle,
    message: &ParsedMessage,
    client: &str,
    model: &str,
) -> (String, provider::ProviderIdentity) {
    let session_evidence = evidence::for_message(evidence, client, &message.session_id);
    let raw_provider = session_evidence
        .and_then(|item| item.explicit_provider.as_deref())
        .unwrap_or(message.provider_id.as_str());
    let explicit = session_evidence
        .and_then(|item| item.explicit_provider.as_ref())
        .is_some()
        || parser_provider_is_explicit(client, raw_provider);
    let identity = provider::classify(
        Some(raw_provider),
        model,
        explicit,
        session_evidence.and_then(|item| item.route_hint.as_ref()),
    );
    (normalize_text(raw_provider, "unknown"), identity)
}

fn canonical_codex_days(messages: &[ParsedMessage]) -> HashMap<String, codex_tier::DayReconciliation> {
    let mut days: HashMap<String, codex_tier::DayReconciliation> = HashMap::new();
    for message in messages.iter().filter(|message| message.client == "codex") {
        let metrics = metrics_from_message(message, None);
        let day = days.entry(message.date.clone()).or_insert_with(|| codex_tier::DayReconciliation {
            tokens: 0,
            messages: 0,
            all_priced: true,
        });
        day.tokens = day.tokens.saturating_add(metrics.total_tokens());
        day.messages = day.messages.saturating_add(metrics.messages);
    }
    days
}

fn reconciled_codex_days(
    canonical: &HashMap<String, codex_tier::DayReconciliation>,
    enhanced: &codex_tier::EnhancementResult,
) -> HashSet<String> {
    enhanced
        .by_date
        .iter()
        .filter_map(|(date, candidate)| {
            let upstream = canonical.get(date)?;
            (candidate.all_priced
                && candidate.tokens == upstream.tokens
                && candidate.messages == upstream.messages)
                .then(|| date.clone())
        })
        .collect()
}

pub fn supported_clients() -> Vec<String> {
    let mut clients: Vec<String> = ClientId::iter()
        .map(|client| client.as_str().to_string())
        .collect();
    clients.sort();
    clients
}

pub fn collect(device: DeviceInfo, since: Option<String>) -> Result<Ledger> {
    let started = Instant::now();
    let incremental = since.is_some();
    let scanner_settings = load_scanner_settings();

    // This is the single canonical accounting pass. Tokscale owns client
    // discovery, parsing, source caching/dedup, model attribution and token-bucket
    // normalization for every supported client.
    let parsed = parse_local_clients(LocalParseOptions {
        home_dir: None,
        use_env_roots: true,
        clients: None,
        since: since.clone(),
        until: None,
        year: None,
        scanner_settings: scanner_settings.clone(),
    })
    .map_err(|error| anyhow::anyhow!(error))
    .context("Tokscale failed to scan local AI coding-tool sessions")?;

    let route_evidence = evidence::scan(incremental);
    let pricing = load_pricing();

    // Optional Codex tier detail never becomes authoritative by itself. Its
    // daily totals must reconcile exactly with the Tokscale result before those
    // rows are allowed to replace the canonical Codex rows for that day.
    let canonical_codex = canonical_codex_days(&parsed.messages);
    let codex_enhancement = codex_tier::collect(since.as_deref(), &scanner_settings).unwrap_or_default();
    let accepted_codex_days = reconciled_codex_days(&canonical_codex, &codex_enhancement);

    let mut grouped: BTreeMap<RowKey, RowAccumulator> = BTreeMap::new();

    for message in &parsed.messages {
        if message.client == "codex" && accepted_codex_days.contains(&message.date) {
            continue;
        }
        let client = normalize_text(&message.client, "unknown");
        let model = canonical_model_id(&message.model_id);
        let (raw_provider, identity) = route_for_message(&route_evidence, message, &client, &model);
        add_grouped(
            &mut grouped,
            RowKey {
                date: message.date.clone(),
                client,
                provider: raw_provider,
                upstream_vendor: identity.upstream_vendor,
                route_provider: identity.route_provider,
                route_type: identity.route_type,
                model,
                tier: None,
            },
            metrics_from_message(message, pricing.as_deref()),
            false,
        );
    }

    for enhanced in codex_enhancement
        .rows
        .into_iter()
        .filter(|row| accepted_codex_days.contains(&row.date))
    {
        let model = canonical_model_id(&enhanced.model);
        let raw_provider = normalize_text(&enhanced.provider, "unknown");
        let route_hint = route_evidence
            .provider_hints
            .get(&raw_provider.to_ascii_lowercase());
        let identity = provider::classify(
            Some(&raw_provider),
            &model,
            raw_provider != "unknown",
            route_hint,
        );
        add_grouped(
            &mut grouped,
            RowKey {
                date: enhanced.date,
                client: "codex".to_string(),
                provider: raw_provider,
                upstream_vendor: identity.upstream_vendor,
                route_provider: identity.route_provider,
                route_type: identity.route_type,
                model,
                tier: Some(enhanced.tier),
            },
            enhanced.metrics,
            enhanced.cost_lower_bound,
        );
    }

    let mut rows = Vec::with_capacity(grouped.len());
    let mut totals = Metrics::default();
    for (key, value) in grouped {
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
            cost_lower_bound: value.cost_lower_bound,
            metrics: value.metrics,
        });
    }

    Ok(Ledger {
        schema_version: 3,
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
    previous.rows.sort_by(|left, right| {
        left.date
            .cmp(&right.date)
            .then_with(|| left.client.cmp(&right.client))
            .then_with(|| left.route_provider.cmp(&right.route_provider))
            .then_with(|| left.model.cmp(&right.model))
            .then_with(|| left.tier.cmp(&right.tier))
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

/// Used by the one-shot scheduler to avoid a GitHub write when the accounting
/// snapshot did not change. Volatile scan timestamps/version metadata do not
/// participate in this comparison.
pub fn same_accounting(left: &Ledger, right: &Ledger) -> bool {
    left.rows == right.rows && left.totals == right.totals
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

    #[test]
    fn codex_enhancement_requires_exact_reconciliation() {
        let canonical = HashMap::from([(
            "2026-08-26".to_string(),
            codex_tier::DayReconciliation { tokens: 100, messages: 2, all_priced: true },
        )]);
        let exact = codex_tier::EnhancementResult {
            rows: Vec::new(),
            by_date: HashMap::from([(
                "2026-08-26".to_string(),
                codex_tier::DayReconciliation { tokens: 100, messages: 2, all_priced: true },
            )]),
        };
        assert!(reconciled_codex_days(&canonical, &exact).contains("2026-08-26"));

        let mismatch = codex_tier::EnhancementResult {
            rows: Vec::new(),
            by_date: HashMap::from([(
                "2026-08-26".to_string(),
                codex_tier::DayReconciliation { tokens: 99, messages: 2, all_priced: true },
            )]),
        };
        assert!(!reconciled_codex_days(&canonical, &mismatch).contains("2026-08-26"));
    }
}
