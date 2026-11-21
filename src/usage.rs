//! What the runs followed in this window have spent, and how full the
//! model's context is.

use crate::protocol::{Event, Usage};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Spent {
    total: Usage,
    /// The size of the conversation and of the model's context, as of the
    /// last response that said.
    context: i64,
    window: i64,
}

impl Spent {
    /// Counts one model response, as a turn event reports it.
    pub fn turn(&mut self, event: &Event) {
        if let Some(usage) = event.usage {
            self.total.input += usage.input;
            self.total.output += usage.output;
            self.total.cache_read += usage.cache_read;
            self.total.cache_write += usage.cache_write;
        }

        if event.context > 0 {
            self.context = event.context;
        }

        if event.window > 0 {
            self.window = event.window;
        }
    }

    /// How full the model's context is: as a share when its size is known,
    /// and as a count of tokens otherwise.
    pub fn context(&self) -> Option<String> {
        match (self.context, self.window) {
            (0, _) => None,
            (context, 0) => Some(format!("context {} tokens", short(context))),
            (context, window) => Some(format!(
                "context {}% of {}",
                context * 100 / window,
                short(window)
            )),
        }
    }

    /// The tokens spent, counted as rumbo's `status` counts them: input
    /// written to the cache is input. What was read from the cache is also
    /// said as a share of all input. None until some are spent.
    pub fn tokens(&self) -> Option<String> {
        if self.total == Usage::default() {
            return None;
        }

        let input = self.total.input + self.total.cache_write;
        let cached = match self.cached_share() {
            Some(share) => format!("{} cached ({share}%)", short(self.total.cache_read)),
            None => format!("{} cached", short(self.total.cache_read)),
        };

        Some(format!(
            "{} in · {cached} · {} out",
            short(input),
            short(self.total.output)
        ))
    }

    /// The context and the tokens as they are counted, to the last one, for
    /// a person who wants more than `context` and `tokens` round to: one
    /// count a line, so that each stays short.
    pub fn exactly(&self) -> Option<String> {
        let mut lines = Vec::new();

        match (self.context, self.window) {
            (0, _) => {}
            (context, 0) => lines.push(format!("Context: {} tokens", grouped(context))),
            (context, window) => lines.push(format!(
                "Context: {} of {} tokens",
                grouped(context),
                grouped(window)
            )),
        }

        if self.total != Usage::default() {
            let cached = match self.cached_share() {
                Some(share) => format!("{} ({share}%)", grouped(self.total.cache_read)),
                None => grouped(self.total.cache_read),
            };

            lines.push(format!(
                "In: {}",
                grouped(self.total.input + self.total.cache_write)
            ));
            lines.push(format!("Cached: {cached}"));
            lines.push(format!("Out: {}", grouped(self.total.output)));
        }

        (!lines.is_empty()).then(|| lines.join("\n"))
    }

    /// The share of all input the provider read from its cache, as `status`
    /// reports it. A share that drops says something broke the cache.
    pub fn cached_share(&self) -> Option<i64> {
        let all = self.total.input + self.total.cache_write + self.total.cache_read;

        (all > 0).then(|| self.total.cache_read * 100 / all)
    }
}

/// A count of tokens as it is read at a glance: as it is under a thousand,
/// else in thousands (K) or millions (M), with a decimal while it is short.
pub fn short(count: i64) -> String {
    if count < 1_000 {
        return count.to_string();
    }

    let thousands = scaled(count as f64 / 1e3);

    // What rounds up to a thousand thousands is a million.
    if count < 1_000_000 && thousands != "1000" {
        return format!("{thousands}K");
    }

    format!("{}M", scaled(count as f64 / 1e6))
}

/// A number with one decimal under a hundred, none from there, and no
/// decimal that is zero.
fn scaled(value: f64) -> String {
    if value < 99.95 {
        let tenths = format!("{value:.1}");

        return tenths.strip_suffix(".0").unwrap_or(&tenths).to_string();
    }

    format!("{value:.0}")
}

/// A count with its thousands set apart, as 1,234,567.
fn grouped(count: i64) -> String {
    let digits = count.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);

    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }

        grouped.push(digit);
    }

    grouped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(
        input: i64,
        output: i64,
        cache_read: i64,
        cache_write: i64,
        context: i64,
        window: i64,
    ) -> Event {
        Event {
            kind: "turn".into(),
            usage: Some(Usage {
                input,
                output,
                cache_read,
                cache_write,
            }),
            context,
            window,
            ..Event::default()
        }
    }

    #[test]
    fn counts_like_status_does() {
        let mut spent = Spent::default();

        assert_eq!(spent.tokens(), None);
        assert_eq!(spent.context(), None);
        assert_eq!(spent.cached_share(), None);

        spent.turn(&turn(10, 5, 0, 90, 105, 0));
        spent.turn(&turn(10, 5, 90, 0, 210, 0));

        assert_eq!(
            spent.tokens().as_deref(),
            Some("110 in · 90 cached (45%) · 10 out")
        );
        assert_eq!(spent.cached_share(), Some(45));
        assert_eq!(spent.context(), Some("context 210 tokens".into()));
        assert_eq!(
            spent.exactly().as_deref(),
            Some("Context: 210 tokens\nIn: 110\nCached: 90 (45%)\nOut: 10")
        );
    }

    #[test]
    fn keeps_the_last_sizes_a_response_gave() {
        let mut spent = Spent::default();

        spent.turn(&turn(1, 1, 0, 0, 50_000, 200_000));
        spent.turn(&turn(1, 1, 0, 0, 0, 0));

        assert_eq!(spent.context(), Some("context 25% of 200K".into()));
    }

    #[test]
    fn says_large_counts_in_thousands_and_millions() {
        let mut spent = Spent::default();

        assert_eq!(spent.exactly(), None);

        spent.turn(&turn(1_234_567, 45_600, 2_000_000, 0, 150_000, 1_000_000));

        assert_eq!(
            spent.tokens().as_deref(),
            Some("1.2M in · 2M cached (61%) · 45.6K out")
        );
        assert_eq!(spent.context(), Some("context 15% of 1M".into()));
        assert_eq!(
            spent.exactly().as_deref(),
            Some(
                "Context: 150,000 of 1,000,000 tokens\n\
                 In: 1,234,567\n\
                 Cached: 2,000,000 (61%)\n\
                 Out: 45,600"
            )
        );
    }

    #[test]
    fn rounds_a_count_to_what_reads_at_a_glance() {
        let said = [
            (0, "0"),
            (999, "999"),
            (1_000, "1K"),
            (1_234, "1.2K"),
            (12_345, "12.3K"),
            (99_960, "100K"),
            (123_456, "123K"),
            (999_499, "999K"),
            (999_950, "1M"),
            (1_234_567, "1.2M"),
            (45_000_000, "45M"),
            (123_456_789, "123M"),
        ];

        for (count, short_said) in said {
            assert_eq!(short(count), short_said, "{count}");
        }

        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1_000), "1,000");
        assert_eq!(grouped(1_234_567), "1,234,567");
    }
}
