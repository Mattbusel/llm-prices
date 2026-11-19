// Production table row: model and prices on one line, matching the official page.
pub fn price(model: &str) -> Option<(f64, f64)> {
    match model {
        "claude-sonnet-5-5" => Some((2.0, 10.0)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gateway_ratios() {
        // Multiline raw-string fixture: per line its quotes and braces are unbalanced.
        let body = r#"{"data":[
            {"model_name":"claude-sonnet-5-5","model_ratio":1.5,"completion_ratio":5,
             "cache_ratio":0.1,"create_cache_ratio":1.25},
            {"model":"gpt-4o","pricing":{"input_price":5.0,"output_price":99.0,
             "cache_read_price":0.25,"cache_write_price":3.125}}
        ]}"#;
        assert_eq!(parse(body).len(), 2);
    }

    #[test]
    fn made_up_prices_after_a_raw_string() {
        let p = Price { model: "gpt-4o", input: 6.0, output: 30.0 };
        assert!(p.total() > 0.0);
    }
}
