use serde::{Deserialize, Serialize};

fn is_false(value: &bool) -> bool {
    !*value
}

/// Additive accounting metrics only. These are the dimensions Tokscale exposes
/// with stable semantics across clients and that can be safely summed across
/// date/model/provider/device rows.
///
/// Deliberately excluded: distinct session count and duration/performance. Those
/// are not universally additive across clients or grouping dimensions.
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
    /// True only when a specialized source can prove the cost is a lower bound
    /// (for example a Codex record whose cache-write token count is absent).
    #[serde(default, skip_serializing_if = "is_false")]
    pub cost_lower_bound: bool,
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

/// Browser-readable aggregate snapshot. It intentionally contains only the same
/// aggregate accounting/routing dimensions shown in the dashboard and omits the
/// machine hostname and every raw session payload. The full ledger remains
/// AES-256-GCM encrypted in `ledger.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PublicDeviceInfo {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub arch: String,
    pub app_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

impl From<&Ledger> for PublicLedger {
    fn from(ledger: &Ledger) -> Self {
        Self {
            schema_version: 1,
            kind: "token-monitor-public-ledger".to_string(),
            generated_at: ledger.generated_at.clone(),
            device: PublicDeviceInfo {
                id: ledger.device.id.clone(),
                name: ledger.device.name.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_ledger_omits_hostname_but_preserves_aggregate_rows() {
        let ledger = Ledger {
            schema_version: 3,
            generated_at: "2026-08-26T00:00:00Z".into(),
            device: DeviceInfo {
                id: "device-1".into(),
                name: "Laptop".into(),
                platform: "macos".into(),
                arch: "aarch64".into(),
                hostname: "secret-host.local".into(),
                app_version: "1.0.1".into(),
            },
            rows: Vec::new(),
            totals: Metrics::default(),
            scan_ms: 10,
        };
        let public = PublicLedger::from(&ledger);
        let json = serde_json::to_string(&public).unwrap();
        assert!(json.contains("token-monitor-public-ledger"));
        assert!(!json.contains("secret-host.local"));
        assert_eq!(public.device.name, "Laptop");
    }
}
