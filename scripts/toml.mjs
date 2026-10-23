// Minimal TOML writer for prices.json. TOML has no null, so null fields are omitted.

function key(k) {
  return /^[A-Za-z0-9_-]+$/.test(k) ? k : JSON.stringify(k);
}

function scalar(v, k = '') {
  if (typeof v === 'string') return JSON.stringify(v);
  if (typeof v === 'number') return Number.isInteger(v) && !/tokens$/.test(k) ? `${v}.0` : String(v);
  if (typeof v === 'boolean') return String(v);
  if (Array.isArray(v)) return `[${v.map(scalar).join(', ')}]`;
  throw new Error(`unsupported TOML value ${v}`);
}

function isTable(v) {
  return v && typeof v === 'object' && !Array.isArray(v);
}

function emitTable(lines, path, obj, arrayHeader) {
  lines.push(arrayHeader ? `[[${path}]]` : `[${path}]`);
  const nested = [];
  for (const [k, v] of Object.entries(obj)) {
    if (v === null || v === undefined) continue;
    if (isTable(v)) nested.push([k, v]);
    else if (Array.isArray(v) && v.length && v.every(isTable)) nested.push([k, v]);
    else lines.push(`${key(k)} = ${scalar(v, k)}`);
  }
  for (const [k, v] of nested) {
    if (Array.isArray(v)) for (const item of v) emitTable(lines, `${path}.${key(k)}`, item, true);
    else emitTable(lines, `${path}.${key(k)}`, v, false);
  }
}

export function toToml(doc) {
  const lines = ['# Generated from prices.json by scripts/update.mjs. Do not edit by hand.', ''];
  for (const [k, v] of Object.entries(doc)) {
    if (k === 'models') continue;
    if (v !== null && !isTable(v)) lines.push(`${key(k)} = ${scalar(v)}`);
  }
  for (const [k, v] of Object.entries(doc)) {
    if (k !== 'models' && isTable(v)) {
      lines.push('');
      emitTable(lines, key(k), v, false);
    }
  }
  for (const m of doc.models) {
    lines.push('');
    emitTable(lines, 'models', m, true);
  }
  return lines.join('\n') + '\n';
}
