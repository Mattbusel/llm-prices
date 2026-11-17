// Cuts each provider page down to the parts the parsers read, for test/pages.

import { GEMINI_TEXT_MODELS } from './meta.mjs';

const note = (url, date) => `<!-- Excerpt of ${url} fetched ${date}, kept only for parser tests. -->\n`;

function between(text, start, end) {
  const i = text.indexOf(start);
  const j = end ? text.indexOf(end, i + 1) : text.length;
  if (i < 0 || j < 0) throw new Error(`excerpt: "${start}" not found`);
  return text.slice(i, j);
}

export function excerpt(provider, text, date) {
  if (provider === 'anthropic') {
    return note('https://platform.claude.com/docs/en/about-claude/pricing', date) + between(text, '## Model pricing', '## Cloud platform pricing') + between(text, '### Data residency pricing', '### Tool use pricing');
  }
  if (provider === 'openai') {
    // The Flagship and Specialized sections, each up to the next top-level label.
    const LABELS = ['Flagship models', 'Multimodal models', 'Cyber models', 'Life sciences models', 'Specialized models', 'Images', 'Audio', 'GPT-Live', 'Transcription', 'Tools', 'Cloud platforms', 'Finetuning', 'Fine-tuning'];
    const at = (label, from = 0) => {
      const re = new RegExp(`^\\s*${label}\\s*$`, 'gm');
      re.lastIndex = from;
      const m = re.exec(text);
      return m ? m.index : -1;
    };
    const section = (label) => {
      const a = at(label);
      if (a < 0) throw new Error(`excerpt: "${label}" not found`);
      const ends = LABELS.filter((l) => l !== label).map((l) => at(l, a + label.length)).filter((i) => i > a);
      return text.slice(a, ends.length ? Math.min(...ends) : text.length);
    };
    return note('https://developers.openai.com/api/docs/pricing', date) + section('Flagship models') + '\nMultimodal models\n\n' + section('Specialized models') + '\nTools\n';
  }
  if (provider === 'google') {
    const kept = text
      .split(/\n## /)
      .slice(1)
      .filter((p) => GEMINI_TEXT_MODELS.some((k) => p.split('###')[0].includes('[`' + k + '`]')));
    return note('https://ai.google.dev/gemini-api/docs/pricing', date) + '\n## ' + kept.join('\n## ');
  }
  if (provider === 'deepseek') {
    // From the pricing table to the next section heading (keeps the footnotes under the table).
    const t0 = text.indexOf('<table');
    const n1 = text.search(/Search (&amp;|&) Fetch Pricing/);
    if (t0 < 0 || n1 < t0) throw new Error('excerpt: deepseek table not found');
    return note('https://api-docs.deepseek.com/quick_start/pricing', date) + '<html><body>' + text.slice(t0, text.lastIndexOf('<', n1)) + '</body></html>\n';
  }
  throw new Error(`excerpt: unknown provider ${provider}`);
}
