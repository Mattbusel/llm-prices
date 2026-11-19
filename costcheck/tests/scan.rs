use std::fs;
use std::path::Path;

use llm_costcheck::files::glob_to_regex;
use llm_costcheck::scan::{
    classify_key, is_test_file, judge, scan_text, scannable, unit_hint, Opts, Unit,
};
use llm_costcheck::{check, Field, Index, Kind, Table};

fn table() -> &'static Table {
    static T: std::sync::OnceLock<Table> = std::sync::OnceLock::new();
    T.get_or_init(Table::embedded)
}

fn index() -> Index<'static> {
    Index::new(table(), &[])
}

fn fixture(name: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
    )
    .unwrap()
}

fn scan(name: &str, include_tests: bool) -> llm_costcheck::scan::FileScan {
    scan_text(
        name,
        &fixture(name),
        &index(),
        Opts {
            tolerance: 0.01,
            include_tests,
        },
    )
}

fn drift(name: &str) -> Vec<String> {
    let idx = index();
    let mut v: Vec<String> = scan(name, false)
        .findings
        .iter()
        .filter(|f| !f.ok)
        .map(|f| format!("{}:{}:{}", f.line, idx.row(f.row).model, f.field.name()))
        .collect();
    v.sort();
    v
}

fn priced(name: &str) -> Vec<String> {
    let idx = index();
    scan(name, false)
        .priced
        .iter()
        .map(|&r| idx.row(r).model.clone())
        .collect()
}

fn row(model: &str) -> &'static llm_costcheck::Row {
    table().rows.iter().find(|r| r.model == model).unwrap()
}

#[test]
fn model_ids_resolve_across_prefixes_and_dates() {
    let idx = index();
    let m = |t: &str| idx.resolve(t).map(|i| idx.row(i).model.clone());
    assert_eq!(m("claude-opus-5-5").as_deref(), Some("claude-opus-5-5"));
    assert_eq!(
        m("anthropic/claude-opus-5-5").as_deref(),
        Some("claude-opus-5-5")
    );
    assert_eq!(m("us.anthropic.claude-sonnet-4-20250514-v1:0"), None);
    assert_eq!(
        m("us.anthropic.claude-sonnet-4-20250514-v2:0").as_deref(),
        Some("claude-sonnet-4-20250514")
    );
    assert_eq!(
        m("claude-opus-4-1@20250805").as_deref(),
        Some("claude-opus-4-1-20250805")
    );
    assert_eq!(
        m("claude-sonnet-4-5").as_deref(),
        Some("claude-sonnet-4-5-20250929")
    );
    assert_eq!(
        m("claude-haiku-4-5-20251001").as_deref(),
        Some("claude-haiku-4-5")
    );
    assert_eq!(
        m("models/gemini-2.5-pro").as_deref(),
        Some("gemini-2.5-pro")
    );
    assert_eq!(m("deepseek-v4-flash").as_deref(), Some("deepseek-flash"));
    assert_eq!(m("gpt-4o-2024-08-06"), None);
    assert_eq!(m("gpt-4o-2024-05-13").as_deref(), Some("gpt-4o-2024-05-13"));
    assert_eq!(m("claude-3-opus-20240229"), None);
    let with_bedrock = Index::new(table(), &["anthropic".into(), "bedrock".into()]);
    let b = with_bedrock
        .resolve("us.anthropic.claude-sonnet-4-5-20250929-v1:0")
        .unwrap();
    assert_eq!(with_bedrock.row(b).std.input, Some(3.3));
}

#[test]
fn provider_filter_limits_the_index() {
    let only = Index::new(table(), &["openai".into()]);
    assert!(only.resolve("claude-opus-5-5").is_none());
    assert!(only.resolve("gpt-4o").is_some());
    assert!(index().resolve("mistral-large-4").is_none());
    let hosts = Index::new(table(), &["groq".into(), "together".into()]);
    assert!(
        hosts.resolve("openai/gpt-oss-120b").is_none(),
        "sold by both, so no single price applies"
    );
}

