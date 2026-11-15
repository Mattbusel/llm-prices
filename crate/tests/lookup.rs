//! Walks every row's model id and aliases through the id forms `lookup` documents and checks
//! that each one resolves to that row.

use llm_prices::{all, lookup, lookup_provider, Price, Provider};

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

/// Some other provider sells a model under this exact id (`openai/gpt-oss-120b` on Groq and
/// Together AI), so plain `lookup` cannot tell the rows apart.
fn shared_id(p: &Price, id: &str) -> bool {
    all().iter().any(|q| q.provider != p.provider && ids(q).any(|m| m.eq_ignore_ascii_case(id)))
}

#[track_caller]
fn assert_resolves(form: &str, p: &Price) {
    match lookup(form) {
        Some(found) => assert_eq!(found.model, p.model, "{form:?} resolved to the wrong row"),
        None => panic!("{form:?} did not resolve (expected {})", p.model),
    }
}

#[track_caller]
fn assert_resolves_under_provider(form: &str, p: &Price) {
    match lookup_provider(p.provider, form) {
        Some(found) => assert!(std::ptr::eq(found, p), "{form:?} under {} resolved to {}/{}", p.provider, found.provider, found.model),
        None => panic!("{form:?} did not resolve under {} (expected {})", p.provider, p.model),
    }
}

#[test]
fn ids_and_aliases_resolve_exactly() {
    for p in all() {
        for id in ids(p) {
            for form in [id.to_string(), id.to_ascii_uppercase(), format!("  {id}\n")] {
                assert_resolves(&form, p);
                assert_resolves_under_provider(&form, p);
            }
            if !shared_id(p, id) {
                assert!(std::ptr::eq(lookup(id).unwrap(), p), "{id}");
            }
        }
    }
}

#[test]
fn ids_are_unique_per_provider() {
    let mut seen = std::collections::HashSet::new();
    for p in all() {
        for id in ids(p) {
            assert!(seen.insert((p.provider, id.to_ascii_lowercase())), "{id} appears on two {} rows", p.provider);
        }
    }
}

/// Only id forms that exist in the wild: the provider's own prefix and `openrouter/` for every
/// model maker's row, Bedrock forms for Anthropic rows, `models/` for Gemini rows, and the
/// LiteLLM prefixes for the hosts. `lookup` happens to accept other combinations
/// (`us.anthropic.gpt-4o`), but that is tolerated, not promised, so it is not tested here.
#[test]
fn provider_prefixes_resolve() {
    for p in all() {
        let prefixes: &[&str] = match p.provider {
            "anthropic" => &["anthropic/", "openrouter/", "bedrock/", "us.anthropic.", "bedrock/us.anthropic.", "vertex_ai/"],
            "openai" => &["openai/", "openrouter/", "azure/"],
            "google" => &["google/", "openrouter/", "models/", "gemini/", "vertex_ai/"],
            "deepseek" => &["deepseek/", "openrouter/"],
            "mistral" => &["mistral/", "openrouter/", "mistralai/"],
            "xai" => &["xai/", "openrouter/", "x-ai/"],
            "cohere" => &["cohere/", "openrouter/"],
            "groq" => &["groq/"],
            "together" => &["together_ai/", "together/"],
            "fireworks" => &["fireworks_ai/", "fireworks/"],
            other => panic!("new provider {other}: add its prefixes here"),
        };
        for id in ids(p) {
            for prefix in prefixes {
                let form = format!("{prefix}{id}");
                assert_resolves(&form, p);
                // The prefix names the provider, so plain lookup finds this exact row even
                // when another host sells the same id.
                if !matches!(*prefix, "openrouter/" | "us.anthropic." | "models/") {
                    assert!(std::ptr::eq(lookup(&form).unwrap(), p), "{form}");
                }
            }
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
                assert!(std::ptr::eq(lookup_provider(Provider::Bedrock, &format!("{region}anthropic.{id}-v1:0")).unwrap(), p));
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
            assert!(std::ptr::eq(lookup_provider("vertex_ai", &format!("{base}@{date}")).unwrap(), p));
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
            assert_resolves(&format!("{base}-latest"), p);
        }
    }
}

/// OpenAI snapshot ids (`-YYYY-MM-DD`) resolve to the base row. A snapshot the table prices
/// on its own row keeps that row.
#[test]
fn dated_openai_ids_resolve_to_the_base_row() {
    for (id, base) in [
        ("gpt-4o-2024-08-06", "gpt-4o"),
        ("gpt-4o-2024-11-20", "gpt-4o"),
        ("openai/gpt-4o-2024-11-20", "gpt-4o"),
        ("azure/gpt-4.1-2025-04-14", "gpt-4.1"),
        ("o3-2025-04-16", "o3"),
        ("gpt-4o-mini-2024-07-18", "gpt-4o-mini"),
        ("gpt-5-2025-08-07", "gpt-5"),
    ] {
        assert_eq!(lookup(id).map(|p| p.model), Some(base), "{id}");
        assert_eq!(lookup_provider("openai", id).map(|p| p.model), Some(base), "{id}");
    }
    assert_eq!(lookup("gpt-4o-2024-05-13").unwrap().input, Some(5.0));
    assert_eq!(lookup("gpt-4o").unwrap().input, Some(2.5));
    // Every dated row of every provider is still its own answer.
    for p in all() {
        for id in ids(p) {
            assert_resolves(&format!("{id}-2099-01-01"), p);
        }
    }
}

