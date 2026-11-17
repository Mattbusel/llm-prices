"""Builds litellm_reference.json: costs computed by LiteLLM itself (generic_cost_per_token, the
function behind litellm.completion_cost for token-priced chat models) for a sample of entries
from LiteLLM's model_prices_and_context_window.json, so the LiteLLM price source of the Go
processor and of llm-prices-otel can be checked against the reference implementation.

    pip install litellm==<version>
    python make_reference.py model_prices_and_context_window.json litellm_reference.json

Entries priced with tiered_pricing tables or off-peak windows are left out (both sources report
them as unsupported_price), as are entries that are not chat or completion models.
"""

from __future__ import annotations

import json
import sys
from datetime import datetime, timezone

import litellm
from litellm.litellm_core_utils.llm_cost_calc.utils import generic_cost_per_token
from litellm.types.utils import CompletionTokensDetailsWrapper, PromptTokensDetailsWrapper, Usage

# llm-prices provider name for each LiteLLM provider prefix the sources map.
PROVIDERS = {
    "openai": "openai", "anthropic": "anthropic", "gemini": "google", "vertex_ai": "vertex",
    "bedrock": "bedrock", "bedrock_converse": "bedrock", "azure": "azure", "deepseek": "deepseek",
    "mistral": "mistral", "xai": "xai", "groq": "groq", "together_ai": "together",
    "fireworks_ai": "fireworks", "openrouter": "openrouter", "perplexity": "perplexity",
    "cohere_chat": "cohere",
}
TIERS = ("flex", "priority")


def provider_of(entry):
    lp = entry.get("litellm_provider") or ""
    for prefix, name in sorted(PROVIDERS.items(), key=lambda kv: -len(kv[0])):
        if lp == prefix or lp.startswith(prefix + "-") or lp.startswith(prefix + "_"):
            return prefix, name
    return None, None


def usable(entry):
    if not isinstance(entry, dict) or entry.get("mode") not in ("chat", "completion", "responses"):
        return False
    if "tiered_pricing" in entry or any("off_peak" in k for k in entry):
        return False
    if any(k.startswith(("input_cost_per_character", "input_cost_per_second", "input_cost_per_query")) for k in entry):
        return False
    return isinstance(entry.get("input_cost_per_token"), (int, float)) and isinstance(entry.get("output_cost_per_token"), (int, float))


def litellm_cost(model, entry, lp, uncached, read, write, out, reasoning, tier):
    prompt = uncached + read + write
    usage = Usage(
        prompt_tokens=prompt,
        completion_tokens=out,
        total_tokens=prompt + out,
        prompt_tokens_details=PromptTokensDetailsWrapper(cached_tokens=read, cache_creation_tokens=write, text_tokens=uncached),
        completion_tokens_details=CompletionTokensDetailsWrapper(reasoning_tokens=reasoning) if reasoning else None,
        cache_read_input_tokens=read,
        cache_creation_input_tokens=write,
    )
    p, c = generic_cost_per_token(
        model=model, usage=usage, custom_llm_provider=lp, service_tier=tier, model_info=entry,
        current_time=datetime(2026, 11, 17, 12, 0, tzinfo=timezone.utc),
    )
    return p + c


def main(src, dst):
    data = json.load(open(src, encoding="utf-8"))
    keys = sorted(k for k, v in data.items() if usable(v) and provider_of(v)[0] is not None)
    special = [k for k in keys if any("_above_" in f or f.endswith(TIERS) or f == "output_cost_per_reasoning_token" for f in data[k])]
    sample = sorted(set(keys[::20]) | set(special[::4]))
    scenarios = [
        (1000, 0, 0, 500, 0),
        (2000, 3000, 1000, 800, 300),
        (310000, 0, 0, 1000, 0),
        (150000, 60000, 0, 4000, 1000),
    ]
    entries, cases, skipped = {}, [], 0
    for k in sample:
        # Only the fields pricing reads, so the reference file stays small.
        e = {f: v for f, v in data[k].items() if "cost" in f or f in ("litellm_provider", "mode")}
        lp, name = provider_of(e)
        model = k.split("/", 1)[1] if "/" in k and k.split("/", 1)[0] == lp else k
        tiers = [None] + [t for t in TIERS if isinstance(e.get(f"input_cost_per_token_{t}"), (int, float))]
        for tier in tiers:
            for uncached, read, write, out, reasoning in scenarios:
                try:
                    want = litellm_cost(model, e, lp, uncached, read, write, out, reasoning, tier)
                except Exception:  # LiteLLM rejects the entry or the usage: nothing to compare
                    skipped += 1
                    continue
                entries[k] = e
                cases.append({"key": k, "provider": name, "litellm_provider": lp, "tier": tier,
                              "input": uncached, "cache_read": read, "cache_write": write,
                              "output": out, "reasoning": reasoning, "total": want})
    doc = {
        "description": "LiteLLM's own costs (generic_cost_per_token) for a sample of its price map; see make_reference.py.",
        "litellm_version": getattr(litellm, "__version__", None) or __import__("importlib.metadata").metadata.version("litellm"),
        "entries": entries,
        "cases": cases,
    }
    with open(dst, "w", encoding="utf-8", newline="\n") as f:
        json.dump(doc, f, separators=(",", ":"), sort_keys=True)
        f.write("\n")
    print(f"{len(entries)} entries, {len(cases)} cases, {skipped} skipped")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