#[test]
fn key_classification() {
    assert_eq!(classify_key("input_per_mtok"), Some(Field::Input));
    assert_eq!(classify_key("inputPerMillion"), Some(Field::Input));
    assert_eq!(classify_key("prompt"), Some(Field::Input));
    assert_eq!(classify_key("completion"), Some(Field::Output));
    assert_eq!(classify_key("output_cost_per_token"), Some(Field::Output));
    assert_eq!(
        classify_key("cache_read_input_token_cost"),
        Some(Field::CacheRead)
    );
    assert_eq!(
        classify_key("cache_creation_input_token_cost"),
        Some(Field::CacheWrite)
    );
    assert_eq!(classify_key("cached_input"), Some(Field::CacheRead));
    assert_eq!(classify_key("Cached"), Some(Field::CacheRead));
    assert_eq!(classify_key("cache_write_1h"), Some(Field::CacheWrite));
    assert_eq!(classify_key("batch_input"), Some(Field::BatchInput));
    for k in [
        "input_tokens",
        "max_input_tokens",
        "contextWindow",
        "temperature",
        "cacheWriteMultiplier",
        "batch_discount",
        "outputPriceRatio",
    ] {
        assert_eq!(classify_key(k), None, "{k}");
    }
}

#[test]
fn unit_hints() {
    assert_eq!(unit_hint("input_cost_per_token"), Some(Unit::T));
    assert_eq!(unit_hint("# Prices in USD per 1K tokens."), Some(Unit::K));
    assert_eq!(unit_hint("price_per_1k_input"), Some(Unit::K));
    assert_eq!(unit_hint("input_per_mtok"), Some(Unit::M));
    assert_eq!(unit_hint("// USD per million tokens"), Some(Unit::M));
    assert_eq!(unit_hint("per_1_000_000"), Some(Unit::M));
    assert_eq!(unit_hint("input"), None);
}

#[test]
fn judge_reads_every_unit_and_ignores_non_prices() {
    let r = row("gpt-4o");
    assert!(judge(r, Field::Input, 2.5, None, 0.01, None).unwrap().ok);
    assert!(judge(r, Field::Input, 0.0025, None, 0.01, None).unwrap().ok);
    assert!(judge(r, Field::Input, 2.5e-6, None, 0.01, None).unwrap().ok);
    let bad = judge(r, Field::Input, 0.005, Some(Unit::K), 0.01, None).unwrap();
    assert!(!bad.ok);
    assert_eq!(bad.per_m, 5.0);
    assert!(judge(r, Field::Input, 128000.0, None, 0.01, None).is_none());
    assert!(judge(r, Field::Input, 0.0, None, 0.01, None).is_none());
}

#[test]
fn judge_accepts_batch_tiers_and_regional_prices() {
    let opus = row("claude-opus-5-5");
    assert!(
        judge(opus, Field::Input, 2.0, Some(Unit::M), 0.01, None)
            .unwrap()
            .ok
    ); // batch
    assert!(
        judge(opus, Field::Input, 8.0, Some(Unit::M), 0.01, None)
            .unwrap()
            .ok
    ); // fast
    assert!(
        judge(opus, Field::Input, 4.4, Some(Unit::M), 0.01, None)
            .unwrap()
            .ok
    ); // US-only 1.1x
    assert!(
        judge(
            row("deepseek-flash"),
            Field::Input,
            0.15,
            Some(Unit::M),
            0.01,
            None
        )
        .unwrap()
        .ok
    ); // off-peak
    assert!(
        !judge(
            row("o4-mini"),
            Field::CacheRead,
            0.55,
            Some(Unit::M),
            0.01,
            None
        )
        .unwrap()
        .ok
    );
    let sonnet = row("claude-sonnet-5-5");
    assert!(
        !judge(sonnet, Field::Input, 2.03, Some(Unit::M), 0.01, None)
            .unwrap()
            .ok
    );
    assert!(
        judge(sonnet, Field::Input, 2.03, Some(Unit::M), 0.02, None)
            .unwrap()
            .ok
    );
}

#[test]
fn scheduled_prices_from_a_prices_file() {
    let json = r#"{"generated":"x","models":[{"provider":"google","model":"gemini-4-argon","aliases":[],"status":"listed",
      "input":2,"output":10,"cache_read":0.1,"cache_write":null,"long_context":null,"other_tiers":{},
      "scheduled":[{"effective":"2026-12-01","input":4,"output":20}],"source_url":"u","checked":"d"}]}"#;
    let t = Table::from_json(json).unwrap();
    assert!(
        judge(&t.rows[0], Field::Input, 4.0, Some(Unit::M), 0.01, None)
            .unwrap()
            .ok
    );
}

