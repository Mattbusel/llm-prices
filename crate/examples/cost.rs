//! Prices one API response from its token counts.
//!
//! ```text
//! cargo run --example cost -- <model> <input> <output> [cache_read] [cache_write_5m] [cache_write_1h] [--batch]
//! cargo run --example cost -- claude-sonnet-4-6 2048 1024 45000 3500
//! cargo run --example cost -- us.anthropic.claude-haiku-4-5-20251001-v1:0 10000 1000 90000 --batch
//! ```
//!
//! With no arguments it prices a few sample responses.

use llm_prices::{lookup, Usage};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let batch = args.iter().any(|a| a == "--batch");
    args.retain(|a| a != "--batch");

    if args.is_empty() {
        let samples = [
            // Anthropic: cached system prompt plus a new 5-minute cache entry.
            ("claude-sonnet-4-6", Usage { input: 2_048, output: 1_024, cache_read: 45_000, cache_write_5m: 3_500, ..Usage::default() }),
            // OpenAI: 300k-token prompt, 200k cached, over GPT-5.4's long-context threshold.
            ("gpt-5.4", Usage { input: 100_000, output: 4_000, cache_read: 200_000, ..Usage::default() }),
            // Gemini through the Batch API.
            ("models/gemini-2.5-flash", Usage { input: 50_000, output: 2_000, batch: true, ..Usage::default() }),
        ];
        for (model, usage) in samples {
            report(model, &usage);
        }
        return;
    }

    let n = |i: usize| -> u64 {
        args.get(i).map_or(0, |s| {
            s.replace(['_', ','], "").parse().unwrap_or_else(|_| {
                eprintln!("not a token count: {s}");
                std::process::exit(2)
            })
        })
    };
    let usage = Usage {
        input: n(1),
        output: n(2),
        cache_read: n(3),
        cache_write_5m: n(4),
        cache_write_1h: n(5),
        batch,
    };
    if !report(&args[0], &usage) {
        std::process::exit(1);
    }
}

fn report(model: &str, usage: &Usage) -> bool {
    let Some(price) = lookup(model) else {
        eprintln!("{model}: not in the table (generated {})", llm_prices::GENERATED);
        return false;
    };
    println!("{model} -> {} ({}, checked {})", price.model, price.provider, price.checked);
    println!("  {usage:?}");
    match price.cost_usage(usage) {
        Some(c) => {
            println!("  input        ${:.6}", c.input);
            println!("  cache read   ${:.6}", c.cache_read);
            println!("  cache write  ${:.6}", c.cache_write);
            println!("  output       ${:.6}", c.output);
            println!("  total        ${:.6}{}", c.total, if c.long_context { "  (long-context prices)" } else { "" });
            println!("  source       {}", price.source_url);
            true
        }
        None => {
            eprintln!("  no cost: a price this usage needs is not listed for {}", price.model);
            false
        }
    }
}
