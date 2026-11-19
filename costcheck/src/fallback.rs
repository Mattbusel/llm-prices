//! Finds default prices: the price a cost tracker silently uses for a model it does not know.
//!
//! Shapes found in real trackers:
//!
//! - a catch-all match arm or switch default: `_ => (1.0, 3.0)`, `default: return Rates{...}`
//! - a named default: `DEFAULT_PRICING = {"input": 1, "output": 3}`, `fn fallback_cost() -> ...`
//! - a default key in a price table: `"default": {"input": 1.0, "output": 3.0}`
//! - an inline default at the lookup: `PRICES.get(model, {"input": 1, "output": 3})`,
//!   `PRICES[model] ?? { input: 1, output: 3 }`, `.unwrap_or((1.0, 3.0))`
//!
//! A default of zero is reported too: it books every unknown model at $0.

use crate::index::{re, Index};
use crate::prices::Field;
use crate::scan::{
    blank_strings, classify_key, depth_delta, div_unit, file_hint, indent, kv, models_on_line,
    parse_num, pos, test_mask, unit_hint, Finding, Unit, MAX_BLOCK,
};

/// A default price for unknown models.
#[derive(Debug, Clone, PartialEq)]
pub struct Fallback {
    pub file: String,
    /// 1-based line.
    pub line: usize,
    /// Which shape: `match-default`, `named-default`, `default-key` or `inline-default`.
    pub kind: &'static str,
    /// Input price, USD per 1M tokens.
    pub input: f64,
    /// Output price, USD per 1M tokens.
    pub output: f64,
    /// The source line, trimmed.
    pub snippet: String,
}

impl Fallback {
    pub fn is_zero(&self) -> bool {
        self.input == 0.0 && self.output == 0.0
    }
}

fn starts_scope(line: &str) -> bool {
    re!(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:fn|def|func|function|class|impl)\b")
        .is_match(line)
        || re!(r"^\}\s*$").is_match(line)
}

/// True when a priced line sits in the 80 lines above `i` with no new function between.
fn priced_above(lines: &[&str], i: usize, priced_lines: &[usize]) -> bool {
    let lo = i.saturating_sub(80);
    let Some(&nearest) = priced_lines.iter().filter(|&&l| l > lo && l - 1 < i).max() else {
        return false;
    };
    !(nearest..i).any(|j| starts_scope(lines[j]))
}

fn priced_near(i: usize, priced_lines: &[usize]) -> bool {
    priced_lines
        .iter()
        .any(|&l| (l as i64 - 1 - i as i64).abs() <= 80)
}

/// Zero after an arm or default: `=> 0.0`, `: return 0`, `=> (0.0, 0.0)`, `=> Some((0.0, 0.0))`.
fn zero_value(rest: &str, whole_line: bool) -> bool {
    let Some(m) = re!(r"^\s*(?:=>|->|:|,)?\s*(?:return\s+)?(?:Some\(\s*)?[(\[{]?\s*0(?:\.0*)?(?:\s*,\s*0(?:\.0*)?)*\s*[)\]}]?\s*\)?")
        .find(rest)
    else {
        return false;
    };
    let tail = &rest[m.end()..];
    if tail.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '.' || c == '_') {
        return false;
    }
    !whole_line || re!(r"^[\s,;)}\]]*(?://.*|#.*)?$").is_match(tail)
}

