# llm-prices for JavaScript and TypeScript

The repository root is an ES module package with types, no dependencies and no network
access. Install it straight from GitHub (no npm registry account needed):

```sh
npm i github:Mattbusel/llm-prices#v1        # moves with every 1.x release
npm i github:Mattbusel/llm-prices#v1.3.0    # pinned
```

```js
import { lookup, lookupProvider } from 'llm-prices';

lookup('claude-sonnet-4-6').input;                         // 3 (USD per 1M tokens)
lookupProvider('groq', 'openai/gpt-oss-120b').provider;    // 'groq'
lookupProvider('deepseek', 'deepseek-chat').model;         // 'deepseek-flash'
lookup('gpt-4o').costUsage({ input: 12000, output: 800 }).total;
```

From a CDN, in a browser, Deno or a worker:

```js
import { lookup } from 'https://cdn.jsdelivr.net/gh/Mattbusel/llm-prices@v1/index.mjs';
import { lookupProvider } from 'https://esm.sh/gh/Mattbusel/llm-prices@v1';
```

The raw table is also served as JSON: `https://cdn.jsdelivr.net/gh/Mattbusel/llm-prices@v1/prices.json`
(and `import prices from 'llm-prices/prices.json' with { type: 'json' }` after installing).

## What the package contains

| File | What it is |
|---|---|
| `index.mjs`, `index.d.ts` | The API, built from [`js/src/index.ts`](../js/src/index.ts) by `scripts/rootjs.mjs` |
| `data.mjs`, `data.d.mts` | The rows, generated from `prices.json` by `scripts/build.mjs` (the daily refresh updates them) |
| `prices.json` | The source table |

The API is the same as the `js/` package: `lookup`, `lookupProvider`, `parseProvider`,
`providerRows`, `fallbackProviders`, `all`, `Price` (`cost`, `costUsage`, `costUsageExact`,
`rate`, `rateStr`), `Provider`, `PROVIDERS`, `GENERATED`. Provider names accept the LangChain,
LiteLLM and SDK spellings listed in [python.md](python.md#pricing-a-request-when-you-know-the-provider-lookup_provider).

CommonJS: `require('llm-prices')` works on Node 20.19+ and 22.12+, which load ES modules from
`require`. On older Node use `await import('llm-prices')`.

## Keeping hard-coded prices honest instead

If your code keeps its own price table, the GitHub Action in this repository checks it
against the provider pages on every pull request:

```yaml
- uses: Mattbusel/llm-prices@v1
```

It reads JSON, TOML, YAML, Python, TypeScript/JavaScript, Rust and Go. Ratio keys such as
`cacheWriteMultiplier` are ignored, and prices inside a record with a `threshold` key
(`{ threshold: 200000, prompt: 2.5 }`) are compared with the long-context prices.
