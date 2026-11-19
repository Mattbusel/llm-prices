//! Your LLM cost tracking is wrong, and nothing tells you.
//!
//! `llm-costcheck` reads a codebase the way a reviewer would and finds the three ways cost
//! tracking goes wrong:
//!
//! 1. **Drift**: a hard-coded model price that no longer matches the provider's page.
//! 2. **Fallback**: a default price silently used for any model the table does not know
//!    (including a default of $0, which books every new model as free).
//! 3. **Missing cache pricing**: a model priced without its cache-read price, so cached
//!    prompt tokens are billed at the full input price or not at all.
//!
//! Every price is checked against [`llm-prices`](https://crates.io/crates/llm-prices), dated
//! and cited to the official pages, and every issue carries the error in percent and in
//! dollars on a reference call, with the file and line.
//!
//! ```
//! use llm_costcheck::{check, Index, Opts, Table, Kind};
//!
//! let table = Table::embedded();
//! let index = Index::new(&table, &[]);
//! let src = r#"PRICES = {"claude-opus-5-5": {"input": 15.0, "output": 75.0}}"#;
//! let report = check([("pricing.py".to_string(), src.to_string())], &index, Opts::default());
//! let issue = &report.issues[0];
//! assert_eq!(issue.kind, Kind::Drift);
//! assert!(issue.headline().contains("wrong by +275.0% for claude-opus-5-5"));
//! ```

#![allow(clippy::needless_range_loop)]

pub mod check;
pub mod fallback;
pub mod files;
pub mod index;
pub mod prices;
pub mod scan;

pub use check::{check, Issue, Kind, Report};
pub use fallback::Fallback;
pub use index::{Index, DEFAULT_PROVIDERS};
pub use prices::{Field, Row, Table};
pub use scan::{Finding, Opts, Unit};
