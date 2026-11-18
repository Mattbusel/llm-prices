# Security policy

## Supported versions

The latest 1.x release gets fixes: the GitHub Action (`Mattbusel/llm-prices@v1`, the `v1` tag
moves to every 1.x release), the Rust crate, the Python packages, the npm package
`@mattbusel/llm-prices`, the Go module and the `genaicost` Collector processor.

| version | supported |
|---|---|
| latest 1.x | yes |
| older 1.x | no; upgrade to the latest 1.x (no breaking changes within 1.x) |

## Reporting a vulnerability

Please do not open a public issue for a security problem. Use GitHub's private
vulnerability reporting on this repository: the Security tab, then "Report a
vulnerability" (https://github.com/Mattbusel/llm-prices/security/advisories/new).
The report stays private between you and the maintainer until an advisory is published.

What happens next:

- You get a first reply within 14 days, usually much sooner.
- If the problem is confirmed, the fix ships as a patch release of every affected package,
  and a GitHub security advisory (with a CVE requested through GitHub when the issue
  warrants one) is published at the same time. The advisory credits you unless you ask not
  to be named.
- The target is a released fix within 60 days of the report. If that is not possible, the
  advisory is coordinated with you before anything is made public.

A wrong price is a data bug, not a security problem: open a normal issue with the provider's
page link for that.

## Scope

In scope, for example:

- The GitHub Action running code, reading files outside the scanned workspace, or leaking
  secrets from the workflow it runs in.
- A way to get a forged `prices.json` accepted by the `genaicost` Collector processor, which
  only loads a table whose Ed25519 signature (`prices.json.sig`, made in
  [release-assets.yml](.github/workflows/release-assets.yml)) verifies against the public key
  in `otel/genaicostprocessor/config.go`.
- A panic, hang or unbounded memory use in any package reachable from untrusted input
  (model ids, usage numbers, a LiteLLM-format or prices.json file).

The core libraries (the Rust crate, the `llm-prices` Python package, the npm package and the
Go module) make no network calls. The OpenTelemetry packages can load a price table from a
location you configure; the Collector processor's reload of the release table is
signature-checked. The `llm-prices` Python package is published from GitHub Actions
with PyPI trusted publishing and carries PEP 740 provenance attestations; the npm package is
published with npm provenance.
