//! The order the window lists a project's sessions in. The host lists them
//! most recently used first, so a session would move to the top each time a
//! run works in it, and the list would jump under the person's eyes. The
//! window keeps the order it listed them in instead: a session listed again
//! stays where it was, one listed for the first time comes first, as the
//! newest, and one the host no longer lists is gone.

use std::collections::HashMap;

use crate::protocol::Value;

/// The sessions listed `now`, as the host says them, in the order of those
/// listed `before`: the ones new to the list first, in the host's order,
/// then the others where they were.
pub fn kept(before: &[Value], now: Vec<Value>) -> Vec<Value> {
    let places: HashMap<&str, usize> = before
        .iter()
        .enumerate()
        .map(|(place, listed)| (listed.value.as_str(), place))
        .collect();

    let (mut first, mut rest): (Vec<Value>, Vec<Value>) = now
        .into_iter()
        .partition(|listed| !places.contains_key(listed.value.as_str()));

    rest.sort_by_key(|listed| places[listed.value.as_str()]);
    first.extend(rest);
    first
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(ids: &[&str]) -> Vec<Value> {
        ids.iter()
            .map(|id| Value {
                value: id.to_string(),
                label: format!("title of {id}"),
                description: String::new(),
            })
            .collect()
    }

    fn ids(values: &[Value]) -> Vec<&str> {
        values.iter().map(|listed| listed.value.as_str()).collect()
    }

    #[test]
    fn takes_the_hosts_order_the_first_time() {
        let kept = kept(&[], listed(&["c", "b", "a"]));

        assert_eq!(ids(&kept), ["c", "b", "a"]);
    }

    #[test]
    fn keeps_a_session_used_again_where_it_was() {
        let before = listed(&["c", "b", "a"]);
        let kept = kept(&before, listed(&["a", "c", "b"]));

        assert_eq!(ids(&kept), ["c", "b", "a"]);
    }

    #[test]
    fn puts_new_sessions_first_in_the_hosts_order() {
        let before = listed(&["b", "a"]);
        let kept = kept(&before, listed(&["e", "a", "d", "b"]));

        assert_eq!(ids(&kept), ["e", "d", "b", "a"]);
    }

    #[test]
    fn drops_the_sessions_no_longer_listed() {
        let before = listed(&["c", "b", "a"]);
        let kept = kept(&before, listed(&["a", "c"]));

        assert_eq!(ids(&kept), ["c", "a"]);
    }

    #[test]
    fn shows_what_the_host_says_of_each_now() {
        let before = listed(&["b", "a"]);
        let mut now = listed(&["a", "b"]);

        now[0].label = "renamed".into();

        let kept = kept(&before, now);

        assert_eq!(kept[1].label, "renamed");
    }
}
