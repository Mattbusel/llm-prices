// Every file generated from prices.json, as { relative path: content }. build.mjs writes or
// checks them; update.mjs writes them after a refresh.

import { toToml } from './toml.mjs';
import { toRust } from './rust.mjs';
import { toMjs, toMjsTypes, toPython, toTs } from './packages.mjs';
import { toLitellm } from './litellm.mjs';
import { pricesVersion } from './version.mjs';

// `text` is prices.json itself, copied byte for byte into the crate and the Go module (go:embed
// cannot read files outside the module directory). Its SHA-256 is the PRICES_VERSION constant of
// the Rust, Python and JavaScript data (version.mjs).
export function generatedFiles(doc, text) {
  const version = pricesVersion(text);
  return {
    'prices.toml': toToml(doc),
    'crate/prices.json': text,
    'go/prices.json': text,
    'crate/src/data.rs': toRust(doc, version),
    'python/src/llm_prices/_data.py': toPython(doc, version),
    'js/src/data.ts': toTs(doc, version),
    'data.mjs': toMjs(doc, version),
    'data.d.mts': toMjsTypes(),
    'litellm.json': toLitellm(doc),
    'python/src/llm_prices/litellm.json': toLitellm(doc),
  };
}
