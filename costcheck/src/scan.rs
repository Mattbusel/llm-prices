//! Finds hard-coded LLM prices in one source file and judges each against the table.
//!
//! The matching rules are a port of the scanner behind the llm-prices GitHub Action (the
//! JavaScript in `src/scan.mjs` of the same repo), which has been run against real price
//! tables in agent frameworks and gateways.

use std::collections::HashSet;

use crate::index::{re, Index};
use crate::prices::{Field, Row};

/// The unit a hard-coded number is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unit {
    /// USD per 1M tokens.
    M,
    /// USD per 1K tokens.
    K,
    /// USD per token.
    T,
}

impl Unit {
    pub fn factor(self) -> f64 {
        match self {
            Unit::M => 1.0,
            Unit::K => 1000.0,
            Unit::T => 1e6,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Unit::M => "per 1M",
            Unit::K => "per 1K",
            Unit::T => "per token",
        }
    }
}

/// One hard-coded price compared with the table.
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub file: String,
    /// 1-based line.
    pub line: usize,
    /// Row index in the table.
    pub row: usize,
    pub field: Field,
    /// The number as written.
    pub written: String,
    pub unit: Unit,
    /// The written number in USD per 1M tokens.
    pub per_m: f64,
    /// The table's price for the field, USD per 1M tokens.
    pub expected: f64,
    pub ok: bool,
    /// True when the price sits in a long-context (threshold) tier.
    pub tier: bool,
}

/// What one file contains.
#[derive(Debug, Default)]
pub struct FileScan {
    pub findings: Vec<Finding>,
    /// Models named in quotes, first place each is named: (row, 1-based line).
    pub seen: Vec<(usize, usize)>,
    /// Rows with at least one price found.
    pub priced: HashSet<usize>,
    /// Rows whose price block mentions caching in any form.
    pub cache_mentioned: HashSet<usize>,
}

/// Scan options.
#[derive(Debug, Clone, Copy)]
pub struct Opts {
    /// Relative difference allowed before a price counts as wrong.
    pub tolerance: f64,
    /// Also scan Rust `#[cfg(test)]` items.
    pub include_tests: bool,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            tolerance: 0.01,
            include_tests: false,
        }
    }
}

pub(crate) const MAX_BLOCK: usize = 20;

#[derive(Debug, Clone)]
pub(crate) struct Hit {
    pub row: usize,
    pub col: usize,
    pub len: usize,
    pub quoted: bool,
    pub as_value: bool,
}

