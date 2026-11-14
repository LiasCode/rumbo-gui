//! What was entered in the composer, for Up and Down to bring back, the way
//! rumbo's terminal UI does it.

#[derive(Debug, Default)]
pub struct History {
    /// Oldest first.
    entries: Vec<String>,
    /// The entry in the composer, `entries.len()` when none is.
    recalled: usize,
    /// What was being typed before going back.
    draft: String,
}

impl History {
    /// Adds what was entered, unless it repeats the entry before, and leaves
    /// the history at its end.
    pub fn remember(&mut self, text: &str) {
        if self.entries.last().map(String::as_str) != Some(text) {
            self.entries.push(text.to_string());
        }

        self.recalled = self.entries.len();
        self.draft.clear();
    }

    /// The text to put in the composer for a step back (-1) or forward (1),
    /// given what it holds now. Past the newest entry is the draft. None
    /// means there is nothing further that way.
    pub fn recall(&mut self, step: isize, current: &str) -> Option<String> {
        let next = self.recalled.checked_add_signed(step)?;

        if next > self.entries.len() {
            return None;
        }

        if self.recalled == self.entries.len() {
            self.draft = current.to_string();
        }

        self.recalled = next;

        Some(match self.entries.get(next) {
            Some(entry) => entry.clone(),
            None => self.draft.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_back_and_returns_to_the_draft() {
        let mut history = History::default();

        assert_eq!(history.recall(-1, ""), None);

        history.remember("first");
        history.remember("second");
        history.remember("second");

        assert_eq!(history.recall(-1, "half typed").as_deref(), Some("second"));
        assert_eq!(history.recall(-1, "second").as_deref(), Some("first"));
        assert_eq!(history.recall(-1, "first"), None);
        assert_eq!(history.recall(1, "first").as_deref(), Some("second"));
        assert_eq!(history.recall(1, "second").as_deref(), Some("half typed"));
        assert_eq!(history.recall(1, "half typed"), None);
    }

    #[test]
    fn starts_over_at_the_end_after_entering() {
        let mut history = History::default();

        history.remember("first");
        history.recall(-1, "");
        history.remember("again");

        assert_eq!(history.recall(-1, "").as_deref(), Some("again"));
    }
}
