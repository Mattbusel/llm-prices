// Shape seen in Rust agent CLIs: a match on the model id returning a pricing struct.
pub struct ModelPricing {
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    pub cache_read_per_mtok: f64,
    pub cache_write_per_mtok: f64,
}

pub fn pricing_for(model: &str) -> Option<ModelPricing> {
    match model {
        "claude-sonnet-5-5" => Some(ModelPricing {
            input_per_mtok: 2.0,
            output_per_mtok: 10.0,
            cache_read_per_mtok: 0.20, // stale: page says 0.10 (0.05x)
            cache_write_per_mtok: 2.5,
        }),
        "claude-opus-5-5" => Some(ModelPricing {
            input_per_mtok: 4.0,
            output_per_mtok: 20.0,
            cache_read_per_mtok: 0.20,
            cache_write_per_mtok: 5.0,
        }),
        "claude-haiku-4-5" | "claude-haiku-4-5-20251001" => Some(ModelPricing {
            input_per_mtok: 1.0,
            output_per_mtok: 5.0,
            cache_read_per_mtok: 0.10,
            cache_write_per_mtok: 1.25,
        }),
        _ => None,
    }
}

// Tuple form: (input, output) per million tokens.
pub fn quick(model: &str) -> (f64, f64) {
    match model {
        "claude-opus-4-1" => (15.0, 75.0),
        "claude-sonnet-4-6" => (3.0, 15.0),
        "claude-sonnet-5" => (3.0, 15.0), // stale: the $3/$15 increase was cancelled
        _ => (0.0, 0.0),
    }
}