/// Known model ids on one line.
pub(crate) fn models_on_line(index: &Index, line: &str) -> Vec<Hit> {
    let mut out = Vec::new();
    for m in re!(r"[A-Za-z0-9][A-Za-z0-9._:@/-]{1,90}").find_iter(line) {
        let Some(row) = index.resolve(m.as_str()) else {
            continue;
        };
        let before = &line[..m.start()];
        let after = &line[m.end()..];
        let q = |c: char| matches!(c, '"' | '\'' | '`');
        let quoted = before.ends_with(q) && after.starts_with(q);
        // Part of a longer string ("GPT-4.1 Mini", "OpenAI GPT-4o") is a display name or prose,
        // not a model id.
        if !quoted
            && ((before.ends_with(q) && after.starts_with(' '))
                || (after.starts_with(q)
                    && before.ends_with(' ')
                    && before.trim_end().ends_with(|c: char| c.is_alphanumeric())))
        {
            continue;
        }
        // Very short ids (o1, o3, gpt-5) must be quoted to count; they collide with words.
        if index.row(row).model.len() < 6 && !quoted && !re!(r"^\s*[:=]").is_match(after) {
            continue;
        }
        let as_value = re!(r#"(?i)(?:^|[^\w])["']?(?:model|model_id|modelid|modelname|model_name|id|name|slug)["']?\s*[:=]\s*["'`]?$"#)
            .is_match(before);
        out.push(Hit {
            row,
            col: m.start(),
            len: m.len(),
            quoted,
            as_value,
        });
    }
    out
}

fn strip_strings(line: &str) -> std::borrow::Cow<'_, str> {
    re!(r#""(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|`[^`]*`"#).replace_all(line, "\"\"")
}

/// `line` with the contents of its string literals blanked, keeping every byte column.
pub(crate) fn blank_strings(line: &str) -> String {
    re!(r#""(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|`[^`]*`"#)
        .replace_all(line, |c: &regex::Captures| {
            let q = &c[0];
            let first = &q[..1];
            let last = &q[q.len() - 1..];
            format!("{first}{}{last}", " ".repeat(q.len() - 2))
        })
        .into_owned()
}

/// Lines with the contents of Rust raw strings blanked; raw strings may span lines and their
/// quotes and braces are unbalanced per line.
fn blank_rust_raw_strings(lines: &[&str]) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    let mut closer: Option<String> = None;
    for raw in lines {
        let mut line = raw.to_string();
        if let Some(c) = closer.take() {
            match line.find(&c) {
                None => {
                    out.push(" ".repeat(line.len()));
                    closer = Some(c);
                    continue;
                }
                Some(end) => {
                    let k = end + c.len();
                    line = format!("{}{}", " ".repeat(k), &line[k..]);
                }
            }
        }
        let mut res = String::new();
        let mut rest: &str = &line;
        loop {
            let Some(m) = re!(r#"\bb?r(#*)""#).captures(rest) else {
                res.push_str(rest);
                break;
            };
            let whole = m.get(0).expect("match");
            let close = format!("\"{}", &m[1]);
            res.push_str(&rest[..whole.end()]);
            let after = &rest[whole.end()..];
            match after.find(&close) {
                None => {
                    closer = Some(close);
                    res.push_str(&" ".repeat(after.len()));
                    break;
                }
                Some(end) => {
                    res.push_str(&" ".repeat(end + close.len()));
                    rest = &after[end + close.len()..];
                }
            }
        }
        out.push(res);
    }
    out
}

pub(crate) fn depth_delta(line: &str) -> i64 {
    let mut d = 0;
    for ch in strip_strings(line).chars() {
        match ch {
            '{' | '[' | '(' => d += 1,
            '}' | ']' | ')' => d -= 1,
            _ => {}
        }
    }
    d
}

pub(crate) fn indent(line: &str) -> i64 {
    let mut n = 0;
    for c in line.chars() {
        match c {
            '\t' => n += 4,
            c if c.is_whitespace() => n += 1,
            _ => break,
        }
    }
    n
}

fn is_dash_item(line: &str) -> bool {
    re!(r"^\s*-\s").is_match(line)
}

fn is_table_header(line: &str) -> bool {
    re!(r"^\s*\[").is_match(line)
}

/// (start, end) lines, inclusive, of the price block that belongs to the model on line `i`.
fn block_for(lines: &[&str], i: usize, hit: &Hit, ext: &str) -> (usize, usize) {
    let line = lines[i];
    let n = lines.len();
    let indent_lang = matches!(ext, "yaml" | "yml" | "py");
    // TOML table headers own everything to the next header.
    if ext == "toml" && is_table_header(line) {
        let mut j = i + 1;
        while j < n && j - i <= MAX_BLOCK && !is_table_header(lines[j]) {
            j += 1;
        }
        return (i, j - 1);
    }
    if hit.as_value {
        if ext == "toml" {
            let mut a = i;
            while a > 0
                && i - a < MAX_BLOCK
                && !is_table_header(lines[a])
                && !lines[a].trim().is_empty()
            {
                a -= 1;
            }
            let mut b = i + 1;
            while b < n
                && b - i <= MAX_BLOCK
                && !is_table_header(lines[b])
                && !lines[b].trim().is_empty()
            {
                b += 1;
            }
            return (a, b - 1);
        }
        if is_dash_item(line) || (indent_lang && ext != "py") {
            let dash = is_dash_item(line);
            let base = if dash { indent(line) } else { indent(line) - 2 };
            let mut a = i;
            if !dash {
                while a > 0
                    && i - a < MAX_BLOCK
                    && indent(lines[a - 1]) > base
                    && !is_dash_item(lines[a])
                {
                    a -= 1;
                }
                if a > 0 && is_dash_item(lines[a - 1]) {
                    a -= 1;
                }
            }
            let mut b = i + 1;
            while b < n
                && b - i <= MAX_BLOCK
                && !lines[b].trim().is_empty()
                && indent(lines[b]) > indent(lines[a])
            {
                b += 1;
            }
            return (a, b - 1);
        }
        // Bracketed record: walk back to its opening brace and forward to its close.
        let mut a = i;
        let mut d = depth_delta(&line[..hit.col]);
        while d <= 0 && a > 0 && i - a < MAX_BLOCK {
            a -= 1;
            d += depth_delta(lines[a]);
        }
        if d <= 0 {
            // Not inside a record (a module-level constant): only its own line belongs to it.
            return (i, i);
        }
        let mut b = i;
        let mut e = depth_delta(&line[hit.col..]);
        if d > 0 {
            e += d;
            while e > 0 && b + 1 < n && b - i < MAX_BLOCK {
                b += 1;
                e += depth_delta(lines[b]);
            }
        }
        return (a, b);
    }
    // The model is a key, match arm, case label or tuple head: the block is what follows.
    let mut d = depth_delta(&line[hit.col..]);
    let mut b = i;
    if d > 0 {
        while d > 0 && b + 1 < n && b - i < MAX_BLOCK {
            b += 1;
            d += depth_delta(lines[b]);
        }
        return (i, b);
    }
    let t = line.trim();
    if re!(r":\s*(#.*)?$").is_match(t) || re!(r"=>\s*$").is_match(t) {
        let base = indent(line);
        while b + 1 < n
            && b - i < MAX_BLOCK
            && !lines[b + 1].trim().is_empty()
            && indent(lines[b + 1]) > base
        {
            b += 1;
        }
        return (i, b);
    }
    (i, i)
}

pub(crate) const NUM: &str = r"-?\d[\d_]*(?:\.\d+)?(?:[eE][-+]?\d+)?";
pub(crate) const DIV: &str =
    r"(?:\s*/\s*(1_?000_?000(?:\.0*)?|1e6|1\.0e6|1_?000(?:\.0*)?|1e3|1\.0e3))?";

/// `key: number` in any of the usual spellings. Groups: key, number, divisor.
pub(crate) fn kv() -> &'static regex::Regex {
    static R: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    R.get_or_init(|| {
        regex::Regex::new(&format!(
            r#"["']?([A-Za-z_][A-Za-z0-9_\-.]{{0,60}}?)["']?\s*(?::|=>|=)\s*(?:[A-Za-z_][A-Za-z0-9_:!]*\(\s*)?["']?\$?({NUM})["']?{DIV}"#
        ))
        .expect("valid regex")
    })
}