#[test]
fn rust_match_arms_struct_literals_and_tuples() {
    assert_eq!(
        drift("rust_match.rs"),
        [
            "14:claude-sonnet-5-5:cache_read",
            "38:claude-sonnet-5:input",
            "38:claude-sonnet-5:output"
        ]
    );
    let p = priced("rust_match.rs");
    assert!(p.contains(&"claude-opus-4-1-20250805".to_string()));
    assert!(p.contains(&"claude-haiku-4-5".to_string()));
}

#[test]
fn python_dict_in_per_1k_units() {
    assert_eq!(drift("pricing.py"), ["6:gpt-5:output"]);
    let idx = index();
    let r = scan("pricing.py", false);
    let f = r
        .findings
        .iter()
        .find(|f| idx.row(f.row).model == "gpt-5" && f.field == Field::Output)
        .unwrap();
    assert_eq!(f.unit, Unit::K);
    assert!((f.per_m - 12.0).abs() < 1e-9);
    assert_eq!(f.expected, 10.0);
}

#[test]
fn litellm_shaped_json() {
    assert_eq!(drift("litellm_like.json"), ["22:deepseek-flash:input"]);
    let r = scan("litellm_like.json", false);
    assert!(r.findings.iter().all(|f| f.unit == Unit::T));
    assert!(!r
        .findings
        .iter()
        .any(|f| f.written == "200000" || f.written == "64000"));
}

#[test]
fn yaml_toml_ts_go_and_divided_literals() {
    assert_eq!(
        drift("prices.yaml"),
        ["12:gemini-2.5-flash:output", "20:gpt-6-luna:output"]
    );
    assert!(priced("prices.yaml").contains(&"gpt-6.1-sol".to_string()));
    assert_eq!(drift("models.toml"), ["12:claude-fable-5-1:cache_read"]);
    let p = priced("models.toml");
    for m in ["gpt-5.4-mini", "gemini-2.5-flash-lite", "claude-opus-4-8"] {
        assert!(p.contains(&m.to_string()), "{m}");
    }
    assert_eq!(drift("costs.ts"), ["21:gpt-5.6-terra:output"]);
    assert_eq!(drift("pricing.go"), ["13:o4-mini:cache_read"]);
    assert_eq!(drift("divide.rs"), ["7:claude-opus-5:output"]);
    let r = scan("divide.rs", false);
    assert_eq!(
        r.findings.iter().find(|f| f.line == 6).unwrap().unit,
        Unit::M
    );
}

#[test]
fn no_false_positives_on_limits_usage_and_settings() {
    let r = scan("noise.ts", false);
    assert!(r.findings.is_empty(), "{:?}", r.findings);
    assert!(!r.seen.is_empty());
    assert!(scan("noise_calls.rs", false).findings.is_empty());
}

#[test]
fn every_stale_marker_is_flagged_and_nothing_else() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for e in fs::read_dir(dir).unwrap() {
        let name = e.unwrap().file_name().to_string_lossy().into_owned();
        if name.ends_with(".json") || name.starts_with("fallback") || name.starts_with("cache") {
            continue;
        }
        let text = fixture(&name);
        let mut flagged: Vec<usize> = scan(&name, false)
            .findings
            .iter()
            .filter(|f| !f.ok)
            .map(|f| f.line)
            .collect();
        flagged.sort();
        flagged.dedup();
        let marked: Vec<usize> = text
            .lines()
            .enumerate()
            .filter(|(_, l)| l.contains("stale"))
            .map(|(i, _)| i + 1)
            .collect();
        assert_eq!(flagged, marked, "{name}");
    }
}

#[test]
fn rust_test_modules_are_skipped_unless_asked() {
    let idx = index();
    let on = scan("noise_calls.rs", true);
    let mut got: Vec<String> = on
        .findings
        .iter()
        .filter(|f| !f.ok)
        .map(|f| format!("{}:{}:{}", f.line, idx.row(f.row).model, f.field.name()))
        .collect();
    got.sort();
    assert_eq!(
        got,
        [
            "15:gpt-4o:input",
            "15:gpt-4o:output",
            "16:claude-sonnet-5-5:input",
            "16:claude-sonnet-5-5:output"
        ]
    );
    let r = scan("rust_raw_string_tests.rs", false);
    assert!(r.findings.iter().all(|f| f.ok));
    assert!(scan("rust_raw_string_tests.rs", true)
        .findings
        .iter()
        .any(|f| !f.ok));
}

