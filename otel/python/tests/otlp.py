"""Reads OTLP JSON (the golden corpus) into (span name, attributes) pairs for the tests."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Dict, Iterator, List, Tuple


def _value(v: Dict[str, Any]) -> Any:
    if "stringValue" in v:
        return v["stringValue"]
    if "intValue" in v:
        return int(v["intValue"])
    if "doubleValue" in v:
        return float(v["doubleValue"])
    if "boolValue" in v:
        return bool(v["boolValue"])
    if "arrayValue" in v:
        return tuple(_value(x) for x in v["arrayValue"].get("values", []))
    if "kvlistValue" in v:
        return {kv["key"]: _value(kv.get("value", {})) for kv in v["kvlistValue"].get("values", [])}
    if "bytesValue" in v:
        return v["bytesValue"]
    return None


def spans(path: Path) -> Iterator[Tuple[str, str, Dict[str, Any]]]:
    """(scope name, span name, attributes) of every span in an OTLP JSON file."""
    doc = json.loads(path.read_text(encoding="utf-8"))
    for rs in doc.get("resourceSpans", []):
        for ss in rs.get("scopeSpans", []):
            scope = ss.get("scope", {}).get("name", "")
            for s in ss.get("spans", []):
                attrs = {a["key"]: _value(a.get("value", {})) for a in s.get("attributes", [])}
                yield scope, s["name"], attrs


def all_spans(path: Path) -> List[Tuple[str, str, Dict[str, Any]]]:
    return list(spans(path))
