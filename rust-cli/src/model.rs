use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    pub reasoning: i64,
    pub messages: i32,
    pub sessions: i32,
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
        self.sessions = self.sessions.saturating_add(other.sessions);
        self.duration_ms = self.duration_ms.saturating_add(other.duration_ms);
        self.cost_usd += other.cost_usd;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageRow {
    pub date: String,
    pub client: String,
    pub provider: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    #[serde(flatten)]
    pub metrics: Metrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub arch: String,
    pub hostname: String,
    pub app_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
