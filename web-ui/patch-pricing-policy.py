from pathlib import Path
import re

path = Path("src/app.tsx")
text = path.read_text(encoding="utf-8")

needle = "const FAST_SUBSCRIPTION_MULTIPLIER = 2.5;"
if needle not in text:
    raise SystemExit("FAST_SUBSCRIPTION_MULTIPLIER declaration not found")

replacement = r'''function subscriptionSpeedMultiplier(record: UsageRecord): number {
  const tier = String(record.tier || '').trim().toLowerCase();
  if (record.tool.trim().toLowerCase() !== 'codex' || (tier !== 'fast' && tier !== 'priority')) {
    return 1;
  }

  const model = normalizeModelId(record.model);
  // OpenAI Codex / ChatGPT Work official Speed rate card (2026-08):
  // GPT-5.6 and GPT-5.5 Fast consume 2.5x Standard credits; GPT-5.4 consumes 2x.
  // API-key Priority/Fast is deliberately NOT inferred here: it has separate API pricing.
  if (model.startsWith('gpt-5.6') || model.startsWith('gpt-5.5')) return 2.5;
  if (model.startsWith('gpt-5.4')) return 2;
  return 1;
}'''
text = text.replace(needle, replacement, 1)

pattern = re.compile(
    r"const\s+tier\s*=\s*(?P<row>[A-Za-z_$][A-Za-z0-9_$]*)\.tier\.trim\(\)\.toLowerCase\(\);\s*"
    r"const\s+tierMultiplier\s*=.*?FAST_SUBSCRIPTION_MULTIPLIER.*?;",
    re.MULTILINE | re.DOTALL,
)

match = pattern.search(text)
if not match:
    for m in re.finditer("FAST_SUBSCRIPTION_MULTIPLIER", text):
        start = max(0, m.start() - 600)
        end = min(len(text), m.end() + 900)
        print("--- FAST POLICY CONTEXT ---")
        print(text[start:end])
    raise SystemExit("generic Fast multiplier expression not found")
row = match.group("row")
text = text[: match.start()] + f"const tierMultiplier = subscriptionSpeedMultiplier({row});" + text[match.end() :]

text = text.replace(
    "Fast 2.5×",
    "Codex Fast: GPT-5.6/5.5 2.5× · GPT-5.4 2×",
)

if "FAST_SUBSCRIPTION_MULTIPLIER" in text:
    raise SystemExit("generic Fast multiplier survived patch")
if "subscriptionSpeedMultiplier" not in text:
    raise SystemExit("surface-aware speed policy missing")

path.write_text(text, encoding="utf-8")
