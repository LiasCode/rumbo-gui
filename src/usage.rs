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
            (context, 0) => Some(format!("context {context} tokens")),
            (context, window) => Some(format!(
                "context {}% of {}k",
                context * 100 / window,
                window / 1000
            )),
        }
    }

    /// The tokens spent, counted as rumbo's `status` counts them: input
    /// written to the cache is input. What was read from the cache is also
    /// said as a share of all input.
    pub fn tokens(&self) -> String {
        let input = self.total.input + self.total.cache_write;
        let cached = match self.cached_share() {
            Some(share) => format!("{} cached ({share}%)", self.total.cache_read),
            None => format!("{} cached", self.total.cache_read),
        };

        format!("{input} in · {cached} · {} out", self.total.output)
    }

    /// The share of all input the provider read from its cache, as `status`
    /// reports it. A share that drops says something broke the cache.
    pub fn cached_share(&self) -> Option<i64> {
        let all = self.total.input + self.total.cache_write + self.total.cache_read;

        (all > 0).then(|| self.total.cache_read * 100 / all)
    }
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

        assert_eq!(spent.context(), None);
        assert_eq!(spent.cached_share(), None);

        spent.turn(&turn(10, 5, 0, 90, 105, 0));
        spent.turn(&turn(10, 5, 90, 0, 210, 0));

        assert_eq!(spent.tokens(), "110 in · 90 cached (45%) · 10 out");
        assert_eq!(spent.cached_share(), Some(45));
        assert_eq!(spent.context(), Some("context 210 tokens".into()));
    }

    #[test]
    fn keeps_the_last_sizes_a_response_gave() {
        let mut spent = Spent::default();

        spent.turn(&turn(1, 1, 0, 0, 50_000, 200_000));
        spent.turn(&turn(1, 1, 0, 0, 0, 0));

        assert_eq!(spent.context(), Some("context 25% of 200k".into()));
    }
}
