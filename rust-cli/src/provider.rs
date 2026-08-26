#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIdentity {
    pub upstream_vendor: String,
    pub route_provider: String,
    pub route_type: String,
}

fn norm(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace([' ', '_'], "-")
}

pub fn infer_upstream_vendor(model: &str) -> String {
    let m = norm(model);
    if m.contains("claude") || m.contains("anthropic") || m.contains("sonnet") || m.contains("opus") || m.contains("haiku") { return "anthropic".into(); }
    if m.contains("gpt") || m.contains("openai") || m.starts_with("o1") || m.starts_with("o3") || m.starts_with("o4") || m.contains("codex") { return "openai".into(); }
    if m.contains("gemini") || m.contains("google") { return "google".into(); }
    if m.contains("grok") { return "xai".into(); }
    if m.contains("deepseek") { return "deepseek".into(); }
    if m.contains("minimax") { return "minimax".into(); }
    if m.contains("mistral") || m.contains("mixtral") { return "mistral".into(); }
    if m.contains("llama") || m.starts_with("meta-") { return "meta".into(); }
    if m.contains("qwen") { return "qwen".into(); }
    if m.contains("kimi") || m.starts_with("k2") || m.starts_with("k3") { return "moonshotai".into(); }
    if m.contains("glm") { return "zai".into(); }
    if m.contains("mimo") { return "xiaomi".into(); }
    if m.contains("command-r") || m.contains("cohere") { return "cohere".into(); }
    if m.contains("nova-") || m.starts_with("amazon.") { return "amazon".into(); }
    "unknown".into()
}

fn official_provider_name(raw: &str) -> Option<&'static str> {
    match raw {
        "openai" | "openai-codex" => Some("openai"),
        "anthropic" => Some("anthropic"),
        "google" | "gemini" | "google-ai" => Some("google"),
        "xai" | "x-ai" => Some("xai"),
        "deepseek" | "deepseek-ai" => Some("deepseek"),
        "moonshot" | "moonshotai" | "kimi" | "kimi-for-coding" => Some("moonshotai"),
        "zai" | "z-ai" | "zhipu" | "bigmodel" => Some("zai"),
        "minimax" | "minimax-ai" => Some("minimax"),
        "mistral" | "mistralai" => Some("mistral"),
        "cohere" => Some("cohere"),
        "qwen" | "dashscope" | "alibaba" | "aliyun" => Some("qwen"),
        _ => None,
    }
}

/// Classify a route only from evidence supplied by the source session. `explicit`
/// must be false when a parser inferred provider_id from the model name.
pub fn classify(raw_provider: Option<&str>, model: &str, explicit: bool) -> ProviderIdentity {
    let upstream_vendor = infer_upstream_vendor(model);
    let raw = raw_provider.map(norm).filter(|v| !v.is_empty() && v != "unknown");
    let Some(raw) = raw else {
        return ProviderIdentity { upstream_vendor, route_provider: "unknown".into(), route_type: "unknown".into() };
    };

    let cloud = [
        (["aws", "bedrock", "amazon-bedrock"].as_slice(), "aws-bedrock"),
        (["azure", "azure-ai", "azure-openai"].as_slice(), "azure-openai"),
        (["vertex", "vertex-ai", "google-vertex", "gcp-vertex"].as_slice(), "google-vertex"),
    ];
    for (aliases, canonical) in cloud {
        if aliases.iter().any(|alias| raw == *alias || raw.contains(alias)) {
            return ProviderIdentity { upstream_vendor, route_provider: canonical.into(), route_type: "cloud".into() };
        }
    }

    if raw.contains("openrouter") {
        return ProviderIdentity { upstream_vendor, route_provider: "openrouter".into(), route_type: "aggregator".into() };
    }

    for (needle, canonical) in [
        ("newapi", "newapi"), ("new-api", "newapi"), ("oneapi", "oneapi"), ("one-api", "oneapi"),
        ("litellm", "litellm"), ("cli-proxy", "cliproxyapi"), ("cliproxy", "cliproxyapi"),
        ("relay", "custom-relay"), ("gateway", "custom-gateway"), ("proxy", "custom-proxy"),
    ] {
        if raw.contains(needle) {
            return ProviderIdentity { upstream_vendor, route_provider: canonical.into(), route_type: "relay".into() };
        }
    }

    for (needle, canonical) in [
        ("together", "together-ai"), ("fireworks", "fireworks-ai"), ("groq", "groq"),
        ("siliconflow", "siliconflow"), ("cloudflare", "cloudflare-workers-ai"), ("replicate", "replicate"),
        ("cerebras", "cerebras"), ("sambanova", "sambanova"), ("zenmux", "zenmux"),
        ("nano-gpt", "nano-gpt"), ("novita", "novita"),
    ] {
        if raw.contains(needle) {
            return ProviderIdentity { upstream_vendor, route_provider: canonical.into(), route_type: "inference-provider".into() };
        }
    }

    if raw.contains("ollama") || raw.contains("llama.cpp") || raw == "local" || raw.contains("localhost") {
        return ProviderIdentity { upstream_vendor, route_provider: raw, route_type: "self-hosted".into() };
    }

    if explicit {
        if let Some(official) = official_provider_name(&raw) {
            // The raw session itself named the first-party provider. Do not reach
            // this branch for a parser's model-name fallback.
            return ProviderIdentity { upstream_vendor, route_provider: official.into(), route_type: "official".into() };
        }
        // An explicit but otherwise unknown model provider (for example a custom
        // Codex `model_provider = "corp-gateway"`) is valuable routing evidence.
        return ProviderIdentity { upstream_vendor, route_provider: raw, route_type: "custom".into() };
    }

    ProviderIdentity { upstream_vendor, route_provider: raw, route_type: "unknown".into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinguishes_upstream_from_route() {
        let id = classify(Some("amazon-bedrock"), "claude-sonnet-4", true);
        assert_eq!(id.upstream_vendor, "anthropic");
        assert_eq!(id.route_provider, "aws-bedrock");
        assert_eq!(id.route_type, "cloud");
    }

    #[test]
    fn inferred_openai_is_not_called_official() {
        let id = classify(Some("openai"), "gpt-5.6-sol", false);
        assert_eq!(id.upstream_vendor, "openai");
        assert_eq!(id.route_type, "unknown");
    }

    #[test]
    fn explicit_custom_provider_is_preserved() {
        let id = classify(Some("my-newapi-gateway"), "gpt-5.6-sol", true);
        assert_eq!(id.route_provider, "newapi");
        assert_eq!(id.route_type, "relay");
    }
}
