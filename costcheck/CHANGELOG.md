# Changelog

## 0.1.0 (2026-11-19)

First release.

- `drift`: hard-coded model prices that differ from the provider's official page, in Python, TypeScript, JavaScript, Rust, Go, JSON, YAML and TOML, in per-token, per-1K and per-1M units. The matching rules are a port of the llm-prices GitHub Action scanner, with its fixtures.
- `fallback`: default prices for models a table does not list (catch-all match arms and switch defaults, named defaults, `"default"` keys, inline defaults at the lookup), including a default of $0.
- `cache`: models priced without a cache-read price when the provider bills cache reads at a fraction of input.
- `missing`: models used in quotes with no price anywhere (reported; fails only with `--fail-on missing`).
- Every issue carries the error in percent and in dollars on a fixed reference call, with file:line, the source page and the date it was checked.
- Why the defaults fail closed: a default price, a missing cache price and a stale price all produce a number that looks fine on a dashboard. The only safe default for a cost check is to fail the build until the number is true, so `drift,fallback,cache` all fail by default and `--fail-on` narrows it.
- Output as text, JSON, or GitHub annotations plus a job summary; a composite GitHub Action in `costcheck/action.yml` that fetches the newest `prices.json` on every run.
