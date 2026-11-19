//! The price table the scan compares against: the `llm-prices` crate's embedded rows, or a
//! `prices.json` file in the same schema (fresher, and it carries scheduled prices).

use serde_json::Value;

/// A price field a source file can hard-code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Field {
    Input,
    Output,
    CacheRead,
    CacheWrite,
    BatchInput,
    BatchOutput,
}

impl Field {
    pub fn name(self) -> &'static str {
        match self {
            Field::Input => "input",
            Field::Output => "output",
            Field::CacheRead => "cache_read",
            Field::CacheWrite => "cache_write",
            Field::BatchInput => "batch_input",
            Field::BatchOutput => "batch_output",
        }
    }

    /// The field without its batch prefix.
    pub fn base(self) -> Field {
        match self {
            Field::BatchInput => Field::Input,
            Field::BatchOutput => Field::Output,
            f => f,
        }
    }

    fn batch_of(self) -> Option<Field> {
        match self {
            Field::Input => Some(Field::BatchInput),
            Field::Output => Some(Field::BatchOutput),
            _ => None,
        }
    }
}

/// One set of prices in USD per 1M tokens.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fields {
    pub input: Option<f64>,
    pub output: Option<f64>,
    pub cache_read: Option<f64>,
    /// Cache write prices, one per cache lifetime, the shortest lifetime first.
    pub cache_write: Vec<f64>,
    pub batch_input: Option<f64>,
    pub batch_output: Option<f64>,
}

impl Fields {
    /// Every price listed for `f`.
    pub fn values(&self, f: Field) -> Vec<f64> {
        match f {
            Field::CacheWrite => self.cache_write.clone(),
            _ => self.primary(f).into_iter().collect(),
        }
    }

    /// The main price for `f` (the 5-minute write price for cache writes).
    pub fn primary(&self, f: Field) -> Option<f64> {
        match f {
            Field::Input => self.input,
            Field::Output => self.output,
            Field::CacheRead => self.cache_read,
            Field::CacheWrite => self.cache_write.first().copied(),
            Field::BatchInput => self.batch_input,
            Field::BatchOutput => self.batch_output,
        }
    }

    fn from_price(p: &llm_prices::Price) -> Fields {
        Fields {
            input: p.input,
            output: p.output,
            cache_read: p.cache_read,
            cache_write: p.cache_write.iter().map(|(_, v)| *v).collect(),
            batch_input: p.batch_input,
            batch_output: p.batch_output,
        }
    }

    fn from_long(l: &llm_prices::LongContext) -> Fields {
        Fields {
            input: l.input,
            output: l.output,
            cache_read: l.cache_read,
            cache_write: l.cache_write.iter().map(|(_, v)| *v).collect(),
            batch_input: l.batch_input,
            batch_output: l.batch_output,
        }
    }

    fn from_json(v: &Value) -> Fields {
        let num = |k: &str| v.get(k).and_then(Value::as_f64);
        let cache_write = match v.get("cache_write") {
            Some(Value::Number(n)) => n.as_f64().into_iter().collect(),
            Some(Value::Object(m)) => {
                // Shortest cache lifetime first ("5m" before "1h"), as in the crate.
                let minutes = |k: &str| -> f64 {
                    let n: f64 = k
                        .trim_end_matches(|c: char| c.is_alphabetic())
                        .parse()
                        .unwrap_or(f64::MAX);
                    if k.ends_with('h') {
                        n * 60.0
                    } else {
                        n
                    }
                };
                let mut kv: Vec<(f64, f64)> = m
                    .iter()
                    .filter_map(|(k, v)| Some((minutes(k), v.as_f64()?)))
                    .collect();
                kv.sort_by(|a, b| a.0.total_cmp(&b.0));
                kv.into_iter().map(|(_, v)| v).collect()
            }
            _ => Vec::new(),
        };
        Fields {
            input: num("input"),
            output: num("output"),
            cache_read: num("cache_read"),
            cache_write,
            batch_input: num("batch_input"),
            batch_output: num("batch_output"),
        }
    }
}

/// Another service tier of a model (flex, priority, fast, ...).
#[derive(Debug, Clone, PartialEq)]
pub struct Tier {
    pub name: String,
    pub fields: Fields,
    pub long: Option<Fields>,
}

/// One model's row.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub provider: String,
    pub model: String,
    pub aliases: Vec<String>,
    pub status: String,
    pub source_url: String,
    pub checked: String,
    pub std: Fields,
    pub long: Option<Fields>,
    pub tiers: Vec<Tier>,
    /// Published later prices (a promotion that ends on a known date).
    pub scheduled: Vec<Fields>,
}

impl Row {
    /// The prices that apply above the long-context threshold, shaped as a row, or `None`
    /// when the table lists no long-context tier for this model.
    pub fn tier_row(&self) -> Option<Row> {
        let long = self.long.clone()?;
        Some(Row {
            provider: self.provider.clone(),
            model: self.model.clone(),
            aliases: Vec::new(),
            status: self.status.clone(),
            source_url: self.source_url.clone(),
            checked: self.checked.clone(),
            std: long,
            long: None,
            tiers: self
                .tiers
                .iter()
                .filter_map(|t| {
                    t.long.clone().map(|l| Tier {
                        name: t.name.clone(),
                        fields: l,
                        long: None,
                    })
                })
                .collect(),
            scheduled: Vec::new(),
        })
    }

