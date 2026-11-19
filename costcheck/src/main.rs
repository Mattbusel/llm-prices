//! `llm-costcheck`: fails CI when a codebase's LLM cost tracking is wrong.

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use llm_costcheck::check::{pct, usd};
use llm_costcheck::files::{list_files, split_list};
use llm_costcheck::scan::{is_test_file, scannable};
use llm_costcheck::{check, Index, Issue, Kind, Opts, Report, Table};
use serde_json::json;

const HELP: &str = "llm-costcheck: your LLM cost tracking is wrong, and nothing tells you.

Finds hard-coded model prices that differ from the provider pages, default prices silently
used for unknown models, and models priced without their cache-read price. Prints the error
in percent and dollars with file:line, and exits 1 so CI fails.

USAGE:
    llm-costcheck [DIR] [OPTIONS]

OPTIONS:
    --paths <globs>        Globs to scan, comma separated (default: **)
    --exclude <globs>      Globs to skip (node_modules, target, dist, build, vendor, .git always skipped)
    --providers <list>     anthropic,openai,google,deepseek (default), any of the table's providers, or all
    --fail-on <list>       Issue kinds that fail: drift,fallback,cache,missing or never
                           (default: drift,fallback,cache)
    --tolerance <frac>     Relative difference allowed, 0.01 = 1% (default: 0.01)
    --include-tests        Also scan test files and Rust #[cfg(test)] modules
    --prices <file>        Compare against this prices.json instead of the embedded table
    --format <fmt>         text (default), json or github (annotations + step summary)
    --all                  Also list every hard-coded price that is correct
    --max-bytes <n>        Skip files larger than this (default: 16000000)
    -h, --help             Show this help
    -V, --version          Show the version

Every input can also come from an INPUT_<NAME> environment variable (GitHub Actions).
Exit codes: 0 clean, 1 issues of a failing kind, 2 usage or I/O error.";

struct Args {
    dir: PathBuf,
    paths: Vec<String>,
    exclude: Vec<String>,
    providers: Vec<String>,
    fail_on: HashSet<Kind>,
    tolerance: f64,
    include_tests: bool,
    prices: Option<PathBuf>,
    format: String,
    all: bool,
    max_bytes: u64,
}

fn env_input(name: &str) -> Option<String> {
    let key = format!("INPUT_{}", name.to_uppercase().replace('-', "_"));
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_lowercase().as_str(), "true" | "1" | "yes")
}

fn parse_args() -> Result<Option<Args>, String> {
    let mut flags: Vec<(String, Option<String>)> = Vec::new();
    let mut dir = None;
    let mut it = std::env::args().skip(1).peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => {
                println!("{HELP}");
                return Ok(None);
            }
            "-V" | "--version" => {
                println!(
                    "llm-costcheck {} (prices {})",
                    env!("CARGO_PKG_VERSION"),
                    llm_prices::GENERATED
                );
                return Ok(None);
            }
            "--include-tests" | "--all" => flags.push((a[2..].to_string(), Some("true".into()))),
            s if s.starts_with("--") => {
                let (k, v) = match s.split_once('=') {
                    Some((k, v)) => (k[2..].to_string(), v.to_string()),
                    None => (
                        s[2..].to_string(),
                        it.next().ok_or(format!("{s} needs a value"))?,
                    ),
                };
                flags.push((k, Some(v)));
            }
            s => {
                if dir.is_some() {
                    return Err(format!("unexpected argument {s}"));
                }
                dir = Some(PathBuf::from(s));
            }
        }
    }
    let get = |name: &str| -> Option<String> {
        flags
            .iter()
            .rev()
            .find(|(k, _)| k == name)
            .and_then(|(_, v)| v.clone())
            .or_else(|| env_input(name))
    };
    for (k, _) in &flags {
        if !matches!(
            k.as_str(),
            "paths"
                | "exclude"
                | "providers"
                | "fail-on"
                | "tolerance"
                | "include-tests"
                | "prices"
                | "format"
                | "all"
                | "max-bytes"
                | "working-directory"
        ) {
            return Err(format!("unknown option --{k} (see --help)"));
        }
    }
    let dir = dir
        .or_else(|| get("working-directory").map(PathBuf::from))
        .or_else(|| std::env::var("GITHUB_WORKSPACE").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    let mut fail_on = HashSet::new();
    for k in split_list(&get("fail-on").unwrap_or_else(|| "drift,fallback,cache".into())) {
        let k = k.to_lowercase();
        if k == "never" {
            fail_on.clear();
            break;
        }
        fail_on.insert(Kind::parse(&k).ok_or(format!(
            "--fail-on takes drift, fallback, cache, missing or never (got {k})"
        ))?);
    }
    let tolerance: f64 = get("tolerance")
        .unwrap_or_else(|| "0.01".into())
        .parse()
        .map_err(|_| "--tolerance must be a number such as 0.01")?;
    if tolerance.is_nan() || tolerance < 0.0 {
        return Err("--tolerance must be >= 0".into());
    }
    let format = get("format").unwrap_or_else(|| {
        if std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true") {
            "github".into()
        } else {
            "text".into()
        }
    });
    if !matches!(format.as_str(), "text" | "json" | "github") {
        return Err(format!(
            "--format takes text, json or github (got {format})"
        ));
    }
    Ok(Some(Args {
        dir,
        paths: split_list(&get("paths").unwrap_or_else(|| "**".into())),
        exclude: split_list(&get("exclude").unwrap_or_default()),
        providers: split_list(&get("providers").unwrap_or_default())
            .into_iter()
            .map(|p| p.to_lowercase())
            .collect(),
        fail_on,
        tolerance,
        include_tests: get("include-tests").is_some_and(|v| truthy(&v)),
        prices: get("prices").map(PathBuf::from),
        format,
        all: get("all")
            .or_else(|| get("show-matches"))
            .is_some_and(|v| truthy(&v)),
        max_bytes: get("max-bytes")
            .and_then(|v| v.parse().ok())
            .unwrap_or(16_000_000),
    }))
}