#[test]
fn group_arms_check_each_model() {
    assert_eq!(
        drift("group_arms.rs"),
        ["9:claude-opus-5-5:input", "9:claude-opus-5-5:output"]
    );
    let p = priced("group_arms.rs");
    for m in [
        "claude-opus-4-7",
        "claude-opus-4-6",
        "claude-opus-4-5",
        "claude-opus-5",
    ] {
        assert!(p.contains(&m.to_string()), "{m}");
    }
    assert!(drift("group_case.go").is_empty());
}

#[test]
fn threshold_tiers_compare_with_long_context_prices() {
    let ts = [
        "export const tieredPricing = {",
        "  'gpt-5.4': { threshold: 272000, prompt: 5, completion: 22.5 },",
        "  'claude-haiku-5-5': { threshold: 100000, prompt: 0.5, completion: 2.5 },",
        "  'gpt-4o': { threshold: 128000, prompt: 5, completion: 15 },",
        "  'gpt-6-sol': { threshold: 272000, prompt: 4, completion: 10 },",
        "};",
        "export const multi = {",
        "  'gemini-2.5-pro': {",
        "    prompt: 1.25,",
        "    completion: 10,",
        "    tiers: [",
        "      { threshold: 200000, prompt: 2.5, completion: 10 },",
        "    ],",
        "  },",
        "};",
    ]
    .join("\n");
    let idx = index();
    let r = scan_text("tx.ts", &ts, &idx, Opts::default());
    let mut got: Vec<String> = r
        .findings
        .iter()
        .map(|f| {
            let v = if f.ok {
                "ok".to_string()
            } else {
                format!("drift({})", f.expected)
            };
            format!(
                "{}:{}:{}:{}:{}",
                f.line,
                idx.row(f.row).model,
                f.field.name(),
                f.written,
                v
            )
        })
        .collect();
    got.sort();
    let mut want = vec![
        "12:gemini-2.5-pro:input:2.5:ok",
        "12:gemini-2.5-pro:output:10:drift(15)",
        "2:gpt-5.4:input:5:ok",
        "2:gpt-5.4:output:22.5:ok",
        "3:claude-haiku-5-5:input:0.5:ok",
        "3:claude-haiku-5-5:output:2.5:ok",
        "5:gpt-6-sol:input:4:ok",
        "5:gpt-6-sol:output:10:drift(15)",
        "9:gemini-2.5-pro:input:1.25:ok",
        "10:gemini-2.5-pro:output:10:ok",
    ];
    want.sort();
    assert_eq!(got, want);
}

#[test]
fn ratio_keys_are_not_prices() {
    let ts = "export const pricing = {\n  'claude-sonnet-4-6': { input: 3, output: 15, cacheWriteMultiplier: 1.25, cacheReadMultiplier: 0.1 },\n};";
    let r = scan_text("pricing.ts", ts, &index(), Opts::default());
    let mut got: Vec<String> = r
        .findings
        .iter()
        .map(|f| format!("{}:{}:{}", f.field.name(), f.written, f.ok))
        .collect();
    got.sort();
    assert_eq!(got, ["input:3:true", "output:15:true"]);
}

#[test]
fn file_filters() {
    assert!(scannable("src/pricing.rs"));
    assert!(scannable("config/models.yaml"));
    assert!(!scannable("package-lock.json"));
    assert!(!scannable("README.md"));
    assert!(glob_to_regex("**").is_match("a/b/c.py"));
    assert!(glob_to_regex("src/**").is_match("src/x/y.ts"));
    assert!(!glob_to_regex("src/**").is_match("lib/x.ts"));
    assert!(glob_to_regex("**/*.{py,ts}").is_match("a/b.ts"));
    assert!(glob_to_regex("**/*.py").is_match("top.py"));
    for f in [
        "tests/a.rs",
        "src/__tests__/p.ts",
        "pkg/price_test.go",
        "test_costs.py",
        "web/cost.spec.ts",
        "a/conftest.py",
    ] {
        assert!(is_test_file(f), "{f}");
    }
    for f in ["src/pricing.rs", "lib/latest.py", "src/contest.ts"] {
        assert!(!is_test_file(f), "{f}");
    }
}

