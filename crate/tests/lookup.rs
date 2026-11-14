//! Walks every row's model id and aliases through the id forms `lookup` documents and checks
//! that each one resolves to that row.

use llm_prices::{all, lookup, Price};

/// Every id the table documents for a row: the model id and its aliases.
fn ids(p: &Price) -> impl Iterator<Item = &'static str> {
    std::iter::once(p.model).chain(p.aliases.iter().copied())
}

/// The id with a trailing `-YYYYMMDD` removed, when it has one.
fn undated(id: &str) -> &str {
    match id.rsplit_once('-') {
        Some((head, tail)) if tail.len() == 8 && tail.starts_with("20") && tail.bytes().all(|b| b.is_ascii_digit()) => head,
        _ => id,
    }
}

#[track_caller]
fn assert_resolves(form: &str, p: &Price) {
    match lookup(form) {
        Some(found) => assert_eq!(found.model, p.model, "{form:?} resolved to the wrong row"),
        None => panic!("{form:?} did not resolve (expected {})", p.model),
    }
}

#[test]
fn ids_and_aliases_resolve_exactly() {
    for p in all() {
        for id in ids(p) {
            assert_resolves(id, p);
            assert_resolves(&id.to_ascii_uppercase(), p);
            assert_resolves(&format!("  {id}\n"), p);
        }
    }
}

#[test]
fn ids_are_unique() {
    let mut seen = std::collections::HashSet::new();
    for p in all() {
        for id in ids(p) {
            assert!(seen.insert(id), "{id} appears on two rows");
        }
    }
}

#[test]
fn provider_prefixes_resolve() {
    for p in all() {
        for id in ids(p) {
            for prefix in ["anthropic/", "bedrock/", "models/", "openrouter/", "us.anthropic.", "bedrock/us.anthropic."] {
                assert_resolves(&format!("{prefix}{id}"), p);
            }
            let own = match p.provider {
                "anthropic" => "anthropic/",
                "openai" => "openai/",
                "google" => "google/",
                "deepseek" => "deepseek/",
                other => panic!("new provider {other}: add its prefix here"),
            };
            assert_resolves(&format!("{own}{id}"), p);
        }
    }
}

#[test]
fn bedrock_ids_resolve_for_anthropic_rows() {
    for p in all().iter().filter(|p| p.provider == "anthropic") {
        for id in ids(p) {
            for region in ["", "us.", "eu.", "apac.", "global."] {
                assert_resolves(&format!("{region}anthropic.{id}-v1:0"), p);
                assert_resolves(&format!("{region}anthropic.{id}-v2:0"), p);
            }
        }
    }
}

#[test]
fn vertex_at_date_resolves_for_anthropic_rows() {
    for p in all().iter().filter(|p| p.provider == "anthropic") {
        for id in ids(p) {
            let base = undated(id);
            // Vertex writes the date after `@`: `claude-opus-4-1@20250805`.
            let date = if base == id { "20990101" } else { &id[base.len() + 1..] };
            assert_resolves(&format!("{base}@{date}"), p);
            assert_resolves(&format!("vertex_ai/{base}@{date}"), p);
        }
    }
}

#[test]
fn dated_and_undated_anthropic_ids_resolve() {
    for p in all().iter().filter(|p| p.provider == "anthropic") {
        for id in ids(p) {
            let base = undated(id);
            // The undated form of a dated id, and any dated form of an undated one.
            assert_resolves(base, p);
            assert_resolves(&format!("{base}-20990101"), p);
            assert_resolves(&format!("anthropic/{base}-20990101"), p);
        }
    }
}

/// Current behavior, not a promise: lookup does not strip OpenAI's `-YYYY-MM-DD` snapshot
/// suffix, because OpenAI prices some snapshots differently from the undated alias
/// (`gpt-4o-2024-05-13` is a separate row). A dated OpenAI id resolves only when the table
/// lists that exact snapshot.
#[test]
fn dated_openai_ids_are_not_guessed() {
    for id in ["gpt-4o-2024-08-06", "gpt-4o-2024-11-20", "gpt-4.1-2025-04-14", "o3-2025-04-16", "gpt-4o-mini-2024-07-18"] {
        assert!(lookup(id).is_none(), "{id} now resolves; update this test and the lookup docs");
    }
    assert_eq!(lookup("gpt-4o-2024-05-13").unwrap().input, Some(5.0));
    assert_eq!(lookup("gpt-4o").unwrap().input, Some(2.5));
}

#[test]
fn unknown_ids_do_not_resolve() {
    for id in ["", "   ", "not-a-model", "anthropic/", "claude", "gpt", "models/"] {
        assert!(lookup(id).is_none(), "{id:?}");
    }
}