/// A guard that skips a model it cannot price and books nothing:
/// `if model not in TOKEN_COSTS: return`, `if (!PRICES[model]) return 0;`,
/// `let Some(p) = PRICES.get(model) else { return 0.0 };`.
fn skips_unknown(lines: &[&str], i: usize) -> bool {
    let line = lines[i];
    let guard = re!(r"(?i)^\s*if\s+(?:not\s+)?[\w.\[\]]*model\w*\s+not\s+in\s+[\w.]*(?:cost|pric)\w*")
        .find(line)
        .or_else(|| re!(r"(?i)^\s*if\s*\(\s*!\s*(?:[\w.]*(?:cost|pric)\w*\s*(?:\[\s*[\w.]*model\w*\s*\]|\.(?:get|has)\(\s*[\w.]*model\w*\s*\))|\(\s*[\w.]*model\w*\s+in\s+[\w.]*(?:cost|pric)\w*\s*\))\s*\)").find(line))
        .or_else(|| re!(r"(?i)^\s*let\s+Some\([^)]*\)\s*=\s*[\w.:]*(?:cost|pric)\w*[\w.:]*\.get\([^)]*model[^)]*\)[^{]*else\s*\{").find(line));
    let Some(g) = guard else { return false };
    // The guard's body: the rest of this line and up to three more lines.
    let mut body = vec![&line[g.end()..]];
    body.extend(lines.iter().skip(i + 1).take(3).copied());
    let mut returned = false;
    for (k, l) in body.iter().enumerate() {
        let t = l.trim().trim_start_matches(':').trim();
        if re!(r"\b(raise|throw|panic!|Err\(|bail!|unreachable!|assert)").is_match(t) {
            return false;
        }
        // A bare return from a cost update, or a cost of zero. `return None` is left alone:
        // an explicit "unknown" is the caller's to handle.
        if re!(r"^return\s*(?:0|0\.0+|Ok\(\(\)\)|Ok\(0(?:\.0+)?\)|\(\s*0(?:\.0+)?\s*,\s*0(?:\.0+)?\s*\))?\s*;?\s*\}?\s*;?\s*(?:#.*|//.*)?$").is_match(t) {
            returned = true;
            break;
        }
        // Only logging may come before the return.
        if k > 0
            && !t.is_empty()
            && !re!(
                r"(?i)^(?:logger|logging|log|console|warnings|tracing|eprintln!|println!|print)\b"
            )
            .is_match(t)
        {
            break;
        }
    }
    returned
}

struct Extracted {
    input: Option<(f64, Option<Unit>)>,
    output: Option<(f64, Option<Unit>)>,
}

/// Prices in the block that starts at (`i`, `col`).
fn extract(index: &Index, lines: &[&str], i: usize, col: usize) -> Extracted {
    let n = lines.len();
    let first = &lines[i][col..];
    let mut b = i;
    let mut d = depth_delta(first);
    if d > 0 {
        while d > 0 && b + 1 < n && b - i < MAX_BLOCK {
            b += 1;
            d += depth_delta(lines[b]);
        }
    } else if re!(r"(?::|=>|->|\{)\s*(?:#.*|//.*)?$").is_match(first.trim_end()) {
        let base = indent(lines[i]);
        while b + 1 < n
            && b - i < MAX_BLOCK
            && !lines[b + 1].trim().is_empty()
            && indent(lines[b + 1]) > base
        {
            b += 1;
        }
    }
    let mut ex = Extracted {
        input: None,
        output: None,
    };
    for j in i..=b {
        if j > i && !models_on_line(index, lines[j]).is_empty() {
            break;
        }
        let seg = if j == i { first } else { lines[j] };
        let mut any_key = false;
        for m in kv().captures_iter(seg) {
            any_key = true;
            let Some(field) = classify_key(&m[1]) else {
                continue;
            };
            let unit = div_unit(m.get(3).map(|x| x.as_str())).or_else(|| unit_hint(&m[1]));
            let v = (parse_num(&m[2]), unit);
            match field {
                Field::Input if ex.input.is_none() => ex.input = Some(v),
                Field::Output if ex.output.is_none() => ex.output = Some(v),
                _ => {}
            }
        }
        if j == i && !any_key {
            if let Some(pm) = pos().captures(seg) {
                ex.input = Some((parse_num(&pm[1]), div_unit(pm.get(2).map(|x| x.as_str()))));
                ex.output = Some((parse_num(&pm[3]), div_unit(pm.get(4).map(|x| x.as_str()))));
            }
        }
    }
    ex
}

/// The unit the file's own prices are written in, when they agree.
fn file_unit(findings: &[Finding]) -> Option<Unit> {
    let mut counts = [0usize; 3];
    for f in findings {
        counts[match f.unit {
            Unit::M => 0,
            Unit::K => 1,
            Unit::T => 2,
        }] += 1;
    }
    let (i, &c) = counts.iter().enumerate().max_by_key(|(_, &c)| c)?;
    if c == 0 {
        return None;
    }
    Some([Unit::M, Unit::K, Unit::T][i])
}

