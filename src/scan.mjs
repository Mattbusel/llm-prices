// Finds hard-coded LLM prices in source files and compares them with prices.json.
// Node standard library only. See README "How matching works" for the rules.

const PREFIX = /^(?:(?:models|anthropic|openai|google|gemini|vertex_ai|vertex|bedrock|azure|openrouter|deepseek|litellm)\/)+/;
const BEDROCK = /^(?:(?:us|eu|apac|au|jp|global)\.)?anthropic\./;
const DATE8 = /-(20\d{6})$/;

/** Builds a lookup from every id form a user might write to its prices.json row. */
export function buildIndex(prices, providers) {
  const want = providers && providers.length ? new Set(providers) : null;
  const exact = new Map();
  const undated = new Map();
  for (const row of prices.models) {
    if (want && !want.has(row.provider)) continue;
    for (const id of [row.model, ...(row.aliases || [])]) {
      exact.set(id.toLowerCase(), row);
      // Anthropic publishes dated ids for older models; people often write the undated form.
      if (row.provider === 'anthropic' && DATE8.test(id)) undated.set(id.toLowerCase().replace(DATE8, ''), row);
    }
  }
  return { exact, undated };
}

/** Maps a raw token from a file to a row, or null. */
export function resolveModel(index, token) {
  let t = token.toLowerCase().replace(/[.:\/-]+$/, '');
  if (index.exact.has(t)) return index.exact.get(t);
  t = t.replace(PREFIX, '').replace(BEDROCK, '').replace(/-v\d+(?::\d+)?$/, '').replace(/@(20\d{6})$/, '-$1');
  if (index.exact.has(t)) return index.exact.get(t);
  if (index.undated.has(t)) return index.undated.get(t);
  const und = t.replace(DATE8, '');
  if (und !== t) {
    const r = index.exact.get(und) || index.undated.get(und);
    if (r && r.provider === 'anthropic') return r;
  }
  return null;
}

const TOKEN = /[A-Za-z0-9][A-Za-z0-9._:@\/-]{1,90}/g;

