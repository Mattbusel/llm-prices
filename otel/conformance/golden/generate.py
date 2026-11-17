"""Regenerates the golden corpus: real spans exported by OpenLLMetry, OpenInference and Logfire.

Each run starts a local mock of the OpenAI and Anthropic HTTP APIs (fixed responses with known
token usage), drives the real `openai` and `anthropic` SDKs at it with one instrumentation suite
enabled, and writes every span the suite exported as OTLP JSON (the encoding of the OTLP/HTTP
JSON protocol, the file the Collector's otlpjsonfile receiver reads):

    python generate.py openllmetry  openllmetry.otlp.json
    python generate.py openinference openinference.otlp.json
    python generate.py logfire      logfire.otlp.json

rig (Rust) runs against the same mock: start it with `python generate.py serve 8765`, then
`cargo run --release --manifest-path rig/Cargo.toml > rig.otlp.json` (see rig/src/main.rs).

Install the suite first, in its own virtual environment:

    openllmetry:   openai anthropic opentelemetry-sdk opentelemetry-instrumentation-openai opentelemetry-instrumentation-anthropic opentelemetry-exporter-otlp-proto-common
    openinference: openai anthropic opentelemetry-sdk openinference-instrumentation-openai openinference-instrumentation-anthropic opentelemetry-exporter-otlp-proto-common
    logfire:       openai anthropic logfire opentelemetry-exporter-otlp-proto-common

The spans are written as exported, apart from the trace and span ids, which OTLP JSON writes in
hex. expected.json holds the cost each priced span must get, computed by hand from the mock usage.
"""

from __future__ import annotations

import json
import os
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any, Dict

# Fixed usage per model. The costs in expected.json are these counts times the llm-prices rows.
OPENAI_USAGE: Dict[str, Dict[str, Any]] = {
    "gpt-4o-mini": {
        "served": "gpt-4o-mini-2024-07-18",
        "usage": {"prompt_tokens": 1200, "completion_tokens": 300, "total_tokens": 1500,
                  "prompt_tokens_details": {"cached_tokens": 1000, "audio_tokens": 0},
                  "completion_tokens_details": {"reasoning_tokens": 0, "audio_tokens": 0}},
    },
    "o3": {
        "served": "o3-2025-04-16",
        "usage": {"prompt_tokens": 10000, "completion_tokens": 2000, "total_tokens": 12000,
                  "prompt_tokens_details": {"cached_tokens": 0, "audio_tokens": 0},
                  "completion_tokens_details": {"reasoning_tokens": 1500, "audio_tokens": 0}},
    },
}
ANTHROPIC_USAGE = {
    "input_tokens": 2048,
    "cache_read_input_tokens": 45000,
    "cache_creation_input_tokens": 3500,
    "cache_creation": {"ephemeral_5m_input_tokens": 3500, "ephemeral_1h_input_tokens": 0},
    "output_tokens": 1024,
}


class MockLLM(BaseHTTPRequestHandler):
    def log_message(self, *args: Any) -> None:
        pass

    def _send(self, body: Dict[str, Any]) -> None:
        data = json.dumps(body).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_POST(self) -> None:  # noqa: N802 - http.server API
        req = json.loads(self.rfile.read(int(self.headers.get("Content-Length", "0"))) or b"{}")
        if self.path.endswith("/chat/completions"):
            spec = OPENAI_USAGE[req["model"]]
            self._send({
                "id": "chatcmpl-mock", "object": "chat.completion", "created": 1763380000,
                "model": spec["served"], "service_tier": "default", "system_fingerprint": "fp_mock",
                "choices": [{"index": 0, "message": {"role": "assistant", "content": "ok"}, "finish_reason": "stop"}],
                "usage": spec["usage"],
            })
        elif self.path.endswith("/messages"):
            self._send({
                "id": "msg_mock", "type": "message", "role": "assistant", "model": req["model"],
                "content": [{"type": "text", "text": "ok"}], "stop_reason": "end_turn", "stop_sequence": None,
                "usage": ANTHROPIC_USAGE,
            })
        else:
            self.send_error(404)


def run(suite: str) -> Any:
    from opentelemetry.sdk.trace import TracerProvider
    from opentelemetry.sdk.trace.export import SimpleSpanProcessor
    from opentelemetry.sdk.trace.export.in_memory_span_exporter import InMemorySpanExporter

    server = ThreadingHTTPServer(("127.0.0.1", 0), MockLLM)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    port = server.server_address[1]
    exporter = InMemorySpanExporter()

    os.environ["TRACELOOP_TRACE_CONTENT"] = "false"
    if suite == "openllmetry":
        from opentelemetry.instrumentation.anthropic import AnthropicInstrumentor
        from opentelemetry.instrumentation.openai import OpenAIInstrumentor

        tp = TracerProvider()
        tp.add_span_processor(SimpleSpanProcessor(exporter))
        OpenAIInstrumentor().instrument(tracer_provider=tp)
        AnthropicInstrumentor().instrument(tracer_provider=tp)
    elif suite == "openinference":
        from openinference.instrumentation.anthropic import AnthropicInstrumentor
        from openinference.instrumentation.openai import OpenAIInstrumentor

        tp = TracerProvider()
        tp.add_span_processor(SimpleSpanProcessor(exporter))
        OpenAIInstrumentor().instrument(tracer_provider=tp)
        AnthropicInstrumentor().instrument(tracer_provider=tp)
    elif suite == "logfire":
        import logfire

        logfire.configure(send_to_logfire=False, console=False, additional_span_processors=[SimpleSpanProcessor(exporter)])
        logfire.instrument_openai()
        logfire.instrument_anthropic()
    else:
        raise SystemExit(f"unknown suite {suite}")

    import anthropic
    import openai

    oa = openai.OpenAI(base_url=f"http://127.0.0.1:{port}/v1", api_key="test-key")
    for model in ("gpt-4o-mini", "o3"):
        oa.chat.completions.create(model=model, messages=[{"role": "user", "content": "hi"}])
    an = anthropic.Anthropic(base_url=f"http://127.0.0.1:{port}", api_key="test-key")
    an.messages.create(model="claude-sonnet-4-6", max_tokens=16, messages=[{"role": "user", "content": "hi"}])
    server.shutdown()
    return exporter.get_finished_spans()


def to_otlp_json(spans: Any) -> Dict[str, Any]:
    from google.protobuf.json_format import MessageToDict
    from opentelemetry.exporter.otlp.proto.common.trace_encoder import encode_spans

    doc = MessageToDict(encode_spans(spans), use_integers_for_enums=True)

    def hexify(o: Any) -> Any:
        if isinstance(o, dict):
            for k, v in list(o.items()):
                if k in ("traceId", "spanId", "parentSpanId") and isinstance(v, str):
                    import base64

                    o[k] = base64.b64decode(v).hex()
                else:
                    hexify(v)
        elif isinstance(o, list):
            for x in o:
                hexify(x)
        return o

    return hexify(doc)


if __name__ == "__main__":
    if sys.argv[1] == "serve":
        # The mock alone, for the rig generator (rig/): python generate.py serve 8765
        ThreadingHTTPServer(("127.0.0.1", int(sys.argv[2])), MockLLM).serve_forever()
    suite, out = sys.argv[1], sys.argv[2]
    doc = to_otlp_json(run(suite))
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        json.dump(doc, f, indent=1, sort_keys=True)
        f.write("\n")
    n = sum(len(ss.get("spans", [])) for rs in doc.get("resourceSpans", []) for ss in rs.get("scopeSpans", []))
    print(f"{suite}: {n} spans -> {out}")
