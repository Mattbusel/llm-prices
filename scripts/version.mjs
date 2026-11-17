// The prices version: "sha256:" and the SHA-256 of prices.json as UTF-8 text with LF line
// endings, in lowercase hex. Every binding carries it as PRICES_VERSION (the Go module computes
// it from its embedded copy at start-up) and every release states it in its notes, so a cost
// computed from this table can name the exact table that produced it
// (`gen_ai.usage.cost.pricing.version`). Check a downloaded copy with:
//
//   sha256sum prices.json

import { createHash } from 'node:crypto';

export function pricesVersion(text) {
  if (typeof text !== 'string') throw new TypeError('pricesVersion needs the text of prices.json');
  return `sha256:${createHash('sha256').update(text.replace(/\r\n/g, '\n'), 'utf8').digest('hex')}`;
}

// Throws unless `version` looks like a prices version, so no generator writes an empty one.
export function checkVersion(version) {
  if (typeof version !== 'string' || !/^sha256:[0-9a-f]+$/.test(version)) {
    throw new TypeError(`a generator needs the prices version (pricesVersion(text)), got ${JSON.stringify(version)}`);
  }
  return version;
}