fn issue_json(i: &Issue) -> serde_json::Value {
    json!({
        "kind": i.kind.name(),
        "file": i.file,
        "line": i.line,
        "model": i.model,
        "provider": i.provider,
        "error": i.error,
        "call": i.call,
        "booked_usd": i.booked,
        "real_usd": i.real,
        "headline": i.headline(),
        "detail": i.detail,
        "source_url": i.source_url,
        "checked": i.checked,
        "affected_models": i.affected,
    })
}

fn render_text(rep: &Report, table: &Table, index: &Index, all: bool) -> String {
    let mut out = Vec::new();
    out.push(format!(
        "llm-costcheck: scanned {} files, {} hard-coded prices for {} known models, {} default price(s) (prices table {}).",
        rep.files_scanned,
        rep.findings.len(),
        rep.priced.len(),
        rep.fallbacks.len(),
        table.generated
    ));
    let real: Vec<&Issue> = rep
        .issues
        .iter()
        .filter(|i| i.kind != Kind::Missing)
        .collect();
    if real.is_empty() {
        out.push(String::new());
        out.push("No issues: every hard-coded price found matches the provider pages, no default price for unknown models, and cache reads are priced.".into());
    }
    for kind in [Kind::Drift, Kind::Fallback, Kind::Cache, Kind::Missing] {
        let list: Vec<&Issue> = rep.issues.iter().filter(|i| i.kind == kind).collect();
        if list.is_empty() {
            continue;
        }
        out.push(String::new());
        out.push(match kind {
            Kind::Drift => format!("{} price(s) differ from the provider's page:", list.len()),
            Kind::Fallback => format!("{} default price(s) for unknown models:", list.len()),
            Kind::Cache => format!("{} model(s) priced without a cache-read price:", list.len()),
            Kind::Missing => format!(
                "{} model(s) used but priced nowhere in the scanned files:",
                list.len()
            ),
        });
        for i in list {
            out.push(format!("  {}:{}: {}", i.file, i.line, i.headline()));
            out.push(format!("      {}", i.detail));
            if !i.source_url.is_empty() {
                out.push(format!(
                    "      source: {} (checked {})",
                    i.source_url, i.checked
                ));
            }
        }
    }
    if all {
        let ok: Vec<_> = rep.findings.iter().filter(|f| f.ok).collect();
        if !ok.is_empty() {
            out.push(String::new());
            out.push("Correct prices:".into());
            for f in ok {
                out.push(format!(
                    "  {}:{}: {} {} {} {}",
                    f.file,
                    f.line,
                    index.row(f.row).model,
                    f.field.name(),
                    f.written,
                    f.unit.label()
                ));
            }
        }
    }
    out.join("\n")
}

