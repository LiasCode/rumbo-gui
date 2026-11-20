use gpui::{AnyElement, Context, Styled as _, Window, px};
use ui::Markdown;

use super::section;
use crate::Overview;

const REPLY: &str = r#"## What changed

The window now draws with **its own components**, and `cargo test` covers each one. Read [the architecture](https://github.com/liascode/rumbo-gui) for how they fit together.

1. The *sidebar* lists the projects:
   - each with its sessions,
   - and ~~a menu~~ a menu for each.
2. The changes panel reads Git.

- [x] Draw the conversation
- [ ] Draw the settings

```rust
fn main() {
    println!("A line long enough to scroll sideways in its block, since code keeps its lines whole however narrow the conversation is.");
}
```

> Anything the host says is shown as it says it.

---

| File | Lines | Change |
|:-----|------:|:------:|
| src/main.rs | 111 | edited |
| crates/ui/src/lib.rs | 140 | new |
"#;

pub fn render(_: &mut super::Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    vec![section(
        "A reply",
        "Headings, marks, links, lists and tasks, code, a quote, a rule, and a table. Drag over a block's words, or press twice for a word and three times for all, and Ctrl-C copies them; a press on the link follows it.",
        Markdown::new("reply", REPLY).w(px(560.)),
        cx,
    )]
}
