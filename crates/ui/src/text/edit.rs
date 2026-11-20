//! The text of a field and what is selected in it, and every edit and move
//! that does not depend on how the text is laid out. It knows no GPUI, so
//! it is tested alone. Offsets are bytes of UTF-8, always at the boundary
//! of a grapheme, which is what a person takes for a character.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation as _;

/// What a cursor moves by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Left,
    Right,
    WordLeft,
    WordRight,
    LineStart,
    LineEnd,
    Start,
    End,
}

/// What an edit was, so that typing, or deleting, a run of characters is
/// undone at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKind {
    Typing,
    Deleting,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Snapshot {
    text: String,
    selection: Range<usize>,
    reversed: bool,
}

#[derive(Debug, Default)]
pub struct Buffer {
    text: String,
    /// Where the cursor is, at `selection.end`, or at `selection.start` when
    /// `reversed`; the other end is where the selection was started.
    selection: Range<usize>,
    reversed: bool,
    /// Text an input method is composing, not yet entered.
    marked: Option<Range<usize>>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// The kind of the edit before, until the cursor moves.
    last_edit: Option<EditKind>,
}

/// How many steps can be undone.
const UNDO_STEPS: usize = 200;

impl Buffer {
    pub fn new(text: &str) -> Self {
        Buffer {
            text: text.to_string(),
            selection: text.len()..text.len(),
            ..Buffer::default()
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn selection(&self) -> Range<usize> {
        self.selection.clone()
    }

    pub fn reversed(&self) -> bool {
        self.reversed
    }

    pub fn cursor(&self) -> usize {
        match self.reversed {
            true => self.selection.start,
            false => self.selection.end,
        }
    }

    pub fn marked(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }

    pub fn selected_text(&self) -> &str {
        &self.text[self.selection.clone()]
    }

    /// Replaces the whole text, as a program sets it rather than a person
    /// types it: the cursor goes to its end, and nothing before can be
    /// undone.
    pub fn set_text(&mut self, text: &str) {
        *self = Buffer::new(text);
    }

    /// Puts the cursor at `offset`, selecting nothing.
    pub fn move_to(&mut self, offset: usize) {
        let offset = self.clamp(offset);

        self.selection = offset..offset;
        self.reversed = false;
        self.last_edit = None;
    }

    /// Moves the cursor to `offset`, keeping where the selection started.
    pub fn select_to(&mut self, offset: usize) {
        let offset = self.clamp(offset);
        let anchor = match self.reversed {
            true => self.selection.end,
            false => self.selection.start,
        };

        self.reversed = offset < anchor;
        self.selection = anchor.min(offset)..anchor.max(offset);
        self.last_edit = None;
    }

    pub fn select_range(&mut self, range: Range<usize>, reversed: bool) {
        let start = self.clamp(range.start);
        let end = self.clamp(range.end).max(start);

        self.selection = start..end;
        self.reversed = reversed;
        self.last_edit = None;
    }

    pub fn select_all(&mut self) {
        self.select_range(0..self.text.len(), false);
    }

    /// Selects the word, the run of spaces, or the mark at `offset`, as a
    /// double click does.
    pub fn select_word_at(&mut self, offset: usize) {
        let offset = self.clamp(offset);
        let class = match self.text[offset..].chars().next() {
            Some(next) => Class::of(next),
            None => match self.text[..offset].chars().next_back() {
                Some(before) => Class::of(before),
                None => return,
            },
        };

        let start = self.text[..offset]
            .char_indices()
            .rev()
            .take_while(|(_, c)| Class::of(*c) == class)
            .last()
            .map_or(offset, |(index, _)| index);
        let end = self.text[offset..]
            .char_indices()
            .find(|(_, c)| Class::of(*c) != class)
            .map_or(self.text.len(), |(index, _)| offset + index);

        self.select_range(start..end, false);
    }

    /// Selects the logical line at `offset`, as a triple click does.
    pub fn select_line_at(&mut self, offset: usize) {
        let start = self.line_start(offset);
        let end = self.line_end(offset);

        self.select_range(start..end, false);
    }

    /// Where `motion` takes the cursor from where it is.
    pub fn target(&self, motion: Motion) -> usize {
        let cursor = self.cursor();

        match motion {
            Motion::Left => self.previous_grapheme(cursor),
            Motion::Right => self.next_grapheme(cursor),
            Motion::WordLeft => self.previous_word(cursor),
            Motion::WordRight => self.next_word(cursor),
            Motion::LineStart => self.line_start(cursor),
            Motion::LineEnd => self.line_end(cursor),
            Motion::Start => 0,
            Motion::End => self.text.len(),
        }
    }

    /// Moves the cursor, or with `select` extends the selection. Left or
    /// Right with a selection and without `select` goes to its edge, as
    /// people expect.
    pub fn move_by(&mut self, motion: Motion, select: bool) {
        if select {
            self.select_to(self.target(motion));
            return;
        }

        match motion {
            Motion::Left if !self.selection.is_empty() => self.move_to(self.selection.start),
            Motion::Right if !self.selection.is_empty() => self.move_to(self.selection.end),
            _ => self.move_to(self.target(motion)),
        }
    }

    /// Replaces `range`, or the text being composed, or the selection, with
    /// `text`, leaving the cursor after it.
    pub fn replace(&mut self, range: Option<Range<usize>>, text: &str, kind: EditKind) {
        let range = self.edited(range);

        self.remember(kind, range.start);
        self.text.replace_range(range.clone(), text);

        let end = range.start + text.len();

        self.selection = end..end;
        self.reversed = false;
        self.marked = None;
        self.last_edit = Some(kind);
    }

    /// Replaces as `replace` does, with text an input method is composing,
    /// which stays marked; `selected` is what to select of it, from its
    /// start.
    pub fn replace_and_mark(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
    ) {
        let range = self.edited(range);

        self.remember(EditKind::Typing, range.start);
        self.text.replace_range(range.clone(), text);

        self.marked = (!text.is_empty()).then(|| range.start..range.start + text.len());
        let (start, end) = match selected {
            Some(selected) => (
                range.start + selected.start.min(text.len()),
                range.start + selected.end.min(text.len()),
            ),
            None => (range.start + text.len(), range.start + text.len()),
        };

        self.selection = self.clamp(start)..self.clamp(end).max(self.clamp(start));
        self.reversed = false;
        self.last_edit = Some(EditKind::Typing);
    }

    /// Takes what an input method composed as entered.
    pub fn unmark(&mut self) {
        self.marked = None;
    }

    /// Deletes the selection, or what is before the cursor: a character, or
    /// with `word` the word. False when there was nothing to delete.
    pub fn delete_backward(&mut self, word: bool) -> bool {
        if self.selection.is_empty() {
            let cursor = self.cursor();
            let start = match word {
                true => self.previous_word(cursor),
                false => self.previous_grapheme(cursor),
            };

            if start == cursor {
                return false;
            }

            self.replace(Some(start..cursor), "", EditKind::Deleting);
            return true;
        }

        self.replace(None, "", EditKind::Other);
        true
    }

    /// Deletes the selection, or what is after the cursor: a character, or
    /// with `word` the word. False when there was nothing to delete.
    pub fn delete_forward(&mut self, word: bool) -> bool {
        if self.selection.is_empty() {
            let cursor = self.cursor();
            let end = match word {
                true => self.next_word(cursor),
                false => self.next_grapheme(cursor),
            };

            if end == cursor {
                return false;
            }

            self.replace(Some(cursor..end), "", EditKind::Deleting);
            return true;
        }

        self.replace(None, "", EditKind::Other);
        true
    }

    /// Goes back to before the last step. False when there is none.
    pub fn undo(&mut self) -> bool {
        let Some(before) = self.undo.pop() else {
            return false;
        };

        let now = self.snapshot();

        self.redo.push(now);
        self.restore(before);
        true
    }

    /// Does again the step undone last. False when there is none.
    pub fn redo(&mut self) -> bool {
        let Some(after) = self.redo.pop() else {
            return false;
        };

        let now = self.snapshot();

        self.undo.push(now);
        self.restore(after);
        true
    }

    pub fn previous_grapheme(&self, offset: usize) -> usize {
        self.text[..offset]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(index, _)| index)
    }

    pub fn next_grapheme(&self, offset: usize) -> usize {
        self.text[offset..]
            .graphemes(true)
            .next()
            .map_or(offset, |grapheme| offset + grapheme.len())
    }

    /// The start of the word before `offset`, past the spaces before it.
    pub fn previous_word(&self, offset: usize) -> usize {
        let mut chars = self.text[..offset].char_indices().rev().peekable();

        while chars.next_if(|(_, c)| c.is_whitespace()).is_some() {}

        let Some(&(_, first)) = chars.peek() else {
            return 0;
        };

        let class = Class::of(first);
        let mut start = offset;

        while let Some((index, _)) = chars.next_if(|(_, c)| Class::of(*c) == class) {
            start = index;
        }

        start
    }

    /// The end of the word after `offset`, past the spaces before it.
    pub fn next_word(&self, offset: usize) -> usize {
        let rest = &self.text[offset..];
        let mut chars = rest.char_indices().peekable();

        while chars.next_if(|(_, c)| c.is_whitespace()).is_some() {}

        let Some(&(_, first)) = chars.peek() else {
            return self.text.len();
        };

        let class = Class::of(first);

        while chars.next_if(|(_, c)| Class::of(*c) == class).is_some() {}

        chars
            .peek()
            .map_or(self.text.len(), |(index, _)| offset + index)
    }

    /// The start of the logical line `offset` is on.
    pub fn line_start(&self, offset: usize) -> usize {
        self.text[..offset].rfind('\n').map_or(0, |index| index + 1)
    }

    /// The end of the logical line `offset` is on, before its line break.
    pub fn line_end(&self, offset: usize) -> usize {
        self.text[offset..]
            .find('\n')
            .map_or(self.text.len(), |index| offset + index)
    }

    pub fn offset_to_utf16(&self, offset: usize) -> usize {
        self.text[..offset.min(self.text.len())]
            .chars()
            .map(char::len_utf16)
            .sum()
    }

    pub fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf16 = 0;

        for (index, c) in self.text.char_indices() {
            if utf16 >= offset {
                return index;
            }

            utf16 += c.len_utf16();
        }

        self.text.len()
    }