fn run(files: &[(&str, &str)]) -> llm_costcheck::Report {
    let idx = index();
    check(
        files.iter().map(|(a, b)| (a.to_string(), b.to_string())),
        &idx,
        Opts::default(),
    )
}

#[test]
fn drift_issue_carries_percent_and_dollars() {
    let rep = run(&[("pricing.py", "PRICES = {\n    \"claude-opus-5-5\": {\"input\": 15.0, \"output\": 75.0, \"cache_read\": 0.2},\n}\n")]);
    let d: Vec<_> = rep
        .issues
        .iter()
        .filter(|i| i.kind == Kind::Drift)
        .collect();
    assert_eq!(d.len(), 1);
    let i = d[0];
    assert_eq!((i.file.as_str(), i.line), ("pricing.py", 2));
    assert_eq!(
        i.headline(),
        "your cost tracking is wrong by +275.0% for claude-opus-5-5"
    );
    assert!((i.booked.unwrap() - 1.875).abs() < 1e-9);
    assert!((i.real.unwrap() - 0.5).abs() < 1e-9);
    assert!(
        i.detail.contains("booked $1.88, real $0.500"),
        "{}",
        i.detail
    );
}

#[test]
fn cache_read_drift_uses_the_cached_call() {
    let rep = run(&[("p.ts", "export const P = {\n  'claude-sonnet-5-5': { input: 2, output: 10, cacheRead: 0.2 },\n};\n")]);
    let i = rep.issues.iter().find(|i| i.kind == Kind::Drift).unwrap();
    assert_eq!(i.call, llm_costcheck::check::CACHED_CALL);
    // 40K x 2 + 60K x 0.2 + 5K x 10 = 0.142 booked; 40K x 2 + 60K x 0.1 + 5K x 10 = 0.136 real.
    assert!((i.booked.unwrap() - 0.142).abs() < 1e-9);
    assert!((i.real.unwrap() - 0.136).abs() < 1e-9);
}

#[test]
fn fallback_shapes_are_found() {
    let rep = run(&[("fallback_match.rs", &fixture("fallback_match.rs"))]);
    let fb: Vec<_> = rep
        .fallbacks
        .iter()
        .map(|f| (f.line, f.kind, f.input, f.output))
        .collect();
    assert_eq!(
        fb,
        [
            (8, "match-default", 1.0, 3.0),
            (17, "match-default", 0.0, 0.0)
        ]
    );
    let i = rep
        .issues
        .iter()
        .find(|i| i.kind == Kind::Fallback && i.line == 8)
        .unwrap();
    // claude-opus-5-5 is not in the table: 100K x 1 + 5K x 3 = $0.115 booked, $0.50 real.
    assert_eq!(i.model, "claude-opus-5-5");
    assert_eq!(
        i.headline(),
        "your cost tracking is wrong by -77.0% for claude-opus-5-5"
    );
    assert!(i.affected > 5);
    let z = rep
        .issues
        .iter()
        .find(|i| i.kind == Kind::Fallback && i.line == 17)
        .unwrap();
    assert_eq!(z.error, Some(-1.0));

    let rep = run(&[("fallback.py", &fixture("fallback.py"))]);
    let mut fb: Vec<_> = rep
        .fallbacks
        .iter()
        .map(|f| (f.line, f.kind, f.input, f.output))
        .collect();
    fb.sort_by_key(|f| f.0);
    assert_eq!(
        fb,
        [
            (9, "named-default", 1.0, 3.0),
            (13, "default-key", 2.5, 10.0),
            (18, "inline-default", 0.5, 1.5),
            (22, "inline-default", 0.0, 0.0)
        ]
    );

    let rep = run(&[("fallback.ts", &fixture("fallback.ts"))]);
    let mut fb: Vec<_> = rep
        .fallbacks
        .iter()
        .map(|f| (f.line, f.kind, f.input, f.output))
        .collect();
    fb.sort_by_key(|f| f.0);
    assert_eq!(
        fb,
        [
            (7, "inline-default", 3.0, 15.0),
            (12, "match-default", 1.0, 2.0)
        ]
    );
}

#[test]
fn none_for_unknown_models_is_not_a_fallback() {
    let src = "fn price(m: &str) -> Option<(f64, f64)> {\n    match m {\n        \"gpt-4o\" => Some((2.5, 10.0)),\n        _ => None,\n    }\n}\n";
    let rep = run(&[("ok.rs", src)]);
    assert!(rep.fallbacks.is_empty());
    assert!(rep
        .issues
        .iter()
        .all(|i| i.kind != Kind::Drift && i.kind != Kind::Fallback));
}

