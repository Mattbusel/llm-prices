// One match arm that prices several models at once.
fn known_pricing(model: &str) -> Option<Pricing> {
    match strip_date_suffix(model) {
        "claude-opus-4-7" | "claude-opus-4-6" | "claude-opus-4-5" => Some(Pricing {
            input_per_mtok: 5.0,
            output_per_mtok: 25.0,
            cache_read_per_mtok: 0.50,
        }),
        "claude-opus-5" | "claude-opus-5-5" => Some(Pricing { input_per_mtok: 5.0, output_per_mtok: 25.0 }), // stale for opus-5-5: page says 4 / 20
        _ => None,
    }
}
