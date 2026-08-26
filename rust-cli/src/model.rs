use serde::{Deserialize, Serialize};

fn is_false(value: &bool) -> bool {
    !*value
}

/// Additive accounting metrics only. These are the dimensions Tokscale exposes
/// with stable semantics across clients and that can be safely summed across
/// date/model/provider/device rows.
///
/// `cost_usd` is the current API-equivalent estimate. `plan_cost_usd` is a
/// separate included-plan / legacy-meter planning estimate when a source can
/// establish that billing basis (currently the Codex tier adapter). It is never
/// presented as an invoice.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    pub reasoning: i64,
    pub messages: i32,
    pub cost_usd: f64,
    #[serde(default)]
    pub plan_cost_usd: f64,
}

impl Metrics {
    pub fn total_tokens(&self) -> i64 {
        self.input
            .saturating_add(self.output)
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_write)
            .saturating_add(self.reasoning)
    }

    pub fn add(&mut self, other: &Metrics) {
        self.input = self.input.saturating_add(other.input);
        self.output = self.output.saturating_add(other.output);
        self.cache_read = self.cache_read.saturating_add(other.cache_read);
        self.cache_write = self.cache_write.saturating_add(other.cache_write);
        self.reasoning = self.reasoning.saturating_add(other.reasoning);
        self.messages = self.messages.saturating_add(other.messages);
        self.cost_usd += other.cost_usd;
        self.plan_cost_usd += other.plan_cost_usd;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageRow {
    pub date: String,
    pub client: String,
    /// Raw provider/routing identity emitted by the source parser. Kept for auditability.
    pub provider: String,
    /// Normalized upstream model vendor (OpenAI, Anthropic, Google, etc.).
    pub upstream_vendor: String,
    /// Normalized routing/billing provider (official vendor, AWS Bedrock, Azure, OpenRouter, relay, etc.).
    pub route_provider: String,
    /// One of official, cloud, aggregator, relay, inference-provider, self-hosted, custom, unknown.
    pub route_type: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    /// True when one or both displayed cost estimates are known lower bounds.
    #[serde(default, skip_serializing_if = "is_false")]
    pub cost_lower_bound: bool,
    /// True when this row has an independently established included-plan / legacy
    /// meter estimate. Rows without this flag must not be silently treated as
    /// having a zero plan cost.
    #[serde(default, skip_serializing_if = "is_false")]
    pub plan_cost_available: bool,
    #[serde(flatten)]
    pub metrics: Metrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub arch: String,
    pub hostname: String,
    pub app_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Ledger {
    pub schema_version: u32,
    pub generated_at: String,
    pub device: DeviceInfo,
    pub rows: Vec<UsageRow>,
    pub totals: Metrics,
    pub scan_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptedLedger {
    pub schema_version: u32,
    pub kind: String,
    pub device_hash: String,
    pub updated_at: String,
    pub algorithm: String,
    pub nonce: String,
    pub ciphertext: String,
}

/// Deliberately de-identified device metadata used by the public dashboard.
/// The local hostname, configured device name and original device id are never
/// copied into the public snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicDeviceInfo {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub arch: String,
    pub app_version: String,
}

/// Public, aggregate-only dashboard payload. It contains the same additive rows
/// the UI already displayed after decrypting the private envelope, but strips
/// local identity fields first. No session ids or content exist in `UsageRow`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicLedger {
    pub schema_version: u32,
    pub kind: String,
    pub generated_at: String,
    pub device: PublicDeviceInfo,
    pub rows: Vec<UsageRow>,
    pub totals: Metrics,
    pub scan_ms: u64,
}

impl PublicLedger {
    pub fn from_ledger(ledger: &Ledger, device_hash: &str) -> Self {
        let short = device_hash.get(..6).unwrap_or(device_hash);
        Self {
            schema_version: 1,
            kind: "token-monitor-public-ledger".to_string(),
            generated_at: ledger.generated_at.clone(),
            device: PublicDeviceInfo {
                id: device_hash.to_string(),
                name: format!("{}-{short}", ledger.device.platform),
                platform: ledger.device.platform.clone(),
                arch: ledger.device.arch.clone(),
                app_version: ledger.device.app_version.clone(),
            },
            rows: ledger.rows.clone(),
            totals: ledger.totals.clone(),
            scan_ms: ledger.scan_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_ledger_strips_local_device_identity_but_keeps_aggregates() {
        let metrics = Metrics {
            input: 10,
            output: 3,
            cache_read: 90,
            cache_write: 0,
            reasoning: 2,
            messages: 1,
            cost_usd: 0.25,
            plan_cost_usd: 0.40,
        };
        let ledger = Ledger {
            schema_version: 4,
            generated_at: "2026-08-26T00:00:00Z".into(),
            device: DeviceInfo {
                id: "raw-secret-device-id".into(),
                name: "Lucent Personal Mac".into(),
                platform: "macos".into(),
                arch: "aarch64".into(),
                hostname: "private-host.local".into(),
                app_version: "1.0.1".into(),
            },
            rows: vec![UsageRow {
                date: "2026-08-26".into(),
                client: "codex".into(),
                provider: "openai".into(),
                upstream_vendor: "openai".into(),
                route_provider: "openai".into(),
                route_type: "official".into(),
                model: "gpt-5.6-sol".into(),
                tier: Some("fast".into()),
                cost_lower_bound: false,
                plan_cost_available: true,
                metrics: metrics.clone(),
            }],
            totals: metrics,
            scan_ms: 12,
        };

        let public = PublicLedger::from_ledger(&ledger, "5e4056ad24282d75");
        let json = serde_json::to_string(&public).unwrap();
        assert_eq!(public.device.id, "5e4056ad24282d75");
        assert_eq!(public.device.name, "macos-5e4056");
        assert_eq!(public.rows[0].model, "gpt-5.6-sol");
        assert_eq!(public.totals.total_tokens(), 105);
        assert!(!json.contains("raw-secret-device-id"));
        assert!(!json.contains("Lucent Personal Mac"));
        assert!(!json.contains("private-host.local"));
        assert!(!json.contains("hostname"));
    }
}
