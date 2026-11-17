"""LiteLLM loads litellm.json (register_model) and prices calls with it exactly as prices.json does.

    pip install litellm
    python test/litellm_check.py litellm.json
"""
import json, sys, os
os.environ["LITELLM_LOCAL_MODEL_COST_MAP"] = "True"
import litellm
from decimal import Decimal
path = sys.argv[1]
table = json.load(open(path, encoding="utf-8"))
table.pop("sample_spec")
litellm.register_model(table)
cases = [
    ("claude-sonnet-4-6", 1000, 500, Decimal("3")*1000/10**6 + Decimal("15")*500/10**6),
    ("gpt-4o", 2000, 100, Decimal("2.5")*2000/10**6 + Decimal("10")*100/10**6),
    ("gemini/gemini-2.5-pro", 300000, 1000, Decimal("2.5")*300000/10**6 + Decimal("15")*1000/10**6),
    ("deepseek/deepseek-v4-pro", 1000, 1000, Decimal("1.32")/1000 + Decimal("3.96")/1000),
    ("claude-haiku-5-5", 150000, 10, Decimal("0.5")*150000/10**6 + Decimal("2.5")*10/10**6),
]
ok = True
for model, pt, ct, want in cases:
    a, b = litellm.cost_per_token(model=model, prompt_tokens=pt, completion_tokens=ct)
    got = Decimal(repr(a + b))
    good = abs(got - want) < Decimal("1e-12")
    ok &= good
    print(f"{model}: litellm {a+b:.10f} want {want} {'OK' if good else 'MISMATCH'}")
# priority service tier through litellm
a, b = litellm.cost_per_token(model="gemini/gemini-2.5-pro", prompt_tokens=1000, completion_tokens=1000, service_tier="priority")
want_priority = Decimal("2.25") * 1000 / 10**6 + Decimal("18") * 1000 / 10**6
good = abs(Decimal(repr(a + b)) - want_priority) < Decimal("1e-12")
ok &= good
print(f"gemini/gemini-2.5-pro service_tier=priority: litellm {a+b:.10f} want {want_priority} {'OK' if good else 'MISMATCH'}")
print("registered", sum(1 for k in table if k in litellm.model_cost), "of", len(table))
print("ALL OK" if ok else "FAIL")
sys.exit(0 if ok else 1)
