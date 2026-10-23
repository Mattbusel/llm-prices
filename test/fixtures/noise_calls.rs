// Calls and logs that carry a model id and numbers that are not prices. None may be flagged.
pub fn run(t: &mut Tracker, log: &mut Log) {
    t.observe("gpt-4o", 1.0); // 50% utilisation
    let b = snap("w1", 10.0, vec![("gpt-4o", 10.0)], 100);
    log.ingest_line(r#"{"model":"gpt-4o","prompt_tokens":5,"completion_tokens":2}"#);
    let usage = Usage { model: "claude-opus-5-5", tokens_in: 500, tokens_out: 200 };
    send("claude-sonnet-5-5", 4096, 1024);
}

#[cfg(test)]
mod tests {
    #[test]
    fn records_a_price_change() {
        // Made-up prices are normal in tests and are skipped.
        tracker.record_price(make_price("gpt-4o", 5.0, 15.0, 1_000));
        let p = Price { model: "claude-sonnet-5-5", input: 9.0, output: 99.0 };
    }
}
