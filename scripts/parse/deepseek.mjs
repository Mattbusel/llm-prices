import { SOURCES } from '../meta.mjs';
import { ParseError, row } from './common.mjs';

export function htmlToText(html) {
  let t = html.replace(/<script[\s\S]*?<\/script>/g, '').replace(/<style[\s\S]*?<\/style>/g, '');
  t = t.replace(/<\/(tr|h[1-6]|p|li)>/g, '\n').replace(/<\/t[dh]>/g, ' | ').replace(/<[^>]+>/g, '');
  t = t.replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&#x27;|&#39;/g, "'").replace(/&quot;/g, '"').replace(/&nbsp;|&#160;/g, ' ');
  return t.replace(/[ \t]+/g, ' ').replace(/\n\s*\n+/g, '\n');
}

function pair(text, label) {
  // "<label> | OFF-PEAK | $a | $b |\nPEAK | $c | $d |"
  const re = new RegExp(`${label}\\s*\\|\\s*OFF-PEAK\\s*\\|\\s*\\$([0-9.]+)\\s*\\|\\s*\\$([0-9.]+)\\s*\\|\\s*\\n\\s*PEAK\\s*\\|\\s*\\$([0-9.]+)\\s*\\|\\s*\\$([0-9.]+)`);
  const m = text.match(re);
  if (!m) throw new ParseError(`deepseek: "${label}" rows not found`);
  return { off: [Number(m[1]), Number(m[2])], peak: [Number(m[3]), Number(m[4])] };
}

export function parseDeepSeek(html, checked) {
  const src = SOURCES.deepseek.page;
  const text = htmlToText(html);
  const mm = text.match(/MODEL\s*\|\s*([a-z0-9.\-]+)(?:\(\d\))?\s*\|\s*([a-z0-9.\-]+)(?:\(\d\))?\s*\|/);
  if (!mm) throw new ParseError('deepseek: model header row not found');
  const ids = [mm[1], mm[2]];
  const hit = pair(text, '1M INPUT TOKENS\\(CACHE HIT\\)');
  const miss = pair(text, '1M INPUT TOKENS\\(CACHE MISS\\)');
  const out = pair(text, '1M OUTPUT TOKENS');
  const legacy = text.match(/legacy names ([a-z0-9\-]+) and ([a-z0-9\-]+) are still accepted/);
  const peakHours = text.match(/Peak hours are ([^.]+)\./);
  const rows = ids.map((id, i) =>
    row({
      provider: 'deepseek',
      model: id,
      aliases: i === 0 && legacy ? [legacy[1], legacy[2]] : [],
      input: miss.peak[i],
      output: out.peak[i],
      cache_write: null,
      cache_read: hit.peak[i],
      other_tiers: { offpeak: { input: miss.off[i], output: out.off[i], cache_read: hit.off[i] } },
      source_url: src,
      checked,
      note:
        `input/output/cache_read are peak-hour prices; other_tiers.offpeak is the off-peak price` +
        (peakHours ? ` (peak hours: ${peakHours[1].trim()})` : '') +
        (i === 0 && legacy ? `; ${legacy[1]} and ${legacy[2]} are retired names now billed at the flash price` : ''),
    }),
  );
  return { rows, unknown: [] };
}