#[test]
fn missing_cache_price_is_reported_unless_priced_by_ratio() {
    let rep = run(&[("cache.py", &fixture("cache.py"))]);
    let c: Vec<_> = rep
        .issues
        .iter()
        .filter(|i| i.kind == Kind::Cache)
        .map(|i| (i.line, i.model.as_str()))
        .collect();
    assert_eq!(c, [(3, "gpt-5.4")]);
    let i = rep.issues.iter().find(|i| i.kind == Kind::Cache).unwrap();
    // 100K x 2.5 + 5K x 15 = 0.325 booked; 40K x 2.5 + 60K x 0.25 + 5K x 15 = 0.19 real.
    assert!((i.booked.unwrap() - 0.325).abs() < 1e-9);
    assert!((i.real.unwrap() - 0.19).abs() < 1e-9);

    let rep = run(&[
        ("cache.py", &fixture("cache.py")),
        ("ratio.py", "CACHE_READ_MULTIPLIER = 0.1\n"),
    ]);
    assert!(rep.issues.iter().all(|i| i.kind != Kind::Cache));
}

#[test]
fn a_clean_table_has_no_issues() {
    let src = "PRICES = {\n    \"gpt-5.4\": {\"input\": 2.5, \"output\": 15, \"cached_input\": 0.25},\n    \"claude-sonnet-5-5\": {\"input\": 2, \"output\": 10, \"cache_read\": 0.1},\n}\n\ndef price(model):\n    return PRICES[model]\n";
    let rep = run(&[("clean.py", src)]);
    assert!(rep.issues.is_empty(), "{:?}", rep.issues);
    assert_eq!(rep.findings.len(), 6);
}

#[test]
fn skipping_unknown_models_is_a_zero_default() {
    let py = "class CostManager:\n    def update_cost(self, prompt_tokens, completion_tokens, model):\n        if model not in self.token_costs:\n            logger.warning(f\"Model {model} not found in TOKEN_COSTS.\")\n            return\n        self.total_cost += prompt_tokens * self.token_costs[model][\"prompt\"]\n";
    let ts = "export function cost(model: string, i: number, o: number) {\n  if (!PRICES[model]) return 0;\n  return i * PRICES[model].input;\n}\n";
    let rs = "pub fn cost(model: &str, i: u64) -> f64 {\n    let Some(p) = PRICING.get(model) else { return 0.0 };\n    p.input * i as f64\n}\n";
    for (name, src) in [("cost_manager.py", py), ("cost.ts", ts), ("cost.rs", rs)] {
        let rep = run(&[(name, src)]);
        let kinds: Vec<_> = rep
            .fallbacks
            .iter()
            .map(|f| (f.kind, f.input, f.output))
            .collect();
        assert_eq!(kinds, [("skip-unknown", 0.0, 0.0)], "{name}");
        let i = rep
            .issues
            .iter()
            .find(|i| i.kind == Kind::Fallback)
            .unwrap();
        assert_eq!(i.error, Some(-1.0), "{name}");
    }
    let strict = "def cost(model):\n    if model not in PRICES:\n        raise KeyError(model)\n    return PRICES[model]\n";
    assert!(run(&[("strict.py", strict)]).fallbacks.is_empty());
}

#[test]
fn display_names_credits_and_cache_minimums_are_not_prices() {
    let py = "CatalogModel(\n    slug=\"gpt-4.1-mini\",\n    display_name=\"GPT-4.1 Mini\",\n    cost=CatalogModelCost(\n        input_credits_per_1m=60.0,\n        output_credits_per_1m=240.0,\n        provider_input_usd_per_1m=0.40,\n        provider_output_usd_per_1m=1.60,\n    ),\n)\n";
    let rep = run(&[("catalog.py", py)]);
    assert!(rep.findings.iter().all(|f| f.ok), "{:?}", rep.findings);
    let json = "{\n  \"claude-haiku-5-5\": {\n    \"cache_creation_input_token_cost\": 1.25e-07,\n    \"cache_creation_input_token_cost_batches\": 6.25e-08,\n    \"cache_read_input_token_cost\": 1e-08,\n    \"input_cost_per_token\": 1e-07,\n    \"output_cost_per_token\": 5e-07,\n    \"prompt_cache_min_tokens\": 512\n  }\n}\n";
    let rep = run(&[("rates.json", json)]);
    assert!(
        rep.findings.iter().all(|f| f.ok && !f.tier),
        "{:?}",
        rep.findings
    );
    assert_eq!(rep.findings.len(), 4);
    let consts = "# OpenRouter cache-write price, USD per million tokens.\nCACHE_WRITE_USD_PER_M = 3.75\nCOUNT_MODEL = \"claude-sonnet-5\"\n";
    assert!(run(&[("prefix.py", consts)]).findings.is_empty());
}

