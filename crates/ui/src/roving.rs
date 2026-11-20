//! Where a key moves the choice in a group that takes the keyboard as one
//! stop, as a row of tabs and a group of radios do: to the option beside,
//! coming round at the ends, or to the first or the last, past those
//! disabled.

/// A key that moves the choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    Previous,
    Next,
    First,
    Last,
}

/// The option a key goes to from `chosen`, among options that are
/// `enabled` or not; none when it stays. With none chosen, going on starts
/// at the first and going back at the last.
pub(crate) fn moved(chosen: Option<usize>, enabled: &[bool], step: Step) -> Option<usize> {
    let count = enabled.len();
    let open = |index: &usize| enabled[*index];

    let to = match (chosen, step) {
        (_, Step::First) | (None, Step::Next) => (0..count).find(open),
        (_, Step::Last) | (None, Step::Previous) => (0..count).rev().find(open),
        (Some(chosen), Step::Next) => (1..count).map(|ahead| (chosen + ahead) % count).find(open),
        (Some(chosen), Step::Previous) => (1..count)
            .map(|behind| (chosen + count - behind) % count)
            .find(open),
    };

    to.filter(|to| Some(*to) != chosen)
}

#[cfg(test)]
mod tests {
    use super::{Step, moved};

    #[test]
    fn goes_to_the_option_beside_coming_round_at_the_ends() {
        let open = [true; 4];

        assert_eq!(moved(Some(1), &open, Step::Next), Some(2));
        assert_eq!(moved(Some(3), &open, Step::Next), Some(0));
        assert_eq!(moved(Some(0), &open, Step::Previous), Some(3));
        assert_eq!(moved(Some(2), &open, Step::Previous), Some(1));
    }

    #[test]
    fn passes_the_options_disabled() {
        let open = [true, false, false, true, false];

        assert_eq!(moved(Some(0), &open, Step::Next), Some(3));
        assert_eq!(moved(Some(3), &open, Step::Next), Some(0));
        assert_eq!(moved(Some(0), &open, Step::Previous), Some(3));
        assert_eq!(moved(Some(3), &open, Step::Last), None);
        assert_eq!(moved(Some(3), &open, Step::First), Some(0));

        // A group with one option to choose stays.
        assert_eq!(moved(Some(0), &[true, false], Step::Next), None);
    }

    #[test]
    fn goes_to_the_first_and_the_last() {
        let open = [false, true, true, false];

        assert_eq!(moved(Some(2), &open, Step::First), Some(1));
        assert_eq!(moved(Some(1), &open, Step::Last), Some(2));
        assert_eq!(moved(Some(0), &[], Step::Next), None);
    }

    #[test]
    fn starts_at_the_end_it_goes_toward_with_none_chosen() {
        let open = [false, true, true, false];

        assert_eq!(moved(None, &open, Step::Next), Some(1));
        assert_eq!(moved(None, &open, Step::Previous), Some(2));
        assert_eq!(moved(None, &[false, false], Step::Next), None);
    }
}
