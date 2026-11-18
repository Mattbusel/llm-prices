# Contributing to llm-prices

Thanks for helping. Issues and pull requests are welcome at
https://github.com/Mattbusel/llm-prices.

## Wrong or missing prices

Open an issue with the link to the provider's official pricing page. A price only changes when
that page shows it; third-party aggregators are not a source. The daily refresh job usually
catches changes on its own and opens a pull request.

## Code changes

1. Fork the repository and branch from `main`.
2. `prices.json` is the single source of truth. After editing it, run
   `node scripts/build.mjs` to regenerate the Rust, Go, Python, npm and root module data, and
   `node scripts/build.mjs --check` to confirm they are in sync.
3. Run the tests for what you touched:
   - Action and root module: `node --test test/scan.test.mjs test/data.test.mjs test/cli.test.mjs test/root.test.mjs`
   - Rust: `cd crate && cargo test && cargo clippy --all-targets -- -D warnings`
   - Go: `cd go && go vet ./... && go test ./...`
   - Python: `cd python && python -m pytest && python -m mypy --strict src/llm_prices`
   - npm package: `cd js && npm ci && npm test`
4. Open a pull request against `main`. CI must be green before it is merged.

Every pull request that adds functionality must add tests for it, and every bug fix must add a
test that fails without the fix.

## Security

Security problems go through private reporting, not issues: see [SECURITY.md](SECURITY.md).

## License

By contributing you agree that your contributions are licensed under the MIT license of this
repository.
