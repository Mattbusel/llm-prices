// Every file generated from prices.json, as { relative path: content }. build.mjs writes or
// checks them; update.mjs writes them after a refresh.

import { toToml } from './toml.mjs';
import { toRust } from './rust.mjs';
import { toPython, toTs } from './packages.mjs';

// `text` is prices.json itself, copied byte for byte into the crate.
export function generatedFiles(doc, text) {
  return {
    'prices.toml': toToml(doc),
    'crate/prices.json': text,
    'crate/src/data.rs': toRust(doc),
    'python/src/llm_prices/_data.py': toPython(doc),
    'js/src/data.ts': toTs(doc),
  };
}
