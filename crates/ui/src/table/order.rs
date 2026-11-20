//! The order a table's rows are shown in, and how the keys move the
//! selection through them, apart from drawing them.

use std::cmp::Ordering;

use gpui::SharedString;

/// Which way a column sorts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// A column a table is sorted by, by its place, and which way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Sort {
    pub(crate) column: usize,
    pub(crate) direction: SortDirection,
}

/// What pressing a column's title makes of the sort: a column not sorted
/// by sorts ascending, then descending, then not at all.
pub(crate) fn pressed(sort: Option<Sort>, column: usize) -> Option<Sort> {
    match sort {
        Some(Sort {
            column: sorted,
            direction: SortDirection::Ascending,
        }) if sorted == column => Some(Sort {
            column,
            direction: SortDirection::Descending,
        }),
        Some(Sort {
            column: sorted,
            direction: SortDirection::Descending,
        }) if sorted == column => None,
        _ => Some(Sort {
            column,
            direction: SortDirection::Ascending,
        }),
    }
}

/// Compares two cells as a person reads them: numbers by their value, even
/// with commas between thousands, and before any other cell; and the rest
/// without case and with the numbers in them by value, so "Session 9" comes
/// before "Session 10". Every cell has its place, as sorting needs: a
/// number compared as words would put "1.5" after "1.10".
pub(crate) fn compare(a: &str, b: &str) -> Ordering {
    match (number(a), number(b)) {
        (Some(a), Some(b)) => a.total_cmp(&b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => compare_words(a, b),
    }
}

fn compare_words(a: &str, b: &str) -> Ordering {
    let mut a = Chunks(a);
    let mut b = Chunks(b);

    loop {
        let order = match (a.next(), b.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(a), Some(b)) => compare_chunks(a, b),
        };

        if order != Ordering::Equal {
            return order;
        }
    }
}

/// The value of a cell that is a number and nothing else.
fn number(cell: &str) -> Option<f64> {
    let digits: String = cell.trim().chars().filter(|c| *c != ',').collect();

    digits.parse::<f64>().ok().filter(|value| value.is_finite())
}

/// A run of digits, or of anything else.
struct Chunks<'a>(&'a str);

impl<'a> Iterator for Chunks<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let first = self.0.chars().next()?;
        let digits = first.is_ascii_digit();
        let end = self
            .0
            .find(|c: char| c.is_ascii_digit() != digits)
            .unwrap_or(self.0.len());
        let (chunk, rest) = self.0.split_at(end);

        self.0 = rest;
        Some(chunk)
    }
}

fn compare_chunks(a: &str, b: &str) -> Ordering {
    let digits = |chunk: &str| chunk.starts_with(|c: char| c.is_ascii_digit());

    match (digits(a), digits(b)) {
        // By value: without the zeros before it, the longer is the greater.
        (true, true) => {
            let a = a.trim_start_matches('0');
            let b = b.trim_start_matches('0');

            a.len().cmp(&b.len()).then_with(|| a.cmp(b))
        }
        (false, false) => a.to_lowercase().cmp(&b.to_lowercase()),
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
    }
}

/// The places of the rows in the order shown: as given, or by a column,
/// with rows that tie kept in the order given. A row without the column's
/// cell sorts as an empty one.
pub(crate) fn order<Row: AsRef<[SharedString]>>(rows: &[Row], sort: Option<Sort>) -> Vec<usize> {
    let mut places: Vec<usize> = (0..rows.len()).collect();

    let Some(sort) = sort else {
        return places;
    };

    let cell = |row: usize| {
        rows[row]
            .as_ref()
            .get(sort.column)
            .map(|cell| cell.as_ref())
            .unwrap_or_default()
    };

    places.sort_by(|a, b| {
        let order = compare(cell(*a), cell(*b));

        match sort.direction {
            SortDirection::Ascending => order,
            SortDirection::Descending => order.reverse(),
        }
    });

    places
}

/// A key that moves the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Move {
    Previous,
    Next,
    PageUp,
    PageDown,
    First,
    Last,
}