    pub fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    pub fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }

    pub fn len_utf16(&self) -> usize {
        self.offset_to_utf16(self.text.len())
    }

    /// The range an edit replaces when it names none: what is being
    /// composed, else the selection.
    fn edited(&self, range: Option<Range<usize>>) -> Range<usize> {
        let range = range
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection.clone());
        let start = self.clamp(range.start);

        start..self.clamp(range.end).max(start)
    }

    /// Keeps what the text was before an edit, unless the edit goes on the
    /// one before it: more of the same typing or deleting, where it left
    /// the cursor.
    fn remember(&mut self, kind: EditKind, at: usize) {
        let goes_on = kind != EditKind::Other
            && self.last_edit == Some(kind)
            && self.selection.is_empty()
            && (at == self.cursor() || kind == EditKind::Deleting);

        if !goes_on {
            let before = self.snapshot();

            self.undo.push(before);

            if self.undo.len() > UNDO_STEPS {
                self.undo.remove(0);
            }
        }

        self.redo.clear();
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            text: self.text.clone(),
            selection: self.selection.clone(),
            reversed: self.reversed,
        }
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.text = snapshot.text;
        self.selection = snapshot.selection;
        self.reversed = snapshot.reversed;
        self.marked = None;
        self.last_edit = None;
    }

    /// The nearest grapheme boundary at or before `offset`, within the text.
    fn clamp(&self, offset: usize) -> usize {
        if offset >= self.text.len() {
            return self.text.len();
        }

        self.text
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .take_while(|index| *index <= offset)
            .last()
            .unwrap_or(0)
    }
}

