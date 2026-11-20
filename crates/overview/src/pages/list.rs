use gpui::{AnyElement, Context, ParentElement as _, Styled as _, Window, px};
use ui::{List, ListItem, ListMarker, Sizable as _, h_flex};

use super::section;
use crate::Overview;

/// Items with these words.
fn items(words: &[&'static str]) -> Vec<ListItem> {
    words.iter().map(|words| ListItem::new(*words)).collect()
}

pub fn render(_: &mut super::Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    let numbered = h_flex()
        .items_start()
        .gap_16()
        .child(List::ordered().children(items(&[
            "Pull the latest changes",
            "Run the checks of the whole workspace",
            "Build for release",
        ])))
        .child(List::ordered().start(8).children(items(&[
            "Look at the window",
            "Write what changed",
            "Ask for a review",
            "Make the commit",
            "Push it",
        ])));

    let counted = h_flex()
        .items_start()
        .gap_16()
        .child(List::new(ListMarker::LowerAlpha).children(items(&[
            "A provider",
            "A model",
            "Its options",
        ])))
        .child(List::new(ListMarker::UpperRoman).start(3).children(items(&[
            "Architecture",
            "Domain",
            "Glossary",
            "Use cases",
        ])));

    let bullets = h_flex()
        .items_start()
        .gap_16()
        .child(List::bullet().w(px(320.)).children([
            ListItem::new("The sidebar, with the projects open and those rumbo holds"),
            ListItem::new("The workspace of the project shown").nested(List::bullet().children([
                ListItem::new("Its header, with the session's menu"),
                ListItem::new("The conversation").nested(
                    List::bullet().children(items(&["What was asked", "What the model wrote"])),
                ),
                ListItem::new("The composer"),
            ])),
            ListItem::new("The panels beside it"),
        ]))
        .child(List::new(ListMarker::Dash).children(items(&[
            "Quiet, for a list beside other text",
            "Of the same weight as a bullet",
        ])));

    let tasks = List::tasks().children([
        ListItem::new("Create the overview").done(true),
        ListItem::new("Build every component on GPUI alone").done(true),
        ListItem::new("Move the window's views to them"),
        ListItem::new("Drop GPUI Kit"),
    ]);

    let mixed = List::ordered().w(px(480.)).children([
        ListItem::new("Add a provider that needs no model:").nested(
            List::new(ListMarker::Dash)
                .child(ListItem::new("rumbo provider add toy python3 examples/provider.py")),
        ),
        ListItem::new(
            "Start the window on a folder in Git, with the scratch configuration and data, so the person's own sessions stay untouched.",
        ),
        ListItem::new("Look at it."),
    ]);

    let small = List::ordered()
        .small()
        .children(items(&["Small", "Lists", "Of small words"]));

    vec![
        section(
            "Numbered",
            "Counted from where it starts. The numbers line up at their end, so 9. and 10. take the same room.",
            numbered,
            cx,
        ),
        section("Letters and numerals", "", counted, cx),
        section(
            "Bullets and dashes",
            "A list in an item is set in under its words, and its bullet changes with how deep it is.",
            bullets,
            cx,
        ),
        section(
            "Tasks",
            "A box before each item, ticked and the words struck out for one done.",
            tasks,
            cx,
        ),
        section(
            "Long items and lists in them",
            "Words that do not fit wrap under themselves, past the mark.",
            mixed,
            cx,
        ),
        section("Small", "", small, cx),
    ]
}
