//! The toasts a window shows, apart from drawing them: which are shown, in
//! the order they came, and which go once their time is up.

/// What tells a toast from the others a window shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ToastId(pub(crate) u64);

/// The most shown at once: one more pushes out the oldest.
pub(crate) const MOST: usize = 5;

/// One toast shown.
pub(crate) struct Entry<T> {
    pub(crate) id: ToastId,
    pub(crate) item: T,
    /// Its time is up, but the pointer holds the toasts.
    expired: bool,
}

pub(crate) struct Queue<T> {
    next: u64,
    shown: Vec<Entry<T>>,
    /// The pointer is on the toasts, and none goes for its time.
    held: bool,
}

impl<T> Queue<T> {
    pub(crate) fn new() -> Self {
        Queue {
            next: 0,
            shown: Vec::new(),
            held: false,
        }
    }

    /// The toasts shown, oldest first.
    pub(crate) fn shown(&self) -> &[Entry<T>] {
        &self.shown
    }

    /// Shows `item` after the others, pushing out the oldest beyond the
    /// most.
    pub(crate) fn push(&mut self, item: T) -> ToastId {
        let id = ToastId(self.next);

        self.next += 1;
        self.shown.push(Entry {
            id,
            item,
            expired: false,
        });

        if self.shown.len() > MOST {
            self.shown.remove(0);
        }

        id
    }

    /// Takes the toast `id` away; whether it was shown.
    pub(crate) fn dismiss(&mut self, id: ToastId) -> bool {
        let before = self.shown.len();

        self.shown.retain(|entry| entry.id != id);
        self.settle();
        self.shown.len() != before
    }

    pub(crate) fn clear(&mut self) {
        self.shown.clear();
        self.settle();
    }

    /// With none shown nothing is drawn to tell when the pointer leaves, so
    /// the pointer holds nothing.
    fn settle(&mut self) {
        if self.shown.is_empty() {
            self.held = false;
        }
    }

    /// The time of the toast `id` is up: it goes, or, while the pointer
    /// holds the toasts, it goes when the pointer leaves. Whether it went.
    pub(crate) fn expire(&mut self, id: ToastId) -> bool {
        if !self.held {
            return self.dismiss(id);
        }

        if let Some(entry) = self.shown.iter_mut().find(|entry| entry.id == id) {
            entry.expired = true;
        }

        false
    }

    /// The pointer came onto the toasts, or left them, when those whose
    /// time was up go. Whether any went.
    pub(crate) fn hold(&mut self, held: bool) -> bool {
        self.held = held;

        if held {
            return false;
        }

        let before = self.shown.len();

        self.shown.retain(|entry| !entry.expired);
        self.shown.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::{MOST, Queue};

    fn shown(queue: &Queue<&'static str>) -> Vec<&'static str> {
        queue.shown().iter().map(|entry| entry.item).collect()
    }

    #[test]
    fn shows_toasts_in_the_order_they_came() {
        let mut queue = Queue::new();
        let first = queue.push("first");
        let second = queue.push("second");

        assert_ne!(first, second);
        assert_eq!(shown(&queue), ["first", "second"]);

        assert!(queue.dismiss(first));
        assert!(!queue.dismiss(first));
        assert_eq!(shown(&queue), ["second"]);
    }

    #[test]
    fn pushes_out_the_oldest_beyond_the_most() {
        let mut queue = Queue::new();
        let names = ["a", "b", "c", "d", "e", "f", "g"];

        for name in names {
            queue.push(name);
        }

        assert_eq!(shown(&queue), names[names.len() - MOST..]);
    }

    #[test]
    fn lets_a_toast_go_when_its_time_is_up() {
        let mut queue = Queue::new();
        let id = queue.push("done");

        assert!(queue.expire(id));
        assert!(shown(&queue).is_empty());

        // A toast already put away goes no further.
        assert!(!queue.expire(id));
    }

    #[test]
    fn keeps_the_toasts_the_pointer_is_on() {
        let mut queue = Queue::new();
        let first = queue.push("first");

        queue.push("second");
        queue.hold(true);

        assert!(!queue.expire(first));
        assert_eq!(shown(&queue), ["first", "second"]);

        // Leaving, the one whose time was up goes, and the other stays.
        assert!(queue.hold(false));
        assert_eq!(shown(&queue), ["second"]);
        assert!(!queue.hold(false));
    }

    #[test]
    fn lets_go_of_the_pointer_once_no_toast_is_shown() {
        let mut queue = Queue::new();
        let first = queue.push("first");

        // The last one put away under the pointer is no longer drawn, so
        // it is never told the pointer left.
        queue.hold(true);
        queue.dismiss(first);

        let next = queue.push("next");

        assert!(queue.expire(next));
    }
}