/// "(input, output" right after a model id. Groups: number, divisor, number, divisor.
pub(crate) fn pos() -> &'static regex::Regex {
    static R: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    R.get_or_init(|| {
        regex::Regex::new(&format!(
            r#"^["'`]?[\s,:=>)\]]*(?:[A-Za-z_][\w:!]*\s*)?[(\[{{]*\s*(?:[A-Za-z_][\w:!]*\(\s*)?\$?({NUM}){DIV}\s*,\s*(?:[A-Za-z_][\w:!]*\(\s*)?\$?({NUM}){DIV}(?:[^\w.]|$)"#
        ))
        .expect("valid regex")
    })
}

pub(crate) fn parse_num(s: &str) -> f64 {
    s.replace('_', "").parse().unwrap_or(f64::NAN)
}

pub(crate) fn div_unit(div: Option<&str>) -> Option<Unit> {
    let n = parse_num(div?);
    if n == 1e6 {
        Some(Unit::M)
    } else if n == 1e3 {
        Some(Unit::K)
    } else {
        None
    }
}

/// The price field a key names, or `None` when the key is not a price.
pub fn classify_key(raw: &str) -> Option<Field> {
    let k = re!(r"[^a-z0-9]+")
        .replace_all(&raw.to_lowercase(), "_")
        .into_owned();
    // Ratios, not prices: cacheWriteMultiplier: 1.25, batch_discount: 0.5.
    if re!(
        r"multiplier|mult(_|$)|ratio|factor|discount|markup|uplift|surcharge|percent|pct|fraction"
    )
    .is_match(&k)
    {
        return None;
    }
    let money = re!(r"cost|price|rate|usd|dollar|per_|_per|mtok|fee").is_match(&k);
    if !money
        && re!(r"context|window|max|limit|rpm|tpm|rpd|size|length|version|temperature|top_p|top_k|dimension|seconds|timeout|retries|count|budget|threshold|weight|score|quantity")
            .is_match(&k)
    {
        return None;
    }
    if !money && re!(r"tokens?(_|$)|(^|_)tok(_|$)").is_match(&k) {
        return None;
    }
    // Credits, points and other currencies are not USD prices.
    if re!(r"credit|point|coin|token_?unit|(^|_)(eur|cny|rmb|yuan|jpy|inr|krw)(_|$)").is_match(&k) {
        return None;
    }
    // Prices per image, audio second, query or call are not per-token prices, and audio,
    // image and video tokens have their own prices.
    if re!(r"image|audio|video|pixel|per_?(second|minute|hour|query|queries|call|request|page|character|char)(_|$)|grounding|search|(^|_)web(_|$)|(^|_)tool|storage|(^|_)file").is_match(&k) {
        return None;
    }
    let batch = k.contains("batch");
    let field = if re!(r"cache|cached").is_match(&k) {
        if re!(r"write|creation|create|store|_5m|_1h|5min|1hour|_30m|_12h|5_m|1_h").is_match(&k) {
            Field::CacheWrite
        } else if re!(r"read|hit|cached|cache_input|cache_in\b").is_match(&k) {
            Field::CacheRead
        } else {
            return None;
        }
    } else if re!(r"(^|_)(input|prompt|in|request)(_|$)|input|prompt").is_match(&k) {
        Field::Input
    } else if re!(r"(^|_)(output|completion|out|response|generation)(_|$)|output|completion")
        .is_match(&k)
    {
        Field::Output
    } else {
        return None;
    };
    if batch && matches!(field, Field::CacheRead | Field::CacheWrite) {
        // Batch cache prices are not in the table; never guess them.
        return None;
    }
    Some(match (batch, field) {
        (true, Field::Input) => Field::BatchInput,
        (true, Field::Output) => Field::BatchOutput,
        (_, f) => f,
    })
}

