"""Checks the output of the end-to-end collector run (e2e-config.yaml): every golden span was
priced with the cost in expected.json and the pricing version of the prices.json in this
repository, and the signal_to_metrics connector produced the gen_ai.client.operation.cost
histogram from them.

    python check_e2e.py <out dir> <golden dir> <prices.json>
"""

import hashlib
import json
import math
import sys
from collections import Counter
from pathlib import Path


def lines(path):
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        if line.strip():
            yield json.loads(line)


def attrs(span):
    out = {}
    for a in span.get("attributes", []):
        v = a.get("value", {})
        out[a["key"]] = next(iter(v.values()), None) if v else None
    return out


def main(out_dir, golden_dir, prices_json):
    expected = json.loads((Path(golden_dir) / "expected.json").read_text(encoding="utf-8"))
    want = Counter()
    for suite in expected["suites"].values():
        for s in suite["spans"]:
            want[(s["row"], round(s["amount"], 12))] += 1
    version = "sha256:" + hashlib.sha256(Path(prices_json).read_bytes().replace(b"\r\n", b"\n")).hexdigest()

    got = Counter()
    total = 0.0
    for doc in lines(Path(out_dir) / "traces.json"):
        for rs in doc.get("resourceSpans", []):
            for ss in rs.get("scopeSpans", []):
                for span in ss.get("spans", []):
                    a = attrs(span)
                    assert "llm_prices.unpriced" not in a, (span["name"], a.get("llm_prices.unpriced"))
                    amount = float(a["gen_ai.usage.cost.amount"])
                    assert a["gen_ai.usage.cost.currency"] == "USD"
                    assert a["gen_ai.usage.cost.source"] == "local"
                    assert a["gen_ai.usage.cost.pricing.id"] == "llm-prices"
                    assert a["gen_ai.usage.cost.pricing.version"] == version, a["gen_ai.usage.cost.pricing.version"]
                    got[(a["llm_prices.price.row"], round(amount, 12))] += 1
                    total += amount
    assert got == want, f"priced spans {dict(got)} != expected {dict(want)}"

    count, hist_sum = 0, 0.0
    for doc in lines(Path(out_dir) / "metrics.json"):
        for rm in doc.get("resourceMetrics", []):
            for sm in rm.get("scopeMetrics", []):
                for m in sm.get("metrics", []):
                    if m["name"] != "gen_ai.client.operation.cost":
                        continue
                    assert m.get("unit") == "{cost}"
                    for dp in m["histogram"]["dataPoints"]:
                        count += int(dp["count"])
                        hist_sum += float(dp["sum"])
                        assert [float(b) for b in dp["explicitBounds"]] == [0.001, 0.004, 0.016, 0.064, 0.256, 1.024, 4.096, 16.384, 65.536]
    n = sum(want.values())
    assert count == n, f"histogram count {count}, want {n}"
    assert math.isclose(hist_sum, total, rel_tol=1e-9), (hist_sum, total)
    print(f"ok: {n} golden spans priced (total ${total:.6f}), histogram count {count} sum {hist_sum:.6f}, pricing.version {version}")


if __name__ == "__main__":
    main(*sys.argv[1:4])