fn escape_annotation(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn github(rep: &Report, fail_on: &HashSet<Kind>) {
    for i in &rep.issues {
        let level = if fail_on.contains(&i.kind) {
            "error"
        } else {
            "warning"
        };
        let title = match i.kind {
            Kind::Drift => "LLM price drift",
            Kind::Fallback => "Default price for unknown models",
            Kind::Cache => "No cache-read price",
            Kind::Missing => "Model without a price",
        };
        println!(
            "::{level} file={},line={},title={title}::{}",
            i.file,
            i.line,
            escape_annotation(&format!(
                "{}: {} Source: {} (checked {})",
                i.headline(),
                i.detail,
                i.source_url,
                i.checked
            ))
        );
    }
    if let Ok(path) = std::env::var("GITHUB_STEP_SUMMARY") {
        let mut md = vec![
            "### llm-costcheck".to_string(),
            String::new(),
            format!(
                "Scanned {} files: {} hard-coded prices, {} wrong, {} default price(s), {} model(s) without a cache-read price.",
                rep.files_scanned,
                rep.findings.len(),
                rep.count(Kind::Drift),
                rep.count(Kind::Fallback),
                rep.count(Kind::Cache)
            ),
            String::new(),
        ];
        let shown: Vec<&Issue> = rep
            .issues
            .iter()
            .filter(|i| i.kind != Kind::Missing)
            .collect();
        if !shown.is_empty() {
            md.push("| Where | Kind | Model | Error | Booked | Real | Call |".into());
            md.push("| --- | --- | --- | --- | --- | --- | --- |".into());
            for i in shown {
                md.push(format!(
                    "| `{}:{}` | {} | `{}` | {} | {} | {} | {} |",
                    i.file,
                    i.line,
                    i.kind.name(),
                    i.model,
                    i.error.map(pct).unwrap_or_default(),
                    i.booked.map(usd).unwrap_or_default(),
                    i.real.map(usd).unwrap_or_default(),
                    i.call
                ));
            }
        }
        if let Ok(mut f) = fs::OpenOptions::new().append(true).create(true).open(path) {
            let _ = writeln!(f, "{}", md.join("\n"));
        }
    }
    if let Ok(path) = std::env::var("GITHUB_OUTPUT") {
        if let Ok(mut f) = fs::OpenOptions::new().append(true).create(true).open(path) {
            let worst = rep
                .issues
                .iter()
                .filter_map(|i| i.error)
                .fold(0.0f64, |a, e| if e.abs() > a.abs() { e } else { a });
            let _ = writeln!(
                f,
                "drift-count={}\nfallback-count={}\ncache-count={}\nmissing-count={}\nprices-found={}\nworst-error={}",
                rep.count(Kind::Drift),
                rep.count(Kind::Fallback),
                rep.count(Kind::Cache),
                rep.count(Kind::Missing),
                rep.findings.len(),
                pct(worst)
            );
        }
    }
}

fn run() -> Result<u8, String> {
    let Some(args) = parse_args()? else {
        return Ok(0);
    };
    let table = match &args.prices {
        Some(p) => Table::from_json(
            &fs::read_to_string(p).map_err(|e| format!("cannot read {}: {e}", p.display()))?,
        )?,
        None => Table::embedded(),
    };
    let index = Index::new(&table, &args.providers);
    let root = &args.dir;
    if !root.is_dir() {
        return Err(format!("{} is not a directory", root.display()));
    }
    let files: Vec<String> = list_files(root, &args.paths, &args.exclude, args.max_bytes)
        .into_iter()
        .filter(|f| scannable(f) && (args.include_tests || !is_test_file(f)))
        .collect();
    let texts = files.into_iter().filter_map(|f| {
        let bytes = fs::read(root.join(&f)).ok()?;
        if bytes.contains(&0) {
            return None;
        }
        Some((f, String::from_utf8_lossy(&bytes).into_owned()))
    });
    let opts = Opts {
        tolerance: args.tolerance,
        include_tests: args.include_tests,
    };
    let rep = check(texts, &index, opts);
    match args.format.as_str() {
        "json" => {
            let v = json!({
                "tool": "llm-costcheck",
                "version": env!("CARGO_PKG_VERSION"),
                "prices_generated": table.generated,
                "files_scanned": rep.files_scanned,
                "prices_found": rep.findings.len(),
                "prices_wrong": rep.findings.iter().filter(|f| !f.ok).count(),
                "models_priced": rep.priced.len(),
                "cache_priced_by_ratio": rep.cache_handled_globally,
                "issues": rep.issues.iter().map(issue_json).collect::<Vec<_>>(),
                "prices": rep.findings.iter().map(|f| json!({
                    "file": f.file, "line": f.line, "model": index.row(f.row).model, "field": f.field.name(),
                    "written": f.written, "unit": f.unit.label(), "per_1m": f.per_m, "expected_per_1m": f.expected,
                    "ok": f.ok, "long_context": f.tier,
                })).collect::<Vec<_>>(),
                "fallbacks": rep.fallbacks.iter().map(|f| json!({
                    "file": f.file, "line": f.line, "kind": f.kind, "input_per_1m": f.input, "output_per_1m": f.output, "snippet": f.snippet,
                })).collect::<Vec<_>>(),
            });
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        }
        "github" => {
            println!("{}", render_text(&rep, &table, &index, args.all));
            github(&rep, &args.fail_on);
        }
        _ => println!("{}", render_text(&rep, &table, &index, args.all)),
    }
    let failing = rep.issues.iter().any(|i| args.fail_on.contains(&i.kind));
    Ok(u8::from(failing))
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("llm-costcheck: {e}");
            ExitCode::from(2)
        }
    }
}
