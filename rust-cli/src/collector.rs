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
    plan_cost_available: bool,
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

fn usage_from_message(message: &ParsedMessage) -> TokenBreakdown {
    TokenBreakdown {
        input: message.input,
        output: message.output,
        cache_read: message.cache_read,
        cache_write: message.cache_write,
        reasoning: message.reasoning,
    }
}

/// General price arithmetic and coverage semantics remain delegated to Tokscale.
/// The separate plan estimate is never synthesized here.
fn price_usage(
    model_id: &str,
    provider_id: Option<&str>,
    usage: &TokenBreakdown,
    pricing: Option<&PricingService>,
) -> (f64, bool) {
    let Some(pricing) = pricing else {
        return (0.0, true);
    };
    let covered = pricing.covers_usage_with_provider(model_id, provider_id, usage);
    let cost = pricing
        .calculate_cost_with_provider(model_id, provider_id, usage)
        .max(0.0);
    (cost, !covered)
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

fn parser_provider_is_explicit(client: &str, raw_provider: &str) -> bool {
    !raw_provider.trim().is_empty()
        && raw_provider != "unknown"
        && matches!(client, "opencode" | "micode")
}

fn metrics_from_message(
    message: &ParsedMessage,
    pricing: Option<&PricingService>,
) -> (Metrics, bool) {
    let usage = usage_from_message(message);
    let (cost_usd, cost_lower_bound) = price_usage(
        &message.model_id,
        Some(&message.provider_id),
        &usage,
        pricing,
    );
    (
        Metrics {
            input: message.input.max(0),
            output: message.output.max(0),
            cache_read: message.cache_read.max(0),
            cache_write: message.cache_write.max(0),
            reasoning: message.reasoning.max(0),
            messages: message.message_count.max(0),
            cost_usd,
            plan_cost_usd: 0.0,
        },
        cost_lower_bound,
    )
}

fn add_grouped(
    grouped: &mut BTreeMap<RowKey, RowAccumulator>,
    key: RowKey,
    metrics: Metrics,
    cost_lower_bound: bool,
    plan_cost_available: bool,
) {
    let entry = grouped.entry(key).or_default();
    entry.metrics.add(&metrics);
    entry.cost_lower_bound |= cost_lower_bound;
    entry.plan_cost_available |= plan_cost_available;
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

fn canonical_codex_days(
    messages: &[ParsedMessage],
) -> HashMap<String, codex_tier::DayReconciliation> {
    let mut days = HashMap::new();
    for message in messages.iter().filter(|message| message.client == "codex") {
        let (metrics, _) = metrics_from_message(message, None);
        let day = days
            .entry(message.date.clone())
            .or_insert_with(|| codex_tier::DayReconciliation {
                tokens: 0,
                messages: 0,
                all_priced: true,
            });
        day.tokens = day.tokens.saturating_add(metrics.total_tokens());
        day.messages = day.messages.saturating_add(metrics.messages);
    }
    days
}

/// Tier subdivision is accepted only when additive token and message totals
/// exactly reconcile with Tokscale. Price completeness is not an acceptance
/// condition; incomplete price evidence remains an explicit lower bound.
fn reconciled_codex_days(
    canonical: &HashMap<String, codex_tier::DayReconciliation>,
    enhanced: &codex_tier::EnhancementResult,
) -> HashSet<String> {
    enhanced
        .by_date
        .iter()
        .filter_map(|(date, candidate)| {
            let upstream = canonical.get(date)?;
            (candidate.tokens == upstream.tokens && candidate.messages == upstream.messages)
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
    let canonical_codex = canonical_codex_days(&parsed.messages);
    let codex_enhancement =
        codex_tier::collect(since.as_deref(), &scanner_settings).unwrap_or_default();
    let accepted_codex_days = reconciled_codex_days(&canonical_codex, &codex_enhancement);
    let mut grouped: BTreeMap<RowKey, RowAccumulator> = BTreeMap::new();

    for message in &parsed.messages {
        if message.client == "codex" && accepted_codex_days.contains(&message.date) {
            continue;
        }
        let client = normalize_text(&message.client, "unknown");
        let model = canonical_model_id(&message.model_id);
        let (raw_provider, identity) =
            route_for_message(&route_evidence, message, &client, &model);
        let (metrics, cost_lower_bound) = metrics_from_message(message, pricing.as_deref());
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
            metrics,
            cost_lower_bound,
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
            enhanced.plan_cost_available,
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
            plan_cost_available: value.plan_cost_available,
            metrics: value.metrics,
        });
    }

    Ok(Ledger {
        // v4 guarantees one migration write from v1.0.0 so `public.json` is
        // created even when the underlying usage totals did not change.
        schema_version: 4,
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
    previous.schema_version = partial.schema_version;
    previous.generated_at = partial.generated_at;
    previous.device = partial.device;
    previous.scan_ms = partial.scan_ms;
    previous.totals = Metrics::default();
    for row in &previous.rows {
        previous.totals.add(&row.metrics);
    }
    previous
}

pub fn same_accounting(left: &Ledger, right: &Ledger) -> bool {
    left.schema_version == right.schema_version
        && left.device.id == right.device.id
        && left.rows == right.rows
        && left.totals == right.totals
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_ledger(schema_version: u32, device_id: &str) -> Ledger {
        Ledger {
            schema_version,
            generated_at: "2026-08-26T00:00:00Z".to_string(),
            device: DeviceInfo {
                id: device_id.to_string(),
                name: "test-device".to_string(),
                platform: "linux".to_string(),
                arch: "x86_64".to_string(),
                hostname: "test-host".to_string(),
                app_version: "1.0.1".to_string(),
            },
            rows: Vec::new(),
            totals: Metrics::default(),
            scan_ms: 1,
        }
    }

    #[test]
    fn every_tokscale_client_is_exposed() {
        let clients = supported_clients();
        assert!(clients.contains(&"codex".to_string()));
        assert!(clients.contains(&"claude".to_string()));
        assert!(clients.contains(&"opencode".to_string()));
        assert!(clients.contains(&"gemini".to_string()));
        assert!(
            clients.len() >= 20,
            "expected broad Tokscale client coverage, got {}",
            clients.len()
        );
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
    fn unknown_pricing_is_explicitly_a_lower_bound() {
        let service = PricingService::new(HashMap::new(), HashMap::new());
        let usage = TokenBreakdown {
            input: 100,
            output: 50,
            cache_read: 0,
            cache_write: 0,
            reasoning: 0,
        };
        let (cost, lower_bound) = price_usage(
            "token-monitor-definitely-unlisted-model",
            Some("unknown"),
            &usage,
            Some(&service),
        );
        assert_eq!(cost, 0.0);
        assert!(lower_bound);

        let (offline_cost, offline_lower_bound) =
            price_usage("any-model", Some("unknown"), &usage, None);
        assert_eq!(offline_cost, 0.0);
        assert!(offline_lower_bound);
    }

    #[test]
    fn codex_enhancement_requires_exact_accounting_not_complete_pricing() {
        let canonical = HashMap::from([(
            "2026-08-26".to_string(),
            codex_tier::DayReconciliation {
                tokens: 100,
                messages: 2,
                all_priced: true,
            },
        )]);
        let exact_but_lower_bound = codex_tier::EnhancementResult {
            rows: Vec::new(),
            by_date: HashMap::from([(
                "2026-08-26".to_string(),
                codex_tier::DayReconciliation {
                    tokens: 100,
                    messages: 2,
                    all_priced: false,
                },
            )]),
        };
        assert!(reconciled_codex_days(&canonical, &exact_but_lower_bound)
            .contains("2026-08-26"));

        let mismatch = codex_tier::EnhancementResult {
            rows: Vec::new(),
            by_date: HashMap::from([(
                "2026-08-26".to_string(),
                codex_tier::DayReconciliation {
                    tokens: 99,
                    messages: 2,
                    all_priced: true,
                },
            )]),
        };
        assert!(!reconciled_codex_days(&canonical, &mismatch).contains("2026-08-26"));
    }

    #[test]
    fn unchanged_snapshot_requires_same_schema_and_device() {
        let baseline = empty_ledger(3, "device-a");
        let mut volatile_only = baseline.clone();
        volatile_only.generated_at = "2026-08-26T00:15:00Z".to_string();
        volatile_only.scan_ms = 999;
        assert!(same_accounting(&baseline, &volatile_only));

        let mut schema_migration = baseline.clone();
        schema_migration.schema_version = 4;
        assert!(!same_accounting(&baseline, &schema_migration));

        let mut replacement_device = baseline.clone();
        replacement_device.device.id = "device-b".to_string();
        assert!(!same_accounting(&baseline, &replacement_device));
    }

    #[test]
    fn incremental_merge_adopts_current_schema() {
        let previous = empty_ledger(3, "device-a");
        let partial = empty_ledger(4, "device-a");
        let merged = merge_incremental(previous, partial, "2026-08-24");
        assert_eq!(merged.schema_version, 4);
    }
}