/** Known model ids on one line: [{ row, col, quoted, asKey, asValue }]. */
export function modelsOnLine(index, line) {
  const out = [];
  TOKEN.lastIndex = 0;
  let m;
  while ((m = TOKEN.exec(line))) {
    const row = resolveModel(index, m[0]);
    if (!row) continue;
    const before = line.slice(0, m.index);
    const after = line.slice(m.index + m[0].length);
    const quoted = /["'`]$/.test(before) && /^["'`]/.test(after);
    // Very short ids (o1, o3, gpt-5) must be quoted to count; they collide with ordinary words.
    if (row.model.length < 6 && !quoted && !/^\s*[:=]/.test(after)) continue;
    const asKey = /^["'`]?\s*(?::|=>|=(?!=)|\]|\)\s*=>)/.test(after) || /^\s*\[/.test(line) && /\]\s*$/.test(line.trim()) || /\bcase\s+["'`]?$/.test(before) || /\(\s*["'`]?$/.test(before);
    const asValue = /["']?(?:model|model_id|modelid|modelname|model_name|id|name|slug)["']?\s*[:=]\s*["'`]?$/i.test(before);
    out.push({ row, col: m.index, token: m[0], quoted, asKey, asValue });
  }
  return out;
}

function stripStrings(line) {
  return line.replace(/"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|`[^`]*`/g, '""');
}

function depthDelta(line) {
  const s = stripStrings(line);
  let d = 0;
  for (const ch of s) {
    if (ch === '{' || ch === '[' || ch === '(') d++;
    else if (ch === '}' || ch === ']' || ch === ')') d--;
  }
  return d;
}

function indent(line) {
  return line.match(/^\s*/)[0].replace(/\t/g, '    ').length;
}

const MAX_BLOCK = 20;

/** Returns [startLine, endLine] (inclusive) of the price block that belongs to the model on line i. */
export function blockFor(lines, i, hit, ext) {
  const line = lines[i];
  const isIndentLang = ext === 'yaml' || ext === 'yml' || ext === 'py';
  // TOML table headers: [pricing."gpt-4o"] or [models.gpt-4o] own everything to the next header.
  if (ext === 'toml' && /^\s*\[/.test(line)) {
    let j = i + 1;
    while (j < lines.length && j - i <= MAX_BLOCK && !/^\s*\[/.test(lines[j])) j++;
    return [i, j - 1];
  }
  if (hit.asValue) {
    // The model is a field of a record; the block is the record around it.
    if (ext === 'toml') {
      let a = i;
      while (a > 0 && i - a < MAX_BLOCK && !/^\s*\[/.test(lines[a]) && lines[a].trim() !== '') a--;
      let b = i + 1;
      while (b < lines.length && b - i <= MAX_BLOCK && !/^\s*\[/.test(lines[b]) && lines[b].trim() !== '') b++;
      return [a, b - 1];
    }
    if (/^\s*-\s/.test(line) || (isIndentLang && ext !== 'py')) {
      const base = /^\s*-\s/.test(line) ? indent(line) : indent(line) - 2;
      let a = i;
      if (!/^\s*-\s/.test(line)) {
        while (a > 0 && i - a < MAX_BLOCK && indent(lines[a - 1]) > base && !/^\s*-\s/.test(lines[a])) a--;
        if (a > 0 && /^\s*-\s/.test(lines[a - 1])) a--;
      }
      let b = i + 1;
      while (b < lines.length && b - i <= MAX_BLOCK && lines[b].trim() !== '' && indent(lines[b]) > (indent(lines[a]))) b++;
      return [a, b - 1];
    }
    // Bracketed record: walk back to its opening brace and forward to its close.
    let a = i;
    let d = 0;
    const pre = line.slice(0, hit.col);
    d += depthDelta(pre);
    while (d <= 0 && a > 0 && i - a < MAX_BLOCK) {
      a--;
      d += depthDelta(lines[a]);
    }
    let b = i;
    let e = depthDelta(line.slice(hit.col));
    if (d > 0) {
      e = d + e;
      while (e > 0 && b + 1 < lines.length && b - i < MAX_BLOCK) {
        b++;
        e += depthDelta(lines[b]);
      }
    }
    return [a, b];
  }
  // The model is a key (or a match arm, case label, tuple head): the block is what follows it.
  const after = line.slice(hit.col);
  let d = depthDelta(after);
  let b = i;
  if (d > 0) {
    while (d > 0 && b + 1 < lines.length && b - i < MAX_BLOCK) {
      b++;
      d += depthDelta(lines[b]);
    }
    return [i, b];
  }
  if (/:\s*(#.*)?$/.test(line.trim()) || /=>\s*$/.test(line.trim())) {
    const base = indent(line);
    while (b + 1 < lines.length && b - i < MAX_BLOCK && lines[b + 1].trim() !== '' && indent(lines[b + 1]) > base) b++;
    return [i, b];
  }
  return [i, i];
}

const NUM = String.raw`-?\d[\d_]*(?:\.\d+)?(?:[eE][-+]?\d+)?`;
const DIV = String.raw`(?:\s*\/\s*(1_?000_?000(?:\.0*)?|1e6|1\.0e6|1_?000(?:\.0*)?|1e3|1\.0e3))?`;
const KV = new RegExp(
  String.raw`["']?([A-Za-z_][A-Za-z0-9_\-.]{0,60}?)["']?\s*(?::|=>|=(?!=))\s*(?:[A-Za-z_][A-Za-z0-9_:!]*\(\s*)?["']?\$?(` + NUM + String.raw`)["']?` + DIV,
  'g',
);
// "(input, output" right after a model id: "m" => (3.0, 15.0), ("m", 3.0, 15.0), "m": [2.5, 10],
// "m" => Price::new(3.0, 15.0). Only punctuation or one constructor name may sit between the id
// and the first number, and the two numbers must be adjacent.
const POS = new RegExp(
  String.raw`^["'\`]?[\s,:=>)\]]*(?:[A-Za-z_][\w:!]*\s*)?[(\[{]*\s*(?:[A-Za-z_][\w:!]*\(\s*)?\$?(` + NUM + String.raw`)` + DIV + String.raw`\s*,\s*(?:[A-Za-z_][\w:!]*\(\s*)?\$?(` + NUM + String.raw`)` + DIV + String.raw`(?![\w.])`,
);

function parseNum(s) {
  return Number(String(s).replace(/_/g, ''));
}

function divUnit(div) {
  if (!div) return null;
  const n = parseNum(div.replace(/e/i, 'e'));
  if (n === 1e6) return 'M';
  if (n === 1e3) return 'K';
  return null;
}

/** Classifies a key name to a price field, or null when it is not a price. */
export function classifyKey(rawKey) {
  const k = rawKey.toLowerCase().replace(/[^a-z0-9]+/g, '_');
  const hasMoney = /cost|price|rate|usd|dollar|per_|_per|mtok|fee/.test(k);
  if (/context|window|max|limit|rpm|tpm|rpd|size|length|version|temperature|top_p|top_k|dimension|seconds|timeout|retries|count|budget|threshold|weight|score|quantity/.test(k) && !hasMoney) return null;
  if (/tokens?(_|$)|(^|_)tok(_|$)/.test(k) && !hasMoney) return null;
  const batch = /batch/.test(k);
  let field = null;
  if (/cache|cached/.test(k)) {
    if (/write|creation|create|store|_5m|_1h|5min|1hour|_30m|_12h|5_m|1_h/.test(k)) field = 'cache_write';
    else if (/read|hit|cached|cache_input|cache_in\b/.test(k)) field = 'cache_read';
    else return null;
  } else if (/(^|_)(input|prompt|in|request)(_|$)|input|prompt/.test(k)) field = 'input';
  else if (/(^|_)(output|completion|out|response|generation)(_|$)|output|completion/.test(k)) field = 'output';
  else return null;
  if (batch && field === 'input') field = 'batch_input';
  if (batch && field === 'output') field = 'batch_output';
  return field;
}

/** Unit hint from a key or comment: 'M', 'K', 'T' (per token) or null. */
export function unitHint(text) {
  const t = text.toLowerCase();
  if (/per_?token|cost_per_token|token_cost|per\s+token\b|\/\s*token\b/.test(t)) return 'T';
  if (/1k|per_?1_?000(?!_?000)|per_?thousand|per\s+1,?000\s+tokens|\/\s*1k|_k_|per_k\b|kilo/.test(t)) return 'K';
  if (/1m\b|1m_|_1m|mtok|million|per_?1_?000_?000|per\s+1,?000,?000|\/\s*1m|per_m\b|_m_tok/.test(t)) return 'M';
  return null;
}

const UNIT_FACTOR = { M: 1, K: 1000, T: 1e6 };

/** Every price the provider page lists for this field, any tier. */
function acceptable(row, field) {
  const base = field.replace('batch_', '');
  const pick = (groups) => {
    const vals = [];
    const add = (v) => {
      if (typeof v === 'number') vals.push(v);
      else if (v && typeof v === 'object') for (const x of Object.values(v)) if (typeof x === 'number') vals.push(x);
    };
    for (const g of groups.filter(Boolean)) {
      add(g[field]);
      add(g[base]);
      add(g['batch_' + base]);
    }
    return vals;
  };
  const standard = pick([row, row.long_context]);
  const out = [...standard, ...pick([...(row.scheduled || []), ...Object.values(row.other_tiers || {})])];
  // Regional / data-residency endpoints are 1.1x the standard prices on Anthropic and OpenAI.
  if (row.provider === 'anthropic' || row.provider === 'openai') for (const v of standard) out.push(Math.round(v * 1.1 * 1e9) / 1e9);
  return out;
}

function primary(row, field) {
  const v = row[field];
  if (typeof v === 'number') return v;
  if (v && typeof v === 'object') {
    const first = Object.values(v).find((x) => typeof x === 'number');
    return first ?? null;
  }
  return null;
}

function close(a, b, tol) {
  return Math.abs(a - b) <= Math.max(tol * Math.abs(b), 1e-9);
}

/**
 * Compares one written number with the row.
 * Returns null when the number is not a plausible price for this field, otherwise
 * { ok, perM, unit, expected }.
 */
export function judge(row, field, value, unit, tol, soft = null) {
  if (!(value > 0) || !Number.isFinite(value)) return null;
  const expected = primary(row, field) ?? primary(row, field.replace('batch_', ''));
  if (expected === null) return null;
  const okSet = acceptable(row, field);
  const units = unit ? [unit] : ['M', 'K', 'T'];
  // Any unit reading that matches a listed price means the number is correct.
  for (const u of units) {
    const perM = value * UNIT_FACTOR[u];
    if (okSet.some((v) => close(perM, v, tol))) return { ok: true, perM, unit: u, expected };
  }
  // Otherwise read the number in the most likely unit and only call it drift when that
  // reading is within a believable distance; far-off numbers are not prices at all.
  const ratioOf = (u) => {
    const perM = value * UNIT_FACTOR[u];
    return { perM, unit: u, ratio: Math.max(perM / expected, expected / perM) };
  };
  let best = null;
  if (!unit && soft && ratioOf(soft).ratio <= 20) best = ratioOf(soft);
  if (!best) for (const u of units) {
    const r = ratioOf(u);
    if (!best || r.ratio < best.ratio) best = r;
  }
  const limit = unit ? 50 : 20;
  if (best.ratio > limit) return null;
  return { ok: false, perM: best.perM, unit: best.unit, expected };
}

/** Lines inside Rust #[cfg(test)] items. */
export function testMask(lines, ext) {
  if (ext !== 'rs') return null;
  const mask = new Array(lines.length).fill(false);
  for (let i = 0; i < lines.length; i++) {
    if (!/^\s*#\[cfg\(test\)\]/.test(lines[i])) continue;
    let depth = 0;
    let opened = false;
    let j = i;
    for (; j < lines.length; j++) {
      mask[j] = true;
      for (const ch of stripStrings(lines[j])) {
        if (ch === '{') {
          depth++;
          opened = true;
        } else if (ch === '}') depth--;
      }
      if (opened && depth <= 0) break;
      if (!opened && j > i && /;\s*$/.test(lines[j])) break;
    }
    i = j;
  }
  return mask;
}

/** Test files are skipped unless include-tests is on. */
export function isTestFile(file) {
  return /(^|\/)(tests?|__tests__|spec|specs|benches|testdata|fixtures)\//.test(file) || /(_test\.(go|py|rs)|\.(test|spec)\.[cm]?[jt]sx?|(^|\/)test_[^/]*\.py|(^|\/)conftest\.py)$/.test(file);
}

const EXT_OK = new Set(['json', 'toml', 'yaml', 'yml', 'py', 'ts', 'tsx', 'js', 'jsx', 'mjs', 'cjs', 'rs', 'go']);

export function extOf(file) {
  const m = file.match(/\.([A-Za-z0-9]+)$/);
  return m ? m[1].toLowerCase() : '';
}

export function scannable(file) {
  if (/(^|\/)(package-lock\.json|Cargo\.lock|yarn\.lock|pnpm-lock\.yaml|go\.sum|poetry\.lock)$/.test(file)) return false;
  if (/\.min\.js$/.test(file)) return false;
  return EXT_OK.has(extOf(file));
}

// One match arm or case label for several models: "a" | "b" => ..., case "a", "b":, if m in ("a", "b"):
const Q = String.raw`["'\x60][^"'\x60]+["'\x60]`;
const GROUP = [
  new RegExp(String.raw`^\s*(?:${Q}\s*\|\s*)+${Q}\s*(?:=>|:)`),
  new RegExp(String.raw`^\s*case\s+(?:${Q}\s*,\s*)*${Q}\s*:`),
  new RegExp(String.raw`\bin\s*[\[(]\s*(?:${Q}\s*,\s*)+${Q}\s*,?\s*[\])]\s*:\s*$`),
];

/**
 * Scans one file's text. Returns { findings, seen, priced } where findings are
 * { file, line, model, provider, field, written, unit, perM, expected, ok, source_url }.
 */
export function scanText(file, text, index, opts = {}) {
  const tol = opts.tolerance ?? 0.01;
  const ext = extOf(file);
  const lines = text.split(/\r?\n/);
  const findings = [];
  const seen = new Map();
  const priced = new Set();
  const skip = opts.includeTests ? null : testMask(lines, ext);
  const fileHint = unitHint(lines.slice(0, 40).filter((l) => /\/\/|#|\/\*|\*|"""|'''/.test(l)).join('\n'));
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (line.length > 2000 || (skip && skip[i])) continue;
    const hits = modelsOnLine(index, line);
    if (!hits.length) continue;
    for (const h of hits) if (h.quoted && !seen.has(h.row.model)) seen.set(h.row.model, { file, line: i + 1, row: h.row });
    const rows = new Set(hits.map((h) => h.row));
    // Several models on one line only share a price block when the line is one match arm or
    // case label for all of them ("a" | "b" => ..., case "a", "b":). Any other list is skipped.
    const group = rows.size > 1 && hits.every((h) => h.quoted) && GROUP.some((re) => re.test(line));
    if (rows.size !== 1 && !group) continue;
    const targets = [...rows];
    const hit = hits[hits.length - 1];
    const [a, b] = blockFor(lines, i, hit, ext);
    // Stop the block at the next line that names a different model.
    let end = b;
    for (let j = i + 1; j <= b; j++) {
      const other = modelsOnLine(index, lines[j]).filter((x) => !targets.includes(x.row));
      if (other.length) {
        end = j - 1;
        break;
      }
    }
    let start = a;
    for (let j = i - 1; j >= a; j--) {
      const other = modelsOnLine(index, lines[j]).filter((x) => !targets.includes(x.row));
      if (other.length) {
        start = j + 1;
        break;
      }
    }
    const context = lines.slice(Math.max(0, start - 3), end + 1).join('\n');
    const blockHint = unitHint(context.replace(/"[^"\n]*"/g, (s) => (/(per|1k|1m|mtok|million|thousand)/i.test(s) ? s : '""')));
    const found = [];
    for (let j = start; j <= end; j++) {
      // Drop the model token itself so ids like gpt-4.1 never look like numbers.
      let L = lines[j];
      for (const h of modelsOnLine(index, L)) L = L.slice(0, h.col) + ' '.repeat(h.token.length) + L.slice(h.col + h.token.length);
      KV.lastIndex = 0;
      let m;
      let keyed = 0;
      while ((m = KV.exec(L))) {
        const field = classifyKey(m[1]);
        if (!field) continue;
        keyed++;
        const unit = divUnit(m[3]) || unitHint(m[1]);
        found.push({ line: j + 1, field, raw: m[2], value: parseNum(m[2]), unit, soft: blockHint || fileHint });
      }
      // Positional tuple on the model's own line: (input, output).
      KV.lastIndex = 0;
      const anyKey = keyed > 0 || KV.test(L);
      KV.lastIndex = 0;
      const pm = j === i && !anyKey ? L.slice(hit.col + hit.token.length).match(POS) : null;
      if (pm) {
        const nums = [
          { raw: pm[1], value: parseNum(pm[1]), unit: divUnit(pm[2]), soft: blockHint || fileHint },
          { raw: pm[3], value: parseNum(pm[3]), unit: divUnit(pm[4]), soft: blockHint || fileHint },
        ];
        {
          const plausible = targets.some((t) => judge(t, 'input', nums[0].value, nums[0].unit, tol, nums[0].soft) && judge(t, 'output', nums[1].value, nums[1].unit, tol, nums[1].soft));
          if (plausible) {
            found.push({ line: j + 1, field: 'input', ...nums[0] });
            found.push({ line: j + 1, field: 'output', ...nums[1] });
          }
        }
      }
    }
    for (const f of found) for (const row of targets) {
      const v = judge(row, f.field, f.value, f.unit, tol, f.soft);
      if (!v) continue;
      priced.add(row.model);
      const key = `${f.line}:${f.field}:${f.raw}`;
      if (findings.some((x) => x.key === key && x.model === row.model)) continue;
      findings.push({
        key,
        file,
        line: f.line,
        model: row.model,
        provider: row.provider,
        field: f.field,
        written: f.raw,
        unit: v.unit,
        perM: v.perM,
        expected: v.expected,
        ok: v.ok,
        source_url: row.source_url,
        checked: row.checked,
      });
    }
  }
  return { findings, seen, priced };
}

/**
 * Missing prices: a model of a provider the repo already prices, used somewhere in quotes,
 * with no price found for it anywhere in the scanned files.
 */
export function missingPrices(allSeen, allPriced, pricedProviders) {
  const out = [];
  for (const [model, where] of allSeen) {
    if (allPriced.has(model)) continue;
    if (!pricedProviders.has(where.row.provider)) continue;
    if (where.row.status === 'retired') continue;
    out.push({ file: where.file, line: where.line, model, provider: where.row.provider, source_url: where.row.source_url });
  }
  return out;
}
