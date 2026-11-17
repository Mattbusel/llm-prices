"""Checks Weaver's live-check report (weaver-config.yaml run) for the attributes the genaicost
processor writes, against the GenAI registry of semantic-conventions-genai#443:

- gen_ai.usage.cost.amount, .currency and .source are on every priced span and their only
  finding is not_stable (the proposal marks them development): right types, valid enum value.
- gen_ai.usage.cost.pricing.id and .pricing.version (proposed in #624, not in the #443 registry)
  are reported as missing_attribute and extends_namespace and nothing else.

    python check_weaver.py live_check.json <number of priced spans>
"""

import json
import sys
from collections import Counter, defaultdict

COST = ("gen_ai.usage.cost.amount", "gen_ai.usage.cost.currency", "gen_ai.usage.cost.source")
PRICING = ("gen_ai.usage.cost.pricing.id", "gen_ai.usage.cost.pricing.version")


def main(report, spans):
    spans = int(spans)
    doc = json.load(open(report, encoding="utf-8"))
    seen = Counter()
    advice = defaultdict(Counter)

    def walk(o):
        if isinstance(o, dict):
            name = o.get("name")
            if isinstance(name, str) and "live_check_result" in o and name.startswith("gen_ai.usage.cost"):
                seen[name] += 1
                for a in (o["live_check_result"] or {}).get("all_advice", []):
                    advice[name][(a["id"], a["level"])] += 1
            for v in o.values():
                walk(v)
        elif isinstance(o, list):
            for v in o:
                walk(v)

    walk(doc)
    for k in COST + PRICING:
        assert seen[k] == spans, f"{k} seen on {seen[k]} spans, want {spans}"
    for k in COST:
        assert set(advice[k]) <= {("not_stable", "improvement")}, (k, dict(advice[k]))
    for k in PRICING:
        assert set(advice[k]) <= {("missing_attribute", "violation"), ("extends_namespace", "information")}, (k, dict(advice[k]))
    print(f"ok: {spans} spans; cost attributes match the #443 registry (only finding: not_stable); "
          f"pricing.id/version reported as not in that registry, as expected until #624 lands")
    for k in COST + PRICING:
        print(f"  {k}: {dict(advice[k])}")


if __name__ == "__main__":
    main(*sys.argv[1:3])