/// Unit hint from a key or comment.
pub fn unit_hint(text: &str) -> Option<Unit> {
    let t = text.to_lowercase();
    if re!(r"per_?token|cost_per_token|token_cost|per\s+token\b|/\s*token\b").is_match(&t) {
        return Some(Unit::T);
    }
    if re!(r"1k|per_?1_?000(?:$|[^_0]|_[^0])|per_?thousand|per\s+1,?000\s+tokens|/\s*1k|_k_|per_k\b|kilo").is_match(&t) {
        return Some(Unit::K);
    }
    if re!(r"1m\b|1m_|_1m|mtok|million|per_?1_?000_?000|per\s+1,?000,?000|/\s*1m|per_m\b|_m_tok")
        .is_match(&t)
    {
        return Some(Unit::M);
    }
    None
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= (tol * b.abs()).max(1e-9)
}

/// The verdict on one written number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Verdict {
    pub ok: bool,
    pub per_m: f64,
    pub unit: Unit,
    pub expected: f64,
}

/// Compares one written number with the row. `None` when the number is not a plausible price
/// for this field.
pub fn judge(
    row: &Row,
    field: Field,
    value: f64,
    unit: Option<Unit>,
    tol: f64,
    soft: Option<Unit>,
) -> Option<Verdict> {
    if value.is_nan() || value <= 0.0 || !value.is_finite() {
        return None;
    }
    // A batch price for a model the table lists no batch price for cannot be judged.
    if matches!(field, Field::BatchInput | Field::BatchOutput) && row.std.primary(field).is_none() {
        return None;
    }
    let expected = row.primary(field)?;
    let ok_set = row.acceptable(field);
    let units: Vec<Unit> = match unit {
        Some(u) => vec![u],
        None => vec![Unit::M, Unit::K, Unit::T],
    };
    for &u in &units {
        let per_m = value * u.factor();
        if ok_set.iter().any(|&v| close(per_m, v, tol)) {
            return Some(Verdict {
                ok: true,
                per_m,
                unit: u,
                expected,
            });
        }
    }
    // Read the number in its most likely unit; far-off numbers are not prices at all.
    let ratio_of = |u: Unit| {
        let per_m = value * u.factor();
        (per_m, u, (per_m / expected).max(expected / per_m))
    };
    let mut best = None;
    if unit.is_none() {
        if let Some(s) = soft {
            let r = ratio_of(s);
            if r.2 <= 20.0 {
                best = Some(r);
            }
        }
    }
    if best.is_none() {
        for &u in &units {
            let r = ratio_of(u);
            if best.map_or(true, |b: (f64, Unit, f64)| r.2 < b.2) {
                best = Some(r);
            }
        }
    }
    let (per_m, u, ratio) = best?;
    let limit = if unit.is_some() { 50.0 } else { 20.0 };
    if ratio > limit {
        return None;
    }
    Some(Verdict {
        ok: false,
        per_m,
        unit: u,
        expected,
    })
}

/// Keys that start a price tier above a prompt size: `{ threshold: 200000, prompt: 2.5 }`.
pub fn is_threshold_key(raw: &str) -> bool {
    let k = re!(r"[^a-z0-9]+")
        .replace_all(&raw.to_lowercase(), "_")
        .into_owned();
    if re!(r"cost|price|rate|usd|dollar|cache").is_match(&k) {
        return false;
    }
    re!(r"threshold|(^|_)(above|over|gt|gte)(_|$)|min_?(prompt_|input_|context_)?tokens|tokens_?(above|over)").is_match(&k)
}

type Pos = (usize, usize);
type Span = (Pos, Pos);

fn is_open(c: u8) -> bool {
    matches!(c, b'{' | b'[' | b'(')
}

fn is_close(c: u8) -> bool {
    matches!(c, b'}' | b']' | b')')
}

