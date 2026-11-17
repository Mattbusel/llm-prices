//! The same request on each service tier the provider prices, and with US-only inference.
//!
//! ```text
//! cargo run --example tiers
//! ```

use llm_prices::{lookup, Usage};

fn main() {
    let usage = Usage { input: 250_000, output: 4_000, ..Usage::default() };
    for model in ["gemini-2.5-pro", "gpt-5.4", "claude-opus-5-5"] {
        let price = lookup(model).expect("priced model");
        println!("{model}");
        if let Some(c) = price.cost_usage(&usage) {
            println!("  standard  ${:.6}", c.total);
        }
        if let Some(c) = price.cost_usage(&Usage { batch: true, ..usage }) {
            println!("  batch     ${:.6}", c.total);
        }
        for name in price.tiers() {
            if let Some(c) = price.tier(name).and_then(|t| t.cost_usage(&usage)) {
                println!("  {name:<9} ${:.6}", c.total);
            }
        }
        if let Some(c) = price.cost_usage_geo(&usage, "us") {
            println!("  inference_geo=us ${:.6}", c.total);
        }
    }
}