#[test]
fn a_model_named_in_another_field_does_not_own_the_record() {
    let json = "{\n  \"azure/gpt-4.1\": {\n    \"successor_model\": \"azure/gpt-5.1\",\n    \"cache_read_input_token_cost\": 5e-07,\n    \"input_cost_per_token\": 2e-06,\n    \"output_cost_per_token\": 8e-06\n  }\n}\n";
    let rep = run(&[("prices.json", json)]);
    assert!(rep.findings.iter().all(|f| f.ok), "{:?}", rep.findings);
}

#[test]
fn host_paths_tile_counts_and_per_image_prices_are_skipped() {
    let json = "{\n  \"azure/gpt-5.4\": {\n    \"input_cost_per_token\": 9e-06,\n    \"output_cost_per_token\": 9e-05\n  },\n  \"vertex_ai/gemini-2.5-pro\": {\n    \"input_cost_per_token\": 9e-06\n  },\n  \"gemini-3.1-pro-preview\": {\n    \"input_cost_per_token\": 2e-06,\n    \"output_cost_per_image\": 0.00012,\n    \"cache_read_input_audio_token_cost\": 1e-07,\n    \"output_cost_per_token\": 1.2e-05\n  }\n}\n";
    let rep = run(&[("map.json", json)]);
    assert!(rep.findings.iter().all(|f| f.ok), "{:?}", rep.findings);
    assert_eq!(rep.findings.len(), 2);
    let py = "_TILE_IMAGE_TOKEN_COSTS = {\n    \"gpt-4o\": (85, 170),\n    \"o1\": (75, 150),\n}\n_PTU_CAPACITY = {\n    \"o1\": PTUCapacity(230, 4.0),\n}\n";
    assert!(run(&[("tiles.py", py)]).findings.is_empty());
}

#[test]
fn host_sections_and_host_files_are_not_maker_prices() {
    let json = "{\n  \"302ai\": {\n    \"models\": {\n      \"claude-opus-4-7\": {\n        \"cost\": { \"input\": 10, \"output\": 37.5 }\n      }\n    }\n  },\n  \"anthropic\": {\n    \"models\": {\n      \"claude-opus-4-7\": {\n        \"cost\": { \"input\": 5, \"output\": 30 }\n      }\n    }\n  }\n}\n";
    let rep = run(&[("pricing-data.json", json)]);
    let bad: Vec<_> = rep
        .findings
        .iter()
        .filter(|f| !f.ok)
        .map(|f| (f.line, f.field))
        .collect();
    assert_eq!(bad, [(12, Field::Output)]);
    let host_file = "{\n  \"gemini-2.5-pro\": { \"input_cost_per_token\": 9e-06 }\n}\n";
    assert!(run(&[("model_catalog/vertex_ai.json", host_file)])
        .findings
        .is_empty());
    assert!(!run(&[("model_catalog/gemini.json", host_file)])
        .findings
        .is_empty());
}

#[test]
fn nested_tier_objects_price_their_tier() {
    let json = "{\n  \"claude-opus-4-5\": {\n    \"pricing\": {\n      \"input_per_million_tokens\": 5.0,\n      \"output_per_million_tokens\": 25.0,\n      \"cache_read_per_million_tokens\": 0.5,\n      \"service_tiers\": {\n        \"batch\": {\n          \"input_per_million_tokens\": 2.5,\n          \"output_per_million_tokens\": 12.5,\n          \"cache_read_per_million_tokens\": 0.25\n        }\n      }\n    }\n  }\n}\n";
    let rep = run(&[("anthropic.json", json)]);
    assert!(rep.findings.iter().all(|f| f.ok), "{:?}", rep.findings);
    assert_eq!(rep.findings.len(), 5);
}
