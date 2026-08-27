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

  const model = normalizeModel(record.model);
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
    r"const\s+multiplier\s*=\s*tier\s*===\s*['\"]fast['\"]\s*\|\|\s*tier\s*===\s*['\"]priority['\"]\s*"
    r"\?\s*FAST_SUBSCRIPTION_MULTIPLIER\s*:\s*1\s*;",
    re.MULTILINE,
)

match = pattern.search(text)
if not match:
    raise SystemExit("generic Fast multiplier expression not found")
row = match.group("row")
text = text[: match.start()] + f"const multiplier = subscriptionSpeedMultiplier({row});" + text[match.end() :]

# Keep the official multiplier in the accounting logic, but do not surface
# implementation/source details or numeric multiplier guidance in the dashboard UI.
text = text.replace(
    "Codex Fast: GPT-5.6/5.5 2.5× · GPT-5.4 2×",
    "Fast",
)
text = text.replace(
    "Fast 2.5×",
    "Fast",
)
text = text.replace(
    "fastMultiplier: FAST_SUBSCRIPTION_MULTIPLIER,",
    "fastMultiplier: 2.5,",
)

if "FAST_SUBSCRIPTION_MULTIPLIER" in text:
    raise SystemExit("generic Fast multiplier survived patch")
if "subscriptionSpeedMultiplier" not in text:
    raise SystemExit("surface-aware speed policy missing")
if "Codex Fast: GPT-5.6/5.5 2.5× · GPT-5.4 2×" in text:
    raise SystemExit("internal Fast multiplier label survived patch")

path.write_text(text, encoding="utf-8")