#[test]
fn latest_aliases_resolve() {
    for (id, model) in [
        ("mistral-large-latest", "mistral-large-4"),
        ("mistral-small-latest", "mistral-small-2603"),
        ("codestral-latest", "codestral-2508"),
        ("grok-4.7-latest", "grok-4.7"),
        ("xai/grok-4.3-latest", "grok-4.3"),
        ("claude-3-5-haiku-latest", "claude-3-5-haiku-20241022"),
        ("claude-opus-5-5-latest", "claude-opus-5-5"),
    ] {
        assert_eq!(lookup(id).map(|p| p.model), Some(model), "{id}");
    }
}

#[test]
fn provider_lookup_keeps_hosts_apart() {
    // gpt-oss-120b is sold by three hosts in the table; OpenAI's own API does not sell it.
    let groq = lookup_provider("groq", "openai/gpt-oss-120b").unwrap();
    let together = lookup_provider("together_ai", "openai/gpt-oss-120b").unwrap();
    let fireworks = lookup_provider("fireworks", "accounts/fireworks/models/gpt-oss-120b").unwrap();
    assert_eq!((groq.provider, together.provider, fireworks.provider), ("groq", "together", "fireworks"));
    assert_eq!(fireworks.cache_read, Some(0.015));
    assert_eq!(together.cache_read, None);
    assert!(lookup_provider("openai", "openai/gpt-oss-120b").is_none());
    assert!(lookup_provider("anthropic", "gpt-4o").is_none());
    assert!(lookup_provider("groq", "claude-sonnet-4-6").is_none());
    assert!(lookup_provider("together", "gpt-4o").is_none());
    // Fireworks' short id and LiteLLM's form.
    assert!(std::ptr::eq(lookup_provider("fireworks", "gpt-oss-120b").unwrap(), fireworks));
    assert!(std::ptr::eq(lookup_provider("fireworks_ai", "fireworks_ai/accounts/fireworks/models/gpt-oss-120b").unwrap(), fireworks));
    // Platforms fall back to the maker's row only for makers they resell at list price.
    assert_eq!(lookup_provider("bedrock", "claude-sonnet-4-6").unwrap().provider, "anthropic");
    assert_eq!(lookup_provider("vertex", "gemini-2.5-pro").unwrap().provider, "google");
    assert_eq!(lookup_provider("azure", "gpt-4o").unwrap().provider, "openai");
    assert_eq!(lookup_provider("openrouter", "mistralai/mistral-large-4").unwrap().provider, "mistral");
    assert_eq!(lookup_provider("openrouter", "x-ai/grok-4.7").unwrap().provider, "xai");
    assert!(lookup_provider("bedrock", "gpt-4o").is_none());
    assert!(lookup_provider("azure", "claude-sonnet-4-6").is_none());
    assert!(lookup_provider("vertex", "mistral-large-4").is_none());
    assert!(lookup_provider("bedrock", "deepseek-flash").is_none());
    // Unknown provider names never fall back to another provider.
    assert!(lookup_provider("acme", "gpt-4o").is_none());
}

#[test]
fn provider_rows_cover_the_table() {
    let total: usize = Provider::ALL.iter().map(|p| p.rows().count()).sum();
    assert_eq!(total, all().len());
    for name in ["mistral", "xai", "cohere", "groq", "together", "fireworks"] {
        let p = Provider::parse(name).unwrap();
        assert!(p.rows().count() > 0, "{name}");
        for row in p.rows() {
            assert!(row.source_url.starts_with("https://"), "{}", row.model);
            assert_eq!(row.checked.len(), 10, "{}", row.model);
        }
    }
}

#[test]
fn xai_long_context_starts_at_200k() {
    let g = lookup("grok-4.7").unwrap();
    let lc = g.long_context.unwrap();
    // "≥ 200k prompt tokens" on the page; the table's threshold is exclusive.
    assert_eq!(lc.threshold_tokens, 199_999);
    let at = llm_prices::Usage { input: 200_000, ..Default::default() };
    assert!(g.cost_usage(&at).unwrap().long_context);
    let under = llm_prices::Usage { input: 199_999, ..Default::default() };
    assert!(!g.cost_usage(&under).unwrap().long_context);
}

#[test]
fn unknown_ids_do_not_resolve() {
    for id in ["", "   ", "not-a-model", "anthropic/", "claude", "gpt", "models/", "groq/", "-latest", "latest", "2024-08-06"] {
        assert!(lookup(id).is_none(), "{id:?}");
    }
}

#[test]
fn legacy_deepseek_names_and_delisted_rows_still_price() {
    for id in ["deepseek-chat", "deepseek-reasoner"] {
        assert_eq!(lookup_provider("deepseek", id).map(|p| p.model), Some("deepseek-flash"), "{id}");
    }
    assert_eq!(lookup_provider("google_genai", "gemini-2.5-flash").map(|p| p.model), Some("gemini-2.5-flash"));
    for id in ["gpt-3.5-turbo", "o1", "claude-3-5-haiku-20241022"] {
        let p = lookup(id).unwrap_or_else(|| panic!("{id}"));
        assert_eq!(p.status, "retired", "{id}");
    }
}
