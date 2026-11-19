//! What was entered in the composer, for Up and Down to bring back, the way
//! rumbo's terminal UI does it: what the host recalls of the project's
//! sessions, and then what was entered in this window.

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
    /// Puts what the host recalls, most recent first, before what was
    /// entered here. An entry being shown stays the one shown.
    pub fn recalled(&mut self, asked: &[String]) {
        let mut earlier: Vec<String> = asked.iter().rev().cloned().collect();

        // What was entered here before the answer came may repeat the most
        // recent of them.
        if earlier.last() == self.entries.first() {
            earlier.pop();
        }

        self.recalled += earlier.len();
        earlier.append(&mut self.entries);
        self.entries = earlier;
    }

    /// The most recent entries, newest first and each once, at most `count`.
    pub fn latest(&self, count: usize) -> Vec<String> {
        let mut latest: Vec<String> = Vec::new();

        for entry in self.entries.iter().rev() {
            if latest.len() == count {
                break;
            }

            if !latest.contains(entry) {
                latest.push(entry.clone());
            }
        }

        latest
    }

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
    #[test]
    fn gives_the_latest_entries_newest_first_and_each_once() {
        let mut history = History::default();

        history.recalled(&["c".into(), "a".into(), "b".into(), "a".into()]);
        history.remember("d");

        assert_eq!(history.latest(3), ["d", "c", "a"]);
        assert_eq!(history.latest(10), ["d", "c", "a", "b"]);
    }

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

    #[test]
    fn brings_back_what_the_host_recalls_before_what_was_entered_here() {
        let mut history = History::default();

        history.remember("here");
        history.recalled(&["here".into(), "and its tests".into(), "fix the test".into()]);

        assert_eq!(history.recall(-1, "").as_deref(), Some("here"));
        assert_eq!(history.recall(-1, "here").as_deref(), Some("and its tests"));
        assert_eq!(
            history.recall(-1, "and its tests").as_deref(),
            Some("fix the test")
        );
        assert_eq!(history.recall(-1, "fix the test"), None);
    }

    #[test]
    fn keeps_the_entry_shown_when_the_recall_arrives() {
        let mut history = History::default();

        history.remember("one");
        history.remember("two");

        assert_eq!(history.recall(-1, "draft").as_deref(), Some("two"));

        history.recalled(&["older".into()]);

        assert_eq!(history.recall(-1, "two").as_deref(), Some("one"));
        assert_eq!(history.recall(-1, "one").as_deref(), Some("older"));
        assert_eq!(history.recall(1, "older").as_deref(), Some("one"));
        assert_eq!(history.recall(1, "one").as_deref(), Some("two"));
        assert_eq!(history.recall(1, "two").as_deref(), Some("draft"));
    }
}