/// The records that hold a threshold key: prices inside them are long-context prices.
fn tier_spans(
    lines: &[&str],
    lo: usize,
    hi: usize,
    ext: &str,
    clean: &mut Option<Vec<String>>,
) -> Vec<Span> {
    let mut spans = Vec::new();
    if !lines[lo..=hi]
        .iter()
        .any(|l| re!(r"(?i)threshold|above|over|gt|tokens").is_match(l))
    {
        return spans;
    }
    let clean = clean.get_or_insert_with(|| lines.iter().map(|l| blank_strings(l)).collect());
    let n = lines.len();
    let close_of = |y0: usize, x0: usize| -> Pos {
        let mut depth = 0i64;
        let mut y = y0;
        while y < n && y <= hi + MAX_BLOCK {
            let t = clean[y].as_bytes();
            let start = if y == y0 { x0 } else { 0 };
            for (x, &c) in t.iter().enumerate().skip(start) {
                if is_open(c) {
                    depth += 1;
                } else if is_close(c) {
                    depth -= 1;
                    if depth == 0 {
                        return (y, x);
                    }
                }
            }
            y += 1;
        }
        ((n - 1).min(hi + MAX_BLOCK), usize::MAX)
    };
    for j in lo..=hi {
        for m in
            re!(r#"["']?([A-Za-z_][A-Za-z0-9_]*)["']?\s*(?::|=>|=)\s*\d"#).captures_iter(lines[j])
        {
            if !is_threshold_key(&m[1]) {
                continue;
            }
            let at = m.get(0).expect("match").start();
            let mut open = None;
            let mut depth = 0i64;
            let floor = lo.saturating_sub(MAX_BLOCK);
            let mut y = j as i64;
            while y >= floor as i64 && open.is_none() {
                let yy = y as usize;
                let t = clean[yy].as_bytes();
                let start = if yy == j { at } else { t.len() };
                for x in (0..start.min(t.len())).rev() {
                    if is_close(t[x]) {
                        depth += 1;
                    } else if is_open(t[x]) {
                        if depth == 0 {
                            open = Some((yy, x));
                            break;
                        }
                        depth -= 1;
                    }
                }
                y -= 1;
            }
            if let Some(o) = open {
                spans.push((o, close_of(o.0, o.1)));
            } else if matches!(ext, "yaml" | "yml") && is_dash_item(lines[j]) {
                let mut b = j;
                while b + 1 < n
                    && b - j < MAX_BLOCK
                    && !lines[b + 1].trim().is_empty()
                    && indent(lines[b + 1]) > indent(lines[j])
                {
                    b += 1;
                }
                spans.push(((j, 0), (b, usize::MAX)));
            } else {
                spans.push(((j, 0), (j, usize::MAX)));
            }
        }
    }
    spans
}

fn in_span(spans: &[Span], line: usize, col: usize) -> bool {
    spans
        .iter()
        .any(|&(a, b)| a <= (line, col) && (line, col) <= b)
}

/// Lines inside Rust `#[cfg(test)]` items.
pub(crate) fn test_mask(lines: &[&str], ext: &str) -> Option<Vec<bool>> {
    if ext != "rs" {
        return None;
    }
    let mut mask = vec![false; lines.len()];
    let clean = blank_rust_raw_strings(lines);
    let mut i = 0;
    while i < lines.len() {
        if !re!(r"^\s*#\[cfg\(test\)\]").is_match(lines[i]) {
            i += 1;
            continue;
        }
        let mut depth = 0i64;
        let mut opened = false;
        let mut j = i;
        while j < lines.len() {
            mask[j] = true;
            for ch in strip_strings(&clean[j]).chars() {
                if ch == '{' {
                    depth += 1;
                    opened = true;
                } else if ch == '}' {
                    depth -= 1;
                }
            }
            if opened && depth <= 0 {
                break;
            }
            if !opened && j > i && re!(r";\s*$").is_match(&clean[j]) {
                break;
            }
            j += 1;
        }
        i = j + 1;
    }
    Some(mask)
}

/// Test files are skipped unless include-tests is on.
pub fn is_test_file(file: &str) -> bool {
    re!(r"(^|/)(tests?|__tests__|spec|specs|benches|testdata|fixtures)/").is_match(file)
        || re!(r"(_test\.(go|py|rs)|\.(test|spec)\.[cm]?[jt]sx?|(^|/)test_[^/]*\.py|(^|/)conftest\.py)$").is_match(file)
}

pub fn ext_of(file: &str) -> String {
    match file.rfind('.') {
        Some(i) if !file[i + 1..].contains('/') => file[i + 1..].to_lowercase(),
        _ => String::new(),
    }
}

/// Files the scan reads.
pub fn scannable(file: &str) -> bool {
    if re!(r"(^|/)(package-lock\.json|Cargo\.lock|yarn\.lock|pnpm-lock\.yaml|go\.sum|poetry\.lock|uv\.lock)$").is_match(file) {
        return false;
    }
    if file.ends_with(".min.js") {
        return false;
    }
    matches!(
        ext_of(file).as_str(),
        "json"
            | "toml"
            | "yaml"
            | "yml"
            | "py"
            | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "mjs"
            | "cjs"
            | "rs"
            | "go"
            | "java"
            | "kt"
            | "cs"
            | "rb"
            | "php"
            | "swift"
    )
}

fn is_group_line(line: &str) -> bool {
    const Q: &str = r#"["'`][^"'`]+["'`]"#;
    static R: std::sync::OnceLock<[regex::Regex; 3]> = std::sync::OnceLock::new();
    let rs = R.get_or_init(|| {
        [
            regex::Regex::new(&format!(r"^\s*(?:{Q}\s*\|\s*)+{Q}\s*(?:=>|:)"))
                .expect("valid regex"),
            regex::Regex::new(&format!(r"^\s*case\s+(?:{Q}\s*,\s*)*{Q}\s*:")).expect("valid regex"),
            regex::Regex::new(&format!(
                r"\bin\s*[\[(]\s*(?:{Q}\s*,\s*)+{Q}\s*,?\s*[\])]\s*:\s*$"
            ))
            .expect("valid regex"),
        ]
    });
    rs.iter().any(|r| r.is_match(line))
}

/// The unit hinted by comments in the first 40 lines of a file.
pub(crate) fn file_hint(lines: &[&str]) -> Option<Unit> {
    let text: Vec<&str> = lines
        .iter()
        .take(40)
        .copied()
        .filter(|l| re!(r#"//|#|/\*|\*|"""|'''"#).is_match(l))
        .collect();
    unit_hint(&text.join("\n"))
}

struct Found {
    line: usize,
    field: Field,
    raw: String,
    value: f64,
    unit: Option<Unit>,
    soft: Option<Unit>,
    tier: bool,
    /// The key names a service tier (flex, priority, fast, ...): only a match is reported,
    /// since the table may not list that tier.
    service: bool,
}

/// True when the structure around line `i` is named for something other than prices: image
/// tile token counts, provisioned throughput capacity, rate limits, context windows.
fn non_price_container(lines: &[&str], i: usize) -> bool {
    let lo = i.saturating_sub(40);
    for j in (lo..=i).rev() {
        if let Some(c) = re!(r"^\s*(?:(?:pub(?:\([^)]*\))?|export|const|static|let|var|final|private|public|readonly)\s+)*([A-Za-z_]\w*)\s*(?::[^=]*)?=\s*[^=]").captures(lines[j]) {
            let name = c[1].to_lowercase();
            let pricey = re!(r"cost|price|pricing|rate|usd|dollar|tariff").is_match(&name);
            return re!(r"image|tile|pixel|audio|video|capacity|ptu|throughput|limit|window|context|rpm|tpm|quota").is_match(&name)
                || (!pricey && re!(r"token|count|size|len").is_match(&name));
        }
    }
    false
}

/// The key of the innermost object that is still open at line `j`, looking back no further
/// than line `floor` (the model's own line).
fn enclosing_key(lines: &[&str], floor: usize, j: usize) -> Option<String> {
    let mut depth = 0i64;
    for y in (floor..j).rev() {
        depth += depth_delta(lines[y]);
        if depth > 0 {
            return re!(r#"["']?([A-Za-z_][\w.\-]*)["']?\s*(?::|=)\s*\{\s*$"#)
                .captures(lines[y].trim_end())
                .map(|c| c[1].to_lowercase());
        }
    }
    None
}

/// True for a name that is a host, cloud or reseller (whose prices for a model are its own),
/// not a model maker.
pub fn is_host_name(name: &str) -> bool {
    let n: String = name
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    if n.is_empty()
        || matches!(
            n.as_str(),
            "anthropic"
                | "openai"
                | "google"
                | "gemini"
                | "googleaistudio"
                | "googleai"
                | "deepseek"
                | "models"
                | "pricing"
                | "prices"
        )
    {
        return false;
    }
    re!(r"azure|vertex|bedrock|sagemaker|fireworks|together|groq|openrouter|deepinfra|cerebras|sambanova|replicate|databricks|snowflake|watsonx|oci|cloudflare|nebius|novita|hyperbolic|baseten|lambda|perplexity|mistral|xai|cohere|ai21|302ai|siliconflow|kyma|hostinger|github|copilot|vercel|helicone|portkey|requesty|zenmux|aihubmix|chutes|venice|nvidia|ollama|lmstudio|alibaba|qwen|tencent|streamlake|moonshot|zhipu|minimax")
        .is_match(&n)
}

/// The section each line of a provider-keyed price map sits in: for JSON files whose top-level
/// keys are providers (`{"302ai": {...}, "anthropic": {...}}`), the provider name; `None`
/// outside such sections. A file named for a host (`vertex_ai.json`) is one section.
fn sections(index: &Index, file: &str, ext: &str, lines: &[&str]) -> Option<Vec<Option<String>>> {
    if !matches!(ext, "json" | "yaml" | "yml" | "toml") {
        return None;
    }
    let stem = file.rsplit('/').next().unwrap_or(file);
    let stem = stem.rsplit_once('.').map_or(stem, |(a, _)| a);
    if is_host_name(stem) {
        return Some(vec![Some(stem.to_lowercase()); lines.len()]);
    }
    if ext != "json" {
        return None;
    }
    let mut out = vec![None; lines.len()];
    let mut depth = 0i64;
    let mut cur: Option<String> = None;
    for (i, l) in lines.iter().enumerate() {
        // A key that opens an object at depth 1 starts a new section, unless the key is itself a
        // model id (a LiteLLM-style map) or a neutral word (pricing, models, data, ...).
        if depth == 1 {
            if let Some(c) = re!(r#"^\s*"([^"]+)"\s*:\s*\{"#).captures(l) {
                let key = c[1].to_lowercase();
                let neutral = key.contains('/')
                    || index.resolve(&key).is_some()
                    || index.is_foreign(&key)
                    || re!(r"^_*(?:pric\w*|costs?|rates?|models?|data|defaults?|tiers?|version|schema|meta(?:data)?|updated(?:_at)?|generated(?:_at)?|sources?|currency|units?|config|settings?|chat|completions?|embeddings?|llms?|text)_*$").is_match(&key);
                cur = if neutral { None } else { Some(key) };
            }
        }
        if depth >= 1 {
            out[i] = cur.clone();
        }
        depth += depth_delta(l);
    }
    Some(out)
}

/// True when a section name is the model maker `provider` itself.
fn section_is_maker(section: &str, provider: &str) -> bool {
    let n: String = section
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    match provider {
        "google" => matches!(
            n.as_str(),
            "google" | "gemini" | "googleai" | "googleaistudio" | "googlegenai" | "googlegemini"
        ),
        p => n == p,
    }
}

/// Scans one file's text.
pub fn scan_text(file: &str, text: &str, index: &Index, opts: Opts) -> FileScan {
    let tol = opts.tolerance;
    let ext = ext_of(file);
    let lines: Vec<&str> = text
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    let mut out = FileScan::default();
    let mut seen_rows = HashSet::new();
    let skip = if opts.include_tests {
        None
    } else {
        test_mask(&lines, &ext)
    };
    let mut blanked: Option<Vec<String>> = None;
    let fhint = file_hint(&lines);
    let sections = sections(index, file, &ext, &lines);
    let mut keys = HashSet::new();
    for i in 0..lines.len() {
        let line = lines[i];
        if line.len() > 2000 || skip.as_ref().is_some_and(|m| m[i]) {
            continue;
        }
        let mut hits = models_on_line(index, line);
        // In a provider-keyed map, only the maker's own section quotes the maker's price.
        if let Some(sec) = sections.as_ref().and_then(|s| s[i].as_deref()) {
            hits.retain(|h| section_is_maker(sec, &index.row(h.row).provider));
        }
        if hits.is_empty() {
            continue;
        }
        // A line that also names another provider's own row (OpenRouter, Bedrock, a host)
        // is that provider's price, not the model maker's.
        if re!(r"[A-Za-z0-9][A-Za-z0-9._:@/-]{1,90}")
            .find_iter(line)
            .any(|m| index.is_foreign(m.as_str()))
        {
            continue;
        }
        for h in &hits {
            if h.quoted && seen_rows.insert(h.row) {
                out.seen.push((h.row, i + 1));
            }
        }
        let mut targets: Vec<usize> = Vec::new();
        for h in &hits {
            if !targets.contains(&h.row) {
                targets.push(h.row);
            }
        }
        // Several models on one line share a price block only when the line is one match arm
        // or case label for all of them.
        let group = targets.len() > 1 && hits.iter().all(|h| h.quoted) && is_group_line(line);
        if targets.len() != 1 && !group {
            continue;
        }
        let hit = hits.last().expect("non-empty").clone();
        let (a, b) = block_for(&lines, i, &hit, &ext);
        let names_other = |j: usize| {
            models_on_line(index, lines[j])
                .iter()
                .any(|x| !targets.contains(&x.row))
        };
        let mut end = b;
        for j in i + 1..=b {
            if names_other(j) {
                end = j - 1;
                break;
            }
        }
        let mut start = a;
        for j in (a..i).rev() {
            if names_other(j) {
                start = j + 1;
                break;
            }
        }
        if lines[start..=end]
            .iter()
            .any(|l| l.to_lowercase().contains("cach"))
        {
            out.cache_mentioned.extend(targets.iter().copied());
        }
        let context = lines[start.saturating_sub(3)..=end].join("\n");
        let context = re!(r#""[^"\n]*""#).replace_all(&context, |c: &regex::Captures| {
            if re!(r"(?i)per|1k|1m|mtok|million|thousand").is_match(&c[0]) {
                c[0].to_string()
            } else {
                "\"\"".to_string()
            }
        });
        let soft = unit_hint(&context).or(fhint);
        let mut found: Vec<Found> = Vec::new();
        let spans = tier_spans(&lines, start, end, &ext, &mut blanked);
        for j in start..=end {
            // Drop the model tokens so ids like gpt-4.1 never look like numbers.
            let mut l = lines[j].to_string();
            for h in models_on_line(index, lines[j]) {
                l.replace_range(h.col..h.col + h.len, &" ".repeat(h.len));
            }
            let mut any_key = false;
            // A nested object named for a tier ("batch": {...}, "priority": {...},
            // "context_over_200k": {...}) prices that tier, not the standard one.
            let parent = if j > i {
                enclosing_key(&lines, i, j)
            } else {
                None
            };
            let parent = parent.as_deref().unwrap_or("");
            let parent_batch = re!(r"(?i)batch").is_match(parent);
            let parent_service =
                re!(r"(?i)flex|priority|fast|offpeak|off_peak|scale|reserved|provisioned|tier")
                    .is_match(parent)
                    && !parent_batch;
            let parent_long = re!(r"(?i)long|above|over|context_over|\d+k").is_match(parent);
            for m in kv().captures_iter(&l) {
                any_key = true;
                let Some(mut field) = classify_key(&m[1]) else {
                    continue;
                };
                if parent_batch {
                    field = match field {
                        Field::Input => Field::BatchInput,
                        Field::Output => Field::BatchOutput,
                        Field::BatchInput | Field::BatchOutput => field,
                        // Batch cache prices are not in the table.
                        _ => continue,
                    };
                }
                let unit = div_unit(m.get(3).map(|x| x.as_str())).or_else(|| unit_hint(&m[1]));
                let keyed_long = re!(r"(?i)(above|over|gt)_?\d+k").is_match(&m[1]);
                found.push(Found {
                    line: j + 1,
                    field,
                    raw: m[2].to_string(),
                    value: parse_num(&m[2]),
                    unit,
                    soft,
                    // Keys like input_cost_per_token_above_200k_tokens name their own tier.
                    tier: in_span(&spans, j, m.get(0).expect("match").start()) || keyed_long || parent_long,
                    service: parent_service
                        || ((keyed_long || parent_long) && (parent_batch || m[1].to_lowercase().contains("batch")))
                        || re!(r"(?i)flex|priority|fast|offpeak|off_peak|standard_only|scale|reserved|provisioned").is_match(&m[1]),
                });
            }
            // Positional tuple on the model's own line: (input, output).
            if j == i && !any_key {
                let from = hit.col + hit.len;
                if let Some(pm) = pos()
                    .captures(&l[from..])
                    .filter(|_| !non_price_container(&lines, i))
                {
                    let tier = in_span(&spans, j, from);
                    let n0 = (
                        pm[1].to_string(),
                        parse_num(&pm[1]),
                        div_unit(pm.get(2).map(|x| x.as_str())),
                    );
                    let n1 = (
                        pm[3].to_string(),
                        parse_num(&pm[3]),
                        div_unit(pm.get(4).map(|x| x.as_str())),
                    );
                    let plausible = targets.iter().any(|&t| {
                        let row = index.row(t);
                        let r = if tier {
                            row.tier_row()
                        } else {
                            Some(row.clone())
                        };
                        r.is_some_and(|r| {
                            judge(&r, Field::Input, n0.1, n0.2, tol, soft).is_some()
                                && judge(&r, Field::Output, n1.1, n1.2, tol, soft).is_some()
                        })
                    });
                    if plausible {
                        for (field, n) in [(Field::Input, n0), (Field::Output, n1)] {
                            found.push(Found {
                                line: j + 1,
                                field,
                                raw: n.0,
                                value: n.1,
                                unit: n.2,
                                soft,
                                tier,
                                service: false,
                            });
                        }
                    }
                }
            }
        }
        for f in &found {
            for &t in &targets {
                let row = index.row(t);
                let against = if f.tier {
                    row.tier_row()
                } else {
                    Some(row.clone())
                };
                let Some(against) = against else { continue };
                let Some(v) = judge(&against, f.field, f.value, f.unit, tol, f.soft) else {
                    continue;
                };
                if f.service && !v.ok {
                    continue;
                }
                out.priced.insert(t);
                if !keys.insert((f.line, f.field, f.raw.clone(), t)) {
                    continue;
                }
                out.findings.push(Finding {
                    file: file.to_string(),
                    line: f.line,
                    row: t,
                    field: f.field,
                    written: f.raw.clone(),
                    unit: v.unit,
                    per_m: v.per_m,
                    expected: v.expected,
                    ok: v.ok,
                    tier: f.tier,
                });
            }
        }
    }
    out
}