/// Where a key takes the selection, by its place among `count` rows shown,
/// `page` of them at a time. With none selected, going down starts at the
/// first and going up at the last.
pub(crate) fn moved(at: Option<usize>, count: usize, page: usize, step: Move) -> Option<usize> {
    let last = count.checked_sub(1)?;
    let page = page.max(1);

    let to = match (at, step) {
        (_, Move::First) => 0,
        (_, Move::Last) => last,
        (None, Move::Next | Move::PageDown) => 0,
        (None, Move::Previous | Move::PageUp) => last,
        (Some(at), Move::Previous) => at.saturating_sub(1),
        (Some(at), Move::Next) => at + 1,
        (Some(at), Move::PageUp) => at.saturating_sub(page),
        (Some(at), Move::PageDown) => at + page,
    };

    Some(to.min(last))
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use gpui::SharedString;

    use super::{Move, Sort, SortDirection, compare, moved, order, pressed};

    fn sorted(column: usize, direction: SortDirection) -> Option<Sort> {
        Some(Sort { column, direction })
    }

    #[test]
    fn sorts_a_column_up_then_down_then_not() {
        let up = sorted(1, SortDirection::Ascending);
        let down = sorted(1, SortDirection::Descending);

        assert_eq!(pressed(None, 1), up);
        assert_eq!(pressed(up, 1), down);
        assert_eq!(pressed(down, 1), None);

        // Another column starts ascending.
        assert_eq!(pressed(down, 0), sorted(0, SortDirection::Ascending));
    }

    #[test]
    fn compares_numbers_by_their_value() {
        assert_eq!(compare("9", "10"), Ordering::Less);
        assert_eq!(compare("0.0042", "0.012"), Ordering::Less);
        assert_eq!(compare("1,250", "980"), Ordering::Greater);
        assert_eq!(compare("-3", "2"), Ordering::Less);
        assert_eq!(compare(" 7 ", "7"), Ordering::Equal);

        // Before what is not a number, so no three cells go round.
        assert_eq!(compare("1.5", "1.10"), Ordering::Greater);
        assert_eq!(compare("1.10", "1.7x"), Ordering::Less);
        assert_eq!(compare("1.5", "1.7x"), Ordering::Less);
        assert_eq!(compare("", "2"), Ordering::Greater);
    }

    #[test]
    fn compares_words_without_case_and_their_numbers_by_value() {
        assert_eq!(compare("Session 9", "Session 10"), Ordering::Less);
        assert_eq!(compare("apple", "Banana"), Ordering::Less);
        assert_eq!(compare("Model", "model"), Ordering::Equal);
        assert_eq!(compare("v007", "v7"), Ordering::Equal);
        assert_eq!(compare("12 KB", "3 KB"), Ordering::Greater);

        // What is shorter and alike comes first, and so does the empty.
        assert_eq!(compare("run", "run 2"), Ordering::Less);
        assert_eq!(compare("", "a"), Ordering::Less);
    }

    fn rows(cells: &[&[&str]]) -> Vec<Vec<SharedString>> {
        cells
            .iter()
            .map(|row| {
                row.iter()
                    .map(|cell| SharedString::from(cell.to_string()))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn orders_rows_by_a_column_keeping_ties_as_given() {
        let rows = rows(&[&["b", "2"], &["a", "10"], &["c", "2"], &["d"]]);

        assert_eq!(order(&rows, None), [0, 1, 2, 3]);
        assert_eq!(
            order(&rows, sorted(1, SortDirection::Ascending)),
            [0, 2, 1, 3]
        );
        assert_eq!(
            order(&rows, sorted(1, SortDirection::Descending)),
            [3, 1, 0, 2]
        );
        assert_eq!(
            order(&rows, sorted(0, SortDirection::Descending)),
            [3, 2, 0, 1]
        );
    }

    #[test]
    fn moves_the_selection_within_the_rows() {
        assert_eq!(moved(Some(3), 10, 4, Move::Next), Some(4));
        assert_eq!(moved(Some(9), 10, 4, Move::Next), Some(9));
        assert_eq!(moved(Some(0), 10, 4, Move::Previous), Some(0));
        assert_eq!(moved(Some(3), 10, 4, Move::PageDown), Some(7));
        assert_eq!(moved(Some(8), 10, 4, Move::PageDown), Some(9));
        assert_eq!(moved(Some(3), 10, 4, Move::PageUp), Some(0));
        assert_eq!(moved(Some(3), 10, 4, Move::First), Some(0));
        assert_eq!(moved(Some(3), 10, 4, Move::Last), Some(9));
    }

    #[test]
    fn starts_a_selection_at_the_end_it_goes_toward() {
        assert_eq!(moved(None, 10, 4, Move::Next), Some(0));
        assert_eq!(moved(None, 10, 4, Move::Previous), Some(9));
        assert_eq!(moved(None, 0, 4, Move::Next), None);

        // A selection past rows that went away comes back to the last.
        assert_eq!(moved(Some(20), 10, 4, Move::Previous), Some(9));
    }
}
