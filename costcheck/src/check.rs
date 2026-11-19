//! Repo-level checks: turns file findings into issues with the error in percent and dollars.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::fallback::{find_fallbacks, Fallback};
use crate::index::{re, Index};
use crate::prices::{call, call_cost, Field, Row};
use crate::scan::{scan_text, Finding, Opts};

/// What kind of wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    /// A hard-coded price differs from the provider's page.
    Drift,
    /// Unknown models are billed at a default price.
    Fallback,
    /// A model is priced without its cache-read price.
    Cache,
    /// A model is used but priced nowhere.
    Missing,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Drift => "drift",
            Kind::Fallback => "fallback",
            Kind::Cache => "cache",
            Kind::Missing => "missing",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        Some(match s {
            "drift" => Kind::Drift,
            "fallback" => Kind::Fallback,
            "cache" => Kind::Cache,
            "missing" => Kind::Missing,
            _ => return None,
        })
    }
}

/// One thing that makes the cost tracking wrong.
#[derive(Debug, Clone, PartialEq)]
pub struct Issue {
    pub kind: Kind,
    pub file: String,
    pub line: usize,
    pub model: String,
    pub provider: String,
    /// Booked cost minus real cost, as a fraction of the real cost (0.25 = 25% too high).
    pub error: Option<f64>,
    /// The reference call the dollar figures are for.
    pub call: &'static str,
    /// USD the tracker books for the reference call.
    pub booked: Option<f64>,
    /// USD the provider charges for the reference call.
    pub real: Option<f64>,
    /// One-line explanation with the numbers.
    pub detail: String,
    pub source_url: String,
    pub checked: String,
    /// For fallbacks: how many listed models the table does not price.
    pub affected: usize,
}

impl Issue {
    /// The headline: "your cost tracking is wrong by X% for model Y".
    pub fn headline(&self) -> String {
        match self.error {
            Some(e) => format!(
                "your cost tracking is wrong by {} for {}",
                pct(e),
                self.model
            ),
            None => format!("your cost tracking has no price for {}", self.model),
        }
    }
}

/// A signed percentage: `+25.0%`, `-80.0%`.
pub fn pct(e: f64) -> String {
    let p = e * 100.0;
    if p.abs() >= 1000.0 {
        format!("{p:+.0}%")
    } else {
        format!("{p:+.1}%")
    }
}

/// USD with enough digits for small calls.
pub fn usd(v: f64) -> String {
    if v.abs() >= 1.0 {
        format!("${v:.2}")
    } else if v.abs() >= 0.01 {
        format!("${v:.3}")
    } else {
        format!("${v:.5}")
    }
}

