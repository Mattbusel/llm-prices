//! Refuse a call before sending it when its worst-case cost would break a budget.
//!
//! ```text
//! cargo run --example budget_check
//! ```

use llm_prices::{lookup, Usage};

struct Budget {
    limit_usd: f64,
    spent_usd: f64,
}

impl Budget {
    /// The worst case is the prompt plus the full `max_output_tokens`.
    fn check(&self, model: &str, prompt_tokens: u64, max_output_tokens: u64) -> Result<f64, String> {
        let price = lookup(model).ok_or_else(|| format!("no price for {model}"))?;
        let worst = price.cost(prompt_tokens, max_output_tokens).ok_or_else(|| format!("{model} lists no input or output price"))?;
        if self.spent_usd + worst > self.limit_usd {
            return Err(format!("{model}: worst case ${worst:.6} would exceed ${:.2} (spent ${:.6})", self.limit_usd, self.spent_usd));
        }
        Ok(worst)
    }

    fn record(&mut self, model: &str, usage: &Usage) {
        let cost = lookup(model).and_then(|p| p.cost_usage(usage)).expect("priced model");
        self.spent_usd += cost.total;
    }
}

fn main() {
    let mut budget = Budget { limit_usd: 0.05, spent_usd: 0.0 };
    for i in 0..10 {
        match budget.check("gemini-2.5-pro", 8_000, 2_000) {
            Ok(_) => {
                // What the response reported.
                budget.record("gemini-2.5-pro", &Usage { input: 8_000, output: 1_500, ..Usage::default() });
                println!("call {i}: sent, spent ${:.6}", budget.spent_usd);
            }
            Err(e) => {
                println!("call {i}: refused, {e}");
                break;
            }
        }
    }
}
