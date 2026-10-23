// Per-token constants written as per-million literals divided down.
const SONNET_4_6_IN: f64 = 3.0 / 1_000_000.0;

pub fn rates(model: &str) -> Option<(f64, f64)> {
    Some(match model {
        "claude-sonnet-4-6" => (3.0 / 1_000_000.0, 15.0 / 1_000_000.0),
        "claude-opus-5" => (5.0 / 1_000_000.0, 20.0 / 1_000_000.0), // stale output: page says 25
        _ => return None,
    })
}