    /// The main price of `field`, falling back to the non-batch field.
    pub fn primary(&self, field: Field) -> Option<f64> {
        self.std
            .primary(field)
            .or_else(|| self.std.primary(field.base()))
    }

    /// Every price the provider page lists for this field, any tier, plus the 1.1x regional
    /// (data residency) prices Anthropic and OpenAI charge.
    pub fn acceptable(&self, field: Field) -> Vec<f64> {
        let base = field.base();
        let pick = |g: &Fields, out: &mut Vec<f64>| {
            out.extend(g.values(base));
            if let Some(b) = base.batch_of() {
                out.extend(g.values(b));
            }
        };
        let mut standard = Vec::new();
        pick(&self.std, &mut standard);
        if let Some(l) = &self.long {
            pick(l, &mut standard);
        }
        let mut out = standard.clone();
        for s in &self.scheduled {
            pick(s, &mut out);
        }
        for t in &self.tiers {
            pick(&t.fields, &mut out);
        }
        for t in &self.tiers {
            if let Some(l) = &t.long {
                pick(l, &mut out);
            }
        }
        if self.provider == "anthropic" || self.provider == "openai" {
            for v in standard {
                out.push((v * 1.1 * 1e9).round() / 1e9);
            }
        }
        out
    }

    /// True when the row has both an input and an output price.
    pub fn is_chat(&self) -> bool {
        self.std.input.is_some() && self.std.output.is_some()
    }
}

/// The whole table.
#[derive(Debug, Clone)]
pub struct Table {
    pub rows: Vec<Row>,
    pub generated: String,
}

impl Table {
    /// The rows embedded in the `llm-prices` crate this binary was built with.
    pub fn embedded() -> Table {
        let rows = llm_prices::all()
            .iter()
            .map(|p| Row {
                provider: p.provider.to_string(),
                model: p.model.to_string(),
                aliases: p.aliases.iter().map(|s| s.to_string()).collect(),
                status: p.status.to_string(),
                source_url: p.source_url.to_string(),
                checked: p.checked.to_string(),
                std: Fields::from_price(p),
                long: p.long_context.as_ref().map(Fields::from_long),
                tiers: p
                    .tiers()
                    .filter_map(|name| {
                        let t = p.tier(name)?;
                        Some(Tier {
                            name: name.to_string(),
                            fields: Fields::from_price(t),
                            long: t.long_context.as_ref().map(Fields::from_long),
                        })
                    })
                    .collect(),
                scheduled: Vec::new(),
            })
            .collect();
        Table {
            rows,
            generated: llm_prices::GENERATED.to_string(),
        }
    }

    /// Rows from a `prices.json` file.
    pub fn from_json(text: &str) -> Result<Table, String> {
        let v: Value = serde_json::from_str(text)
            .map_err(|e| format!("prices file is not valid JSON: {e}"))?;
        let models = v
            .get("models")
            .and_then(Value::as_array)
            .ok_or("prices file has no \"models\" array")?;
        let s = |m: &Value, k: &str| m.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let mut rows = Vec::with_capacity(models.len());
        for m in models {
            let long = m
                .get("long_context")
                .filter(|l| l.is_object())
                .map(Fields::from_json);
            let tiers = m
                .get("other_tiers")
                .and_then(Value::as_object)
                .map(|o| {
                    o.iter()
                        .map(|(name, t)| Tier {
                            name: name.clone(),
                            fields: Fields::from_json(t),
                            long: t
                                .get("long_context")
                                .filter(|l| l.is_object())
                                .map(Fields::from_json),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let scheduled = m
                .get("scheduled")
                .and_then(Value::as_array)
                .map(|a| a.iter().map(Fields::from_json).collect())
                .unwrap_or_default();
            rows.push(Row {
                provider: s(m, "provider"),
                model: s(m, "model"),
                aliases: m
                    .get("aliases")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default(),
                status: s(m, "status"),
                source_url: s(m, "source_url"),
                checked: s(m, "checked"),
                std: Fields::from_json(m),
                long,
                tiers,
                scheduled,
            });
        }
        let generated = v
            .get("generated")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        Ok(Table { rows, generated })
    }
}

/// Token counts of the reference calls every dollar figure is computed on.
pub mod call {
    /// Prompt tokens of the standard call.
    pub const PROMPT: u64 = 100_000;
    /// Output tokens of the standard call.
    pub const OUTPUT: u64 = 5_000;
    /// Prompt tokens served from the prompt cache in the cached call.
    pub const CACHED: u64 = 60_000;
}

/// Cost in USD of a call with `fresh` uncached prompt tokens at `input`, `cached` tokens at
/// `cache`, and `output` tokens at `out` (all prices per 1M tokens).
pub fn call_cost(fresh: u64, input: f64, cached: u64, cache: f64, output: u64, out: f64) -> f64 {
    (fresh as f64 * input + cached as f64 * cache + output as f64 * out) / 1e6
}
