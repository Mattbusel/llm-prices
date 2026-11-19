// A tracker that prices unknown models at a guess.
pub fn pricing(model: &str) -> (f64, f64) {
    match model {
        "gpt-4o" => (2.5, 10.0),
        "gpt-4o-mini" => (0.15, 0.6),
        "claude-sonnet-4-6" => (3.0, 15.0),
        "claude-haiku-4-5" => (1.0, 5.0),
        _ => (1.0, 3.0),
    }
}

/// Cost of a call in USD; unknown models cost nothing.
pub fn cost(model: &str, input: u64, output: u64) -> f64 {
    let (i, o) = match model {
        "gpt-4.1" => (2.0, 8.0),
        "o3" => (2.0, 8.0),
        _ => (0.0, 0.0),
    };
    (input as f64 * i + output as f64 * o) / 1e6
}
