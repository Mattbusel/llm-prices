# Governance

## Who decides

llm-prices has one maintainer: Matthew Busel ([@Mattbusel](https://github.com/Mattbusel)),
who works on it through Tensorust LLC. The maintainer reviews and merges pull requests,
decides what goes into a release, and publishes releases. There is no committee and no
voting; decisions are made in the open on issues and pull requests, with the reasoning
written down there.

Anyone can propose a change by opening an issue or a pull request. See
[CONTRIBUTING.md](CONTRIBUTING.md). CI (the Node, Rust, Go and Python test suites, clippy with
warnings as errors, `go vet`, `mypy --strict`, fuzzing of the parsers, and the check that every
package's data matches `prices.json`) must be green before a change is merged.

## How prices change

A price only changes when the provider's official pricing page shows the new number. The
daily [refresh workflow](.github/workflows/refresh.yml) re-reads those pages and opens a pull
request when a number differs; the maintainer reviews it against the page before merging. When
a page cannot be read, the workflow opens an issue and keeps the last good rows for that
provider.

## Releases

The repository follows [Semantic Versioning](https://semver.org/). Within 1.x there are no
breaking changes to the table format or the package APIs.

1. Update `prices.json`, regenerate every package's data (`node scripts/build.mjs`), bump the
   versions, and add a dated entry to [CHANGELOG.md](CHANGELOG.md) (and the package changelogs)
   listing every user-visible change and any fixed vulnerability.
2. Merge to `main` with CI green.
3. Tag and publish:
   - `vX.Y.Z` with a GitHub release whose notes come from the changelog entry; the `v1` tag
     moves to it. [release-assets.yml](.github/workflows/release-assets.yml) attaches
     `prices.json`, its SHA-256 and its Ed25519 signature.
   - `py-vX.Y.Z` runs [publish-python.yml](.github/workflows/publish-python.yml) and
     `otel-py-vX.Y.Z` runs [publish-otel-python.yml](.github/workflows/publish-otel-python.yml):
     each checks that the tag matches the package version, runs the tests, builds, and uploads
     to PyPI with trusted publishing (OIDC, no stored token) and PEP 740 provenance
     attestations, from the dedicated `pypi` environment.
   - `js-vX.Y.Z` runs [publish-js.yml](.github/workflows/publish-js.yml) (npm, with provenance).
   - `go/vX.Y.Z` tags the Go module. The Rust crate is published to
     [crates.io](https://crates.io/crates/llm-prices) from the tagged commit with
     `cargo publish`.

Security fixes follow [SECURITY.md](SECURITY.md) and ship as a patch release.

## Continuity

The project has a single maintainer, so the plan for the maintainer becoming unavailable is
written down here:

- Everything needed to rebuild the table and release is in this repository: the parsers,
  the source URL of every row, the CI workflows and the release steps above. PyPI and npm
  releases come from workflows, not from a personal machine.
- The data and code are MIT licensed, so anyone can fork and continue them without asking.
  Because every row cites its source page and check date, a fork can verify the table
  independently.
- When a regular contributor emerges, the maintainer will offer them commit access and
  co-owner rights on the package registries, so that releases do not depend on one person.
- If the maintainer decides to step back, the repository and the package registry projects
  will be handed to a willing maintainer and the change announced in the README and in a
  pinned issue. If nobody takes it over, the daily refresh will be switched off and the
  repository archived with a note saying the prices are no longer kept current, so nobody
  relies on a stale table without knowing it.

## Changes to this document

Changes to governance are made by pull request like any other change.