fn num(v: f64) -> String {
    let s = format!("{:.6}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

pub const STANDARD_CALL: &str = "100K-token prompt, 5K-token output";
pub const CACHED_CALL: &str = "100K-token prompt with 60K served from cache, 5K-token output";
pub const CACHE_WRITE_CALL: &str = "100K-token prompt with 60K written to cache, 5K-token output";
pub const BATCH_CALL: &str = "batch call, 100K-token prompt, 5K-token output";

/// The whole result of a check.
#[derive(Debug, Default)]
pub struct Report {
    pub files_scanned: usize,
    pub findings: Vec<Finding>,
    pub fallbacks: Vec<Fallback>,
    pub issues: Vec<Issue>,
    /// Rows with a price found.
    pub priced: HashSet<usize>,
    /// True when some file prices cache reads as a share of the input price.
    pub cache_handled_globally: bool,
}

impl Report {
    pub fn count(&self, k: Kind) -> usize {
        self.issues.iter().filter(|i| i.kind == k).count()
    }
}

/// Models a fallback example is drawn from first, when the table does not price them.
const FLAGSHIPS: [&str; 8] = [
    "claude-opus-5-5",
    "claude-sonnet-5-5",
    "gpt-6.2-sol",
    "gpt-5.5",
    "gemini-3.1-pro-preview",
    "gemini-2.5-pro",
    "deepseek-v4-pro",
    "claude-haiku-5-5",
];

fn std_cost(input: f64, output: f64) -> f64 {
    call_cost(call::PROMPT, input, 0, 0.0, call::OUTPUT, output)
}

/// Cache multipliers or cache prices derived from the input price somewhere in the repo.
fn mentions_cache_ratio(text: &str) -> bool {
    re!(r"(?i)cache\w*\s*(?:_|\w)*(?:multiplier|ratio|factor|discount)\w*\s*(?::|=>|=)\s*\d")
        .is_match(text)
        || re!(r"(?i)cache\w*\s*(?::|=>|=)\s*[^,\n;]*\*\s*0?\.\d+").is_match(text)
        || re!(r"(?i)\*\s*0?\.\d+[^\n]{0,40}cache").is_match(text)
}

/// Checks a set of files. `files` yields (relative path, text).
pub fn check<I>(files: I, index: &Index, opts: Opts) -> Report
where
    I: IntoIterator<Item = (String, String)>,
{
    let mut rep = Report::default();
    let mut seen: Vec<(usize, String, usize)> = Vec::new();
    let mut seen_rows = HashSet::new();
    let mut cache_mentioned = HashSet::new();
    for (file, text) in files {
        rep.files_scanned += 1;
        let r = scan_text(&file, &text, index, opts);
        let fb = find_fallbacks(&file, &text, index, &r.findings, opts.include_tests);
        if !rep.cache_handled_globally && mentions_cache_ratio(&text) {
            rep.cache_handled_globally = true;
        }
        for (row, line) in r.seen {
            if seen_rows.insert(row) {
                seen.push((row, file.clone(), line));
            }
        }
        rep.priced.extend(r.priced);
        cache_mentioned.extend(r.cache_mentioned);
        rep.findings.extend(r.findings);
        rep.fallbacks.extend(fb);
    }
    let rows = |i: usize| -> &Row { index.row(i) };
    let mut issues = Vec::new();

    // Drift, one issue per price block of a model (its findings in one file, no more than
    // 20 lines apart): the error on the call the wrong field affects.
    let mut by_model: BTreeMap<(String, usize), Vec<&Finding>> = BTreeMap::new();
    for f in &rep.findings {
        by_model.entry((f.file.clone(), f.row)).or_default().push(f);
    }
    let mut groups: Vec<((String, usize), Vec<&Finding>)> = Vec::new();
    for ((file, r), mut fs) in by_model {
        fs.sort_by_key(|f| f.line);
        let mut cur: Vec<&Finding> = Vec::new();
        for f in fs {
            if cur
                .last()
                .is_some_and(|l| f.line > l.line + crate::scan::MAX_BLOCK)
            {
                groups.push(((file.clone(), r), std::mem::take(&mut cur)));
            }
            cur.push(f);
        }
        groups.push(((file, r), cur));
    }
    for ((file, r), fs) in &groups {
        let bad: Vec<&&Finding> = fs.iter().filter(|f| !f.ok).collect();
        if bad.is_empty() {
            continue;
        }
        let row = rows(*r);
        let written = |field: Field| -> Option<f64> {
            bad.iter()
                .find(|f| f.field == field && !f.tier)
                .map(|f| f.per_m)
                .or_else(|| {
                    fs.iter()
                        .find(|f| f.field == field && !f.tier)
                        .map(|f| f.per_m)
                })
        };
        let real_in = row.std.input.unwrap_or(0.0);
        let real_out = row.std.output.unwrap_or(0.0);
        let real_cr = row.std.cache_read.unwrap_or(real_in);
        let real_cw = row.std.cache_write.first().copied().unwrap_or(real_in);
        let bi = written(Field::Input).unwrap_or(real_in);
        let bo = written(Field::Output).unwrap_or(real_out);
        let first_bad = bad.iter().min_by_key(|f| f.line).expect("non-empty");
        let wrong: HashSet<Field> = bad.iter().filter(|f| !f.tier).map(|f| f.field).collect();
        let (call_name, booked, real) =
            if wrong.contains(&Field::Input) || wrong.contains(&Field::Output) {
                (STANDARD_CALL, std_cost(bi, bo), std_cost(real_in, real_out))
            } else if wrong.contains(&Field::CacheRead) {
                let bcr = written(Field::CacheRead).unwrap_or(real_cr);
                let fresh = call::PROMPT - call::CACHED;
                (
                    CACHED_CALL,
                    call_cost(fresh, bi, call::CACHED, bcr, call::OUTPUT, bo),
                    call_cost(
                        fresh,
                        real_in,
                        call::CACHED,
                        real_cr,
                        call::OUTPUT,
                        real_out,
                    ),
                )
            } else if wrong.contains(&Field::CacheWrite) {
                let bcw = written(Field::CacheWrite).unwrap_or(real_cw);
                let fresh = call::PROMPT - call::CACHED;
                (
                    CACHE_WRITE_CALL,
                    call_cost(fresh, bi, call::CACHED, bcw, call::OUTPUT, bo),
                    call_cost(
                        fresh,
                        real_in,
                        call::CACHED,
                        real_cw,
                        call::OUTPUT,
                        real_out,
                    ),
                )
            } else if wrong.contains(&Field::BatchInput) || wrong.contains(&Field::BatchOutput) {
                let rbi = row.std.batch_input.unwrap_or(real_in);
                let rbo = row.std.batch_output.unwrap_or(real_out);
                let wbi = written(Field::BatchInput).unwrap_or(rbi);
                let wbo = written(Field::BatchOutput).unwrap_or(rbo);
                (BATCH_CALL, std_cost(wbi, wbo), std_cost(rbi, rbo))
            } else {
                // Only long-context prices are wrong.
                let f = first_bad;
                ("long-context price", f.per_m, f.expected)
            };
        let parts: Vec<String> = bad
            .iter()
            .map(|f| {
                let tier = if f.tier { " (long-context)" } else { "" };
                let conv = if f.unit == crate::scan::Unit::M {
                    String::new()
                } else {
                    format!(" (= {}/1M)", num(f.per_m))
                };
                format!(
                    "{}{} {} {} here{}, {}/1M on the provider page",
                    f.field.name(),
                    tier,
                    f.written,
                    f.unit.label(),
                    conv,
                    num(f.expected)
                )
            })
            .collect();
        let error = if real > 0.0 {
            Some((booked - real) / real)
        } else {
            None
        };
        let detail = if call_name == "long-context price" {
            parts.join("; ")
        } else {
            format!(
                "{}. On a {}: booked {}, real {} ({}{}).",
                parts.join("; "),
                call_name,
                usd(booked),
                usd(real),
                if booked >= real { "+" } else { "-" },
                usd((booked - real).abs())
            )
        };
        issues.push(Issue {
            kind: Kind::Drift,
            file: file.clone(),
            line: first_bad.line,
            model: row.model.clone(),
            provider: row.provider.clone(),
            error,
            call: call_name,
            booked: Some(booked),
            real: Some(real),
            detail,
            source_url: row.source_url.clone(),
            checked: row.checked.clone(),
            affected: 0,
        });
    }

    // Fallbacks: what the default does to the listed models the table does not price.
    let mut priced_providers: HashSet<&str> = rep
        .findings
        .iter()
        .map(|f| rows(f.row).provider.as_str())
        .collect();
    if priced_providers.is_empty() {
        priced_providers = crate::index::DEFAULT_PROVIDERS.iter().copied().collect();
    }
    let mut unpriced: Vec<usize> = (0..index.table.rows.len())
        .filter(|&i| {
            let r = rows(i);
            r.status != "retired"
                && r.is_chat()
                && priced_providers.contains(r.provider.as_str())
                && !rep.priced.contains(&i)
                && index.resolve(&r.model) == Some(i)
        })
        .collect();
    unpriced.sort_by(|&a, &b| rows(a).model.cmp(&rows(b).model));
    for fb in &rep.fallbacks {
        let errs: Vec<(usize, f64, f64, f64)> = unpriced
            .iter()
            .map(|&i| {
                let r = rows(i);
                let real = std_cost(r.std.input.unwrap_or(0.0), r.std.output.unwrap_or(0.0));
                let booked = std_cost(fb.input, fb.output);
                (i, (booked - real) / real, booked, real)
            })
            .collect();
        let pick = FLAGSHIPS
            .iter()
            .find_map(|m| errs.iter().find(|e| rows(e.0).model == *m))
            .or_else(|| errs.iter().max_by(|a, b| a.1.abs().total_cmp(&b.1.abs())));
        let rate = if fb.is_zero() {
            "$0".to_string()
        } else {
            format!(
                "${}/${} per 1M (input/output)",
                num(fb.input),
                num(fb.output)
            )
        };
        let (model, provider, error, booked, real, src, checked) = match pick {
            Some(&(i, e, b, r)) => {
                let row = rows(i);
                (
                    row.model.clone(),
                    row.provider.clone(),
                    Some(e),
                    Some(b),
                    Some(r),
                    row.source_url.clone(),
                    row.checked.clone(),
                )
            }
            None => (
                "(any model the table does not list)".to_string(),
                String::new(),
                None,
                None,
                None,
                String::new(),
                String::new(),
            ),
        };
        let worst_under = errs.iter().min_by(|a, b| a.1.total_cmp(&b.1));
        let mut detail = format!(
            "models this code does not price are booked at {} ({}). {} listed {} model(s) are not priced here.",
            rate,
            fb.kind,
            errs.len(),
            {
                let mut p: Vec<&str> = priced_providers.iter().copied().collect();
                p.sort();
                p.join("/")
            }
        );
        if let (Some(b), Some(r)) = (booked, real) {
            detail.push_str(&format!(
                " {} on a {}: booked {}, real {}.",
                model,
                STANDARD_CALL,
                usd(b),
                usd(r)
            ));
        }
        if let Some(&(i, e, b, r)) = worst_under {
            if rows(i).model != model && e < 0.0 {
                detail.push_str(&format!(
                    " Worst: {} booked {}, real {} ({}).",
                    rows(i).model,
                    usd(b),
                    usd(r),
                    pct(e)
                ));
            }
        }
        issues.push(Issue {
            kind: Kind::Fallback,
            file: fb.file.clone(),
            line: fb.line,
            model,
            provider,
            error,
            call: STANDARD_CALL,
            booked,
            real,
            detail,
            source_url: src,
            checked,
            affected: errs.len(),
        });
    }

    // Missing cache prices.
    if !rep.cache_handled_globally {
        let mut cache_priced: HashSet<usize> = HashSet::new();
        let mut where_priced: HashMap<usize, &Finding> = HashMap::new();
        for f in &rep.findings {
            if f.field == Field::CacheRead {
                cache_priced.insert(f.row);
            }
            if f.field == Field::Input && !f.tier {
                where_priced.entry(f.row).or_insert(f);
            }
        }
        let mut list: Vec<(&usize, &&Finding)> = where_priced.iter().collect();
        list.sort_by(|a, b| (&a.1.file, a.1.line).cmp(&(&b.1.file, b.1.line)));
        for (&r, f) in list {
            if cache_priced.contains(&r) || cache_mentioned.contains(&r) {
                continue;
            }
            let row = rows(r);
            let (Some(inp), Some(out), Some(cr)) =
                (row.std.input, row.std.output, row.std.cache_read)
            else {
                continue;
            };
            if cr >= inp * 0.95 {
                continue;
            }
            let fresh = call::PROMPT - call::CACHED;
            let real = call_cost(fresh, inp, call::CACHED, cr, call::OUTPUT, out);
            let booked = call_cost(call::PROMPT, inp, 0, 0.0, call::OUTPUT, out);
            issues.push(Issue {
                kind: Kind::Cache,
                file: f.file.clone(),
                line: f.line,
                model: row.model.clone(),
                provider: row.provider.clone(),
                error: Some((booked - real) / real),
                call: CACHED_CALL,
                booked: Some(booked),
                real: Some(real),
                detail: format!(
                    "no cache-read price here; the provider bills cache reads at {}/1M against {}/1M input. On a {}: booked {} if cached tokens bill at the input price, real {}.",
                    num(cr),
                    num(inp),
                    CACHED_CALL,
                    usd(booked),
                    usd(real)
                ),
                source_url: row.source_url.clone(),
                checked: row.checked.clone(),
                affected: 0,
            });
        }
    }

    // Models used in quotes with no price, for providers the repo prices.
    let found_providers: HashSet<&str> = rep
        .findings
        .iter()
        .map(|f| rows(f.row).provider.as_str())
        .collect();
    for (r, file, line) in &seen {
        let row = rows(*r);
        if rep.priced.contains(r)
            || !found_providers.contains(row.provider.as_str())
            || row.status == "retired"
        {
            continue;
        }
        issues.push(Issue {
            kind: Kind::Missing,
            file: file.clone(),
            line: *line,
            model: row.model.clone(),
            provider: row.provider.clone(),
            error: None,
            call: STANDARD_CALL,
            booked: None,
            real: row
                .std
                .input
                .zip(row.std.output)
                .map(|(i, o)| std_cost(i, o)),
            detail: "used here, but no price for it was found in the scanned files".to_string(),
            source_url: row.source_url.clone(),
            checked: row.checked.clone(),
            affected: 0,
        });
    }

    issues.sort_by(|a, b| (a.kind, &a.file, a.line).cmp(&(b.kind, &b.file, b.line)));
    rep.issues = issues;
    rep
}
