use serde::{Deserialize, Serialize};

/// Additive metrics only. Every field can be safely summed across
/// date/model/provider/device rows. Non-additive values such as distinct session
/// count intentionally do not live here.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    pub reasoning: i64,
    pub messages: i32,
    pub duration_ms: i64,
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
        self.duration_ms = self.duration_ms.saturating_add(other.duration_ms);
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
    /// `true` only when a specialized source could prove that this row's cost
    /// is a lower bound (for example missing Codex cache-write counts).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
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
