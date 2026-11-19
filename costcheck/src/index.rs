//! Maps every id form a source file might use to its row in the table.

use std::collections::{HashMap, HashSet};

use regex::Regex;

use crate::prices::{Row, Table};

macro_rules! re {
    ($s:expr) => {{
        static R: ::std::sync::OnceLock<::regex::Regex> = ::std::sync::OnceLock::new();
        R.get_or_init(|| ::regex::Regex::new($s).expect("valid regex"))
    }};
}
pub(crate) use re;

/// Providers whose rows the daily refresh re-reads from the official pages. The other rows
/// were checked by hand, so they are only used when named.
pub const DEFAULT_PROVIDERS: [&str; 4] = ["anthropic", "openai", "google", "deepseek"];

fn date8() -> &'static Regex {
    re!(r"-(20\d{6})$")
}

/// A path under a host or cloud (`azure/gpt-4o`, `vertex_ai/gemini-2.5-pro`,
/// `bedrock/...`): that host's price, which is not the model maker's list price.
fn is_host_path(t: &str) -> bool {
    match t.split_once('/') {
        Some((first, _)) => !matches!(
            first,
            "models"
                | "anthropic"
                | "openai"
                | "google"
                | "gemini"
                | "deepseek"
                | "litellm"
                | "deepseek-ai"
        ),
        None => false,
    }
}

/// Lookup from model ids to rows of one table.
pub struct Index<'a> {
    pub table: &'a Table,
    exact: HashMap<String, usize>,
    undated: HashMap<String, usize>,
    foreign: HashSet<String>,
    stems: Regex,
}

impl<'a> Index<'a> {
    /// An index over the rows of `providers` (empty: the default four; `all`: every provider).
    pub fn new(table: &'a Table, providers: &[String]) -> Index<'a> {
        let all = providers.iter().any(|p| p == "all");
        let want: HashSet<String> = if all {
            table.rows.iter().map(|r| r.provider.clone()).collect()
        } else if providers.is_empty() {
            DEFAULT_PROVIDERS.iter().map(|s| s.to_string()).collect()
        } else {
            providers.iter().map(|s| s.to_lowercase()).collect()
        };
        let mut exact: HashMap<String, usize> = HashMap::new();
        let mut undated = HashMap::new();
        // Ids two chosen providers both sell say nothing about which price is meant.
        let mut ambiguous = HashSet::new();
        // Ids of rows from providers not chosen: a file naming one is not quoting the maker.
        let mut foreign = HashSet::new();
        for (i, row) in table.rows.iter().enumerate() {
            let ids = std::iter::once(&row.model).chain(row.aliases.iter());
            if !want.contains(&row.provider) {
                for id in ids {
                    foreign.insert(id.to_lowercase());
                }
                continue;
            }
            for id in ids {
                let key = id.to_lowercase();
                let clash = exact
                    .get(&key)
                    .is_some_and(|&p: &usize| table.rows[p].provider != row.provider);
                if ambiguous.contains(&key) || clash {
                    ambiguous.insert(key.clone());
                    exact.remove(&key);
                    continue;
                }
                exact.insert(key.clone(), i);
                // Anthropic publishes dated ids for older models; people write the undated form.
                if row.provider == "anthropic" && date8().is_match(&key) {
                    undated.insert(date8().replace(&key, "").into_owned(), i);
                }
            }
        }
        for key in exact.keys() {
            foreign.remove(key);
        }
        // A token can only resolve when it contains the leading word of some id; checking that
        // first keeps the scan fast on large repos.
        let mut stems: Vec<String> = exact
            .keys()
            .chain(undated.keys())
            .map(|k| {
                let end = k.find(['-', '.', '@', ':', '/', '_']).unwrap_or(k.len());
                regex::escape(&k[..end.max(1)])
            })
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        stems.sort();
        let stems = if stems.is_empty() {
            Regex::new("$^").expect("valid regex")
        } else {
            Regex::new(&format!("(?i){}", stems.join("|"))).expect("valid regex")
        };
        Index {
            table,
            exact,
            undated,
            foreign,
            stems,
        }
    }

    pub fn row(&self, i: usize) -> &'a Row {
        &self.table.rows[i]
    }

    /// True when a token names a row of a provider that was not chosen: a host's or
    /// reseller's own price for a model, not the model maker's.
    pub fn is_foreign(&self, token: &str) -> bool {
        let lower = token.to_lowercase();
        let t = lower.trim_end_matches(['.', ':', '/', '-']);
        if is_host_path(t) && self.stems.is_match(t) {
            return true;
        }
        self.foreign.contains(t)
            || self
                .foreign
                .contains(t.strip_prefix("openrouter/").unwrap_or(t))
    }

    /// The row a raw token from a file names, or `None`.
    pub fn resolve(&self, token: &str) -> Option<usize> {
        if !self.stems.is_match(token) {
            return None;
        }
        let lower = token.to_lowercase();
        let t = lower.trim_end_matches(['.', ':', '/', '-']);
        if let Some(&i) = self.exact.get(t) {
            return Some(i);
        }
        if is_host_path(t) {
            return None;
        }
        if self
            .foreign
            .contains(t.strip_prefix("openrouter/").unwrap_or(t))
        {
            return None;
        }
        let t = re!(r"^(?:(?:models|anthropic|openai|google|gemini|vertex_ai|vertex|bedrock|azure|openrouter|deepseek|litellm)/)+")
            .replace(t, "");
        let t = re!(r"^(?:(?:us|eu|apac|au|jp|ca|global)\.)?anthropic\.").replace(&t, "");
        let t = re!(r"-v\d+(?::\d+)?$").replace(&t, "");
        let t = re!(r"@(20\d{6})$").replace(&t, "-$1").into_owned();
        if let Some(&i) = self.exact.get(&t) {
            return Some(i);
        }
        if let Some(&i) = self.undated.get(&t) {
            return Some(i);
        }
        let und = date8().replace(&t, "");
        if und != t {
            let r = self
                .exact
                .get(und.as_ref())
                .or_else(|| self.undated.get(und.as_ref()));
            if let Some(&i) = r {
                if self.table.rows[i].provider == "anthropic" {
                    return Some(i);
                }
            }
        }
        None
    }
}
