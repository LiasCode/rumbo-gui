//! Where the person has been in the window, to go back and forward between
//! what it showed, as a browser does. Each place the person goes to comes
//! after the one shown, and going somewhere after going back drops the
//! places ahead. Going back or forward passes over places that can no
//! longer be shown, such as a session deleted, and over the place shown.

/// How many places are kept; the oldest go first.
const KEPT: usize = 100;

#[derive(Debug)]
pub struct Navigation<T> {
    places: Vec<T>,
    /// Where the place shown is in `places`.
    at: usize,
}

impl<T> Default for Navigation<T> {
    fn default() -> Self {
        Navigation {
            places: Vec::new(),
            at: 0,
        }
    }
}

impl<T: Clone + PartialEq> Navigation<T> {
    /// The place shown, once there is one.
    pub fn current(&self) -> Option<&T> {
        self.places.get(self.at)
    }

    /// Records a place the person went to. Going to the place shown records
    /// nothing.
    pub fn visit(&mut self, place: T) {
        if self.current() == Some(&place) {
            return;
        }

        if !self.places.is_empty() {
            self.places.truncate(self.at + 1);
        }

        self.places.push(place);

        if self.places.len() > KEPT {
            self.places.remove(0);
        }

        self.at = self.places.len() - 1;
    }

    /// Records that the place shown is now `place` without the person
    /// going anywhere, as when a new session gets its id.
    pub fn replace(&mut self, place: T) {
        match self.places.get_mut(self.at) {
            Some(current) => *current = place,
            None => self.visit(place),
        }
    }

    /// Whether there is a place to go back to.
    pub fn can_go_back(&self, can_show: impl Fn(&T) -> bool) -> bool {
        self.behind(can_show).is_some()
    }

    /// Whether there is a place to go forward to.
    pub fn can_go_forward(&self, can_show: impl Fn(&T) -> bool) -> bool {
        self.ahead(can_show).is_some()
    }

    /// Goes back to the nearest place before the one shown, and gives it.
    pub fn back(&mut self, can_show: impl Fn(&T) -> bool) -> Option<T> {
        let found = self.behind(can_show)?;

        self.at = found;
        Some(self.places[found].clone())
    }

    /// Goes forward to the nearest place after the one shown, and gives it.
    pub fn forward(&mut self, can_show: impl Fn(&T) -> bool) -> Option<T> {
        let found = self.ahead(can_show)?;

        self.at = found;
        Some(self.places[found].clone())
    }

    fn behind(&self, can_show: impl Fn(&T) -> bool) -> Option<usize> {
        let current = self.current()?;

        (0..self.at)
            .rev()
            .find(|&index| self.places[index] != *current && can_show(&self.places[index]))
    }

    fn ahead(&self, can_show: impl Fn(&T) -> bool) -> Option<usize> {
        let current = self.current()?;

        (self.at + 1..self.places.len())
            .find(|&index| self.places[index] != *current && can_show(&self.places[index]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn any(_: &&str) -> bool {
        true
    }

    fn visited(places: &[&'static str]) -> Navigation<&'static str> {
        let mut navigation = Navigation::default();

        for place in places {
            navigation.visit(*place);
        }

        navigation
    }

    #[test]
    fn goes_back_and_forward_between_the_places_visited() {
        let mut navigation = visited(&["a", "b", "c"]);

        assert_eq!(navigation.back(any), Some("b"));
        assert_eq!(navigation.back(any), Some("a"));
        assert_eq!(navigation.back(any), None);
        assert_eq!(navigation.current(), Some(&"a"));

        assert_eq!(navigation.forward(any), Some("b"));
        assert_eq!(navigation.forward(any), Some("c"));
        assert_eq!(navigation.forward(any), None);
    }

    #[test]
    fn says_whether_there_is_somewhere_to_go() {
        let mut navigation = visited(&[]);

        assert!(!navigation.can_go_back(any));
        assert!(!navigation.can_go_forward(any));

        navigation.visit("a");
        assert!(!navigation.can_go_back(any));

        navigation.visit("b");
        assert!(navigation.can_go_back(any));
        assert!(!navigation.can_go_forward(any));

        navigation.back(any);
        assert!(!navigation.can_go_back(any));
        assert!(navigation.can_go_forward(any));
    }

    #[test]
    fn going_somewhere_after_going_back_drops_the_places_ahead() {
        let mut navigation = visited(&["a", "b", "c"]);

        navigation.back(any);
        navigation.back(any);
        navigation.visit("d");

        assert!(!navigation.can_go_forward(any));
        assert_eq!(navigation.back(any), Some("a"));
        assert_eq!(navigation.back(any), None);
    }

    #[test]
    fn going_to_the_place_shown_records_nothing() {
        let mut navigation = visited(&["a", "b", "b"]);

        assert_eq!(navigation.back(any), Some("a"));
        assert_eq!(navigation.back(any), None);
    }

    #[test]
    fn a_place_shown_under_a_new_name_is_the_same_place() {
        let mut navigation = visited(&["a", "new"]);

        navigation.replace("named");

        assert_eq!(navigation.current(), Some(&"named"));
        assert_eq!(navigation.back(any), Some("a"));
        assert_eq!(navigation.forward(any), Some("named"));
    }

    #[test]
    fn passes_over_places_that_cannot_be_shown() {
        let mut navigation = visited(&["a", "gone", "b"]);
        let showable = |place: &&str| *place != "gone";

        assert_eq!(navigation.back(showable), Some("a"));
        assert_eq!(navigation.forward(showable), Some("b"));
    }

    #[test]
    fn passes_over_the_place_shown() {
        // With the session between them deleted, going back from a would
        // show a again.
        let mut navigation = visited(&["a", "gone", "a"]);
        let showable = |place: &&str| *place != "gone";

        assert!(!navigation.can_go_back(showable));
        assert_eq!(navigation.back(showable), None);
    }

    #[test]
    fn keeps_the_latest_places() {
        let mut navigation = Navigation::default();

        for place in 0..KEPT + 10 {
            navigation.visit(place);
        }

        let mut oldest = KEPT + 9;

        while let Some(place) = navigation.back(|_| true) {
            oldest = place;
        }

        assert_eq!(oldest, 10);
    }
}