/// What a character is, for moving by words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Word,
    Space,
    Mark,
}

impl Class {
    fn of(c: char) -> Class {
        if c.is_alphanumeric() || c == '_' {
            Class::Word
        } else if c.is_whitespace() {
            Class::Space
        } else {
            Class::Mark
        }
    }
}

/// What a secret field shows for `text`: a dot for each character.
pub fn mask(text: &str) -> String {
    text.graphemes(true).map(|_| MASK).collect()
}

/// Where `offset` of a secret's text falls in what `mask` shows.
pub fn masked_offset(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())].graphemes(true).count() * MASK.len_utf8()
}

/// Where an offset of what `mask` shows falls in the secret's text.
pub fn unmasked_offset(text: &str, offset: usize) -> usize {
    let characters = offset / MASK.len_utf8();

    text.grapheme_indices(true)
        .nth(characters)
        .map_or(text.len(), |(index, _)| index)
}

const MASK: char = '•';

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str, cursor: usize) -> Buffer {
        let mut buffer = Buffer::new(text);

        buffer.move_to(cursor);
        buffer
    }

    #[test]
    fn moves_by_whole_characters() {
        let buffer = at("e🙂漢", 1);

        assert_eq!(buffer.next_grapheme(1), 5);
        assert_eq!(buffer.previous_grapheme(5), 1);
        assert_eq!(buffer.previous_grapheme(0), 0);
        assert_eq!(buffer.next_grapheme(8), 8);

        // An emoji with a modifier is one character for a person.
        let flag = at("a👍🏽b", 1);

        assert_eq!(flag.next_grapheme(1), 1 + "👍🏽".len());
    }

    #[test]
    fn moves_by_words_past_the_spaces_before_them() {
        let buffer = at("let x = foo_bar(1);  ", 0);

        assert_eq!(buffer.next_word(0), 3);
        assert_eq!(buffer.next_word(3), 5);
        assert_eq!(buffer.next_word(7), 15);
        assert_eq!(buffer.next_word(15), 16);
        assert_eq!(buffer.next_word(19), 21);
        assert_eq!(buffer.previous_word(15), 8);
        assert_eq!(buffer.previous_word(8), 6);
        assert_eq!(buffer.previous_word(3), 0);
        assert_eq!(buffer.previous_word(21), 17);
    }

    #[test]
    fn collapses_a_selection_to_its_edge_before_moving() {
        let mut buffer = Buffer::new("hello world");

        buffer.select_range(2..7, false);
        buffer.move_by(Motion::Left, false);

        assert_eq!(buffer.selection(), 2..2);

        buffer.select_range(2..7, true);
        buffer.move_by(Motion::Right, false);

        assert_eq!(buffer.selection(), 7..7);
    }

    #[test]
    fn extends_a_selection_from_where_it_started() {
        let mut buffer = at("hello world", 5);

        buffer.move_by(Motion::Left, true);
        buffer.move_by(Motion::Left, true);

        assert_eq!((buffer.selection(), buffer.reversed()), (3..5, true));

        buffer.move_by(Motion::WordRight, true);
        buffer.move_by(Motion::WordRight, true);

        assert_eq!((buffer.selection(), buffer.reversed()), (5..11, false));
        assert_eq!(buffer.selected_text(), " world");
    }

    #[test]
    fn moves_by_logical_lines() {
        let mut buffer = at("one\ntwo\nthree", 5);

        assert_eq!(buffer.target(Motion::LineStart), 4);
        assert_eq!(buffer.target(Motion::LineEnd), 7);

        buffer.select_line_at(9);

        assert_eq!(buffer.selected_text(), "three");
    }

    #[test]
    fn selects_the_word_a_double_click_lands_on() {
        let mut buffer = Buffer::new("run cargo_test now");

        buffer.select_word_at(6);
        assert_eq!(buffer.selected_text(), "cargo_test");

        buffer.select_word_at(14);
        assert_eq!(buffer.selected_text(), " ");

        buffer.select_word_at(18);
        assert_eq!(buffer.selected_text(), "now");
    }

    #[test]
    fn types_over_the_selection() {
        let mut buffer = Buffer::new("hello world");

        buffer.select_range(6..11, false);
        buffer.replace(None, "rumbo", EditKind::Typing);

        assert_eq!(buffer.text(), "hello rumbo");
        assert_eq!(buffer.selection(), 11..11);
    }

    #[test]
    fn deletes_characters_and_words_on_either_side() {
        let mut buffer = at("say hello there", 9);

        assert!(buffer.delete_backward(false));
        assert_eq!(buffer.text(), "say hell there");

        assert!(buffer.delete_backward(true));
        assert_eq!(buffer.text(), "say  there");

        assert!(buffer.delete_forward(true));
        assert_eq!(buffer.text(), "say ");

        assert!(!buffer.delete_forward(false));

        buffer.move_to(0);
        assert!(!buffer.delete_backward(false));
    }

    #[test]
    fn undoes_a_run_of_typing_at_once() {
        let mut buffer = Buffer::new("");

        for c in ["h", "e", "y"] {
            buffer.replace(None, c, EditKind::Typing);
        }

        buffer.move_by(Motion::Start, false);
        buffer.replace(None, ">", EditKind::Typing);

        assert_eq!(buffer.text(), ">hey");
        assert!(buffer.undo());
        assert_eq!(buffer.text(), "hey");
        assert!(buffer.undo());
        assert_eq!(buffer.text(), "");
        assert!(!buffer.undo());
        assert!(buffer.redo());
        assert!(buffer.redo());
        assert_eq!((buffer.text(), buffer.selection()), (">hey", 1..1));
        assert!(!buffer.redo());
    }

    #[test]
    fn undoes_a_run_of_deleting_at_once() {
        let mut buffer = Buffer::new("abcd");

        buffer.delete_backward(false);
        buffer.delete_backward(false);
        buffer.replace(None, "X", EditKind::Typing);

        assert!(buffer.undo());
        assert_eq!(buffer.text(), "ab");
        assert!(buffer.undo());
        assert_eq!(buffer.text(), "abcd");
    }

    #[test]
    fn composes_with_an_input_method() {
        let mut buffer = Buffer::new("A🙂Z");

        buffer.replace_and_mark(Some(1..5), "é🙂", Some(2..6));

        assert_eq!(buffer.text(), "Aé🙂Z");
        assert_eq!(buffer.marked(), Some(1..7));
        assert_eq!(buffer.selection(), 3..7);

        buffer.replace_and_mark(None, "漢", None);

        assert_eq!(buffer.text(), "A漢Z");
        assert_eq!(buffer.marked(), Some(1..4));

        buffer.replace(None, "字", EditKind::Typing);

        assert_eq!(buffer.text(), "A字Z");
        assert_eq!(buffer.marked(), None);
    }

    #[test]
    fn speaks_utf16_to_the_platform() {
        let buffer = Buffer::new("e🙂漢");

        assert_eq!(buffer.len_utf16(), 4);
        assert_eq!(buffer.range_from_utf16(&(1..3)), 1..5);
        assert_eq!(buffer.range_to_utf16(&(5..8)), 3..4);
        assert_eq!(buffer.offset_from_utf16(99), 8);
    }

    #[test]
    fn keeps_offsets_on_character_boundaries() {
        let mut buffer = Buffer::new("e🙂漢");

        buffer.move_to(3);
        assert_eq!(buffer.cursor(), 1);

        buffer.move_to(99);
        assert_eq!(buffer.cursor(), 8);
    }

    #[test]
    fn masks_a_secret_character_by_character() {
        let secret = "pä🙂";

        assert_eq!(mask(secret), "•••");
        assert_eq!(masked_offset(secret, 3), 6);
        assert_eq!(unmasked_offset(secret, 6), 3);
        assert_eq!(unmasked_offset(secret, 9), secret.len());
    }
}