/// Default prices in one file. `findings` are the file's own price findings.
pub fn find_fallbacks(
    file: &str,
    text: &str,
    index: &Index,
    findings: &[Finding],
    include_tests: bool,
) -> Vec<Fallback> {
    let ext = crate::scan::ext_of(file);
    let lines: Vec<&str> = text
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    let skip = if include_tests {
        None
    } else {
        test_mask(&lines, &ext)
    };
    let priced_lines: Vec<usize> = findings.iter().map(|f| f.line).collect();
    let fhint = file_hint(&lines);
    let funit = file_unit(findings);
    let mut out: Vec<Fallback> = Vec::new();
    for i in 0..lines.len() {
        let line = lines[i];
        if line.len() > 2000 || skip.as_ref().is_some_and(|m| m[i]) {
            continue;
        }
        let lower = line.to_lowercase();
        // A line that names a model is a row of a table, not a default.
        let names_model = || !models_on_line(index, line).is_empty();
        if skips_unknown(&lines, i) {
            out.push(Fallback {
                file: file.to_string(),
                line: i + 1,
                kind: "skip-unknown",
                input: 0.0,
                output: 0.0,
                snippet: line.trim().chars().take(160).collect(),
            });
            continue;
        }
        // (kind, column where the price expression starts, zero must end the line)
        let mut anchor: Option<(&'static str, usize, bool)> = None;
        if let Some(m) = re!(r"^\s*(?:_\s*=>|default\s*:|else\s*->)").find(line) {
            if priced_above(&lines, i, &priced_lines) {
                anchor = Some(("match-default", m.end(), true));
            }
        }
        if anchor.is_none() {
            if let Some(c) = re!(r#"^\s*["'](?:default|fallback|unknown|\*|_default|__default__|_fallback)["']\s*(?::|=>)"#).find(line) {
                if priced_near(i, &priced_lines) {
                    anchor = Some(("default-key", c.end(), true));
                }
            }
        }
        if anchor.is_none() {
            if let Some(c) = re!(r#"(?i)^\s*(?:[\w()]+\s+){0,4}["']?(\w*(?:default|fallback)\w*)["']?\s*(?:\([^)]*\)\s*(?:->[^{=]*)?)?(?::[^=\n{]{0,80}?)?\s*(?:=[^=>]|=$|:|\{|->)"#)
                .captures(line)
            {
                let name = c[1].to_lowercase();
                if re!(r"pric|cost|rate|tariff").is_match(&name)
                    && !re!(r"limit|budget|max|currency|format|ratio|multiplier").is_match(&name)
                    && !names_model()
                {
                    anchor = Some(("named-default", c.get(1).expect("group").end(), false));
                }
            }
        }
        if anchor.is_none() && !names_model() {
            // Operators inside string literals ("free or paid") are not code.
            let code = blank_strings(line);
            if let Some(m) = re!(r#"(?:\?\?|\|\||\.unwrap_or(?:_else)?\(\s*(?:\|\|\s*)?|\.get\(\s*[\w.\[\]"' ]+\s*,|getOrDefault\(\s*[\w.\[\]"' ]+\s*,|\.get_or\(|\bor\s)"#)
                .find(&code)
            {
                if priced_near(i, &priced_lines) {
                    anchor = Some(("inline-default", m.end(), false));
                }
            }
        }
        let Some((kind, col, whole_line)) = anchor else {
            continue;
        };
        let rest = &line[col..];
        let snippet: String = line.trim().chars().take(160).collect();
        let worded = re!(r"pric|cost|rate").is_match(&lower);
        if (kind == "match-default" || (kind == "inline-default" && worded))
            && zero_value(rest, whole_line)
        {
            out.push(Fallback {
                file: file.to_string(),
                line: i + 1,
                kind,
                input: 0.0,
                output: 0.0,
                snippet,
            });
            continue;
        }
        let ex = extract(index, &lines, i, col);
        let (Some((vi, ui)), Some((vo, uo))) = (ex.input, ex.output) else {
            continue;
        };
        if vi.is_nan() || vo.is_nan() || vi < 0.0 || vo < 0.0 {
            continue;
        }
        let block_hint = unit_hint(&lines[i.saturating_sub(3)..=i].join("\n"));
        let guess = |v: f64, u: Option<Unit>| -> Unit {
            u.or(block_hint)
                .or(funit)
                .or(fhint)
                .unwrap_or(if v > 0.0 && v < 1e-4 {
                    Unit::T
                } else {
                    Unit::M
                })
        };
        let input = vi * guess(vi, ui).factor();
        let output = vo * guess(vo, uo).factor();
        if input > 1000.0 || output > 5000.0 {
            continue;
        }
        if out.iter().any(|f| f.line == i + 1) {
            continue;
        }
        out.push(Fallback {
            file: file.to_string(),
            line: i + 1,
            kind,
            input,
            output,
            snippet,
        });
    }
    out
}
