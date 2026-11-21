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

> Anything the host says is shown as it says it.

---

| File | Lines | Change |
|:-----|------:|:------:|
| src/main.rs | 111 | edited |
| crates/ui/src/lib.rs | 140 | new |
"#;

const CODE: &str = r#"Code is colored by the language its fence names:

```rust
/// How long a run that finished took.
fn finished(seconds: u64) -> String {
    match seconds {
        0 => "in under a second".to_string(),
        seconds => format!("in {seconds}s, a line long enough to scroll sideways in its block"),
    }
}
```

```ts
const answer: number = await host.ask({ name: "status" });
```

A language it does not know, or none, is left plain:

```
$ cargo test --workspace
```
"#;

const MATH: &str = r#"The roots of $ax^2 + bx + c = 0$ are

$$
x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}
$$

and their sum is $-\frac{b}{a}$, while it costs $5 and $10 to say so.

```math
\sum_{i=1}^{n} i^2 = \frac{n(n+1)(2n+1)}{6}
```

A formula it cannot read shows as it is written: $\frac{1}{$.
"#;

const DIAGRAM: &str = r#"```mermaid
flowchart TD
    A[Task sent] --> B{Needs a tool?}
    B -- yes --> C[Run the tool]
    C --> B
    B -- no --> D[Answer streamed]
```

One still being written shows its source, and why it is not drawn yet:

```mermaid
flowchart LR
    A -->
```
"#;

/// The page's images: one in the text, one in a file of the overview's,
/// one of the web, and one that is nowhere.
fn images() -> String {
    let data = "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20width%3D%22240%22%20height%3D%2260%22%3E%3Crect%20width%3D%22240%22%20height%3D%2260%22%20rx%3D%228%22%20fill%3D%22%23f59e0b%22%2F%3E%3Ccircle%20cx%3D%2230%22%20cy%3D%2230%22%20r%3D%2214%22%20fill%3D%22%23fff%22%2F%3E%3C%2Fsvg%3E";

    format!(
        "An image in the text itself:\n\n![A badge in a data URL]({data})\n\nOne in a file, by a path from the folder the text is about:\n\n![A chart](assets/chart.png)\n\nOne of the web, which shows once asked for:\n\n![The Rust logo](https://www.rust-lang.org/logos/rust-logo-512x512.png)\n\nAnd one that is not there:\n\n![A missing chart](assets/missing.png)\n"
    )
}

pub fn render(_: &mut super::Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    vec![
        section(
            "A reply",
            "Headings, marks, links, lists and tasks, a quote, a rule, and a table. Drag over a block's words, or press twice for a word and three times for all, and Ctrl-C copies them; a press on the link follows it.",
            Markdown::new("reply", REPLY).w(px(560.)),
            cx,
        ),
        section(
            "Code",
            "Colored by syntect with the grammars bat ships, in One Half's light or dark theme.",
            Markdown::new("code", CODE).w(px(560.)),
            cx,
        ),
        section(
            "Formulas",
            "TeX between dollars, typeset by latex-rust: among words, standing on their baseline, or on a line of their own. Dollars around a price stay as they are.",
            Markdown::new("math", MATH).w(px(560.)),
            cx,
        ),
        section(
            "Diagrams",
            "A fence named mermaid, laid out and drawn by merman, the Rust Mermaid Zed uses.",
            Markdown::new("diagram", DIAGRAM).w(px(560.)),
            cx,
        ),
        section(
            "Images",
            "From the text, from a file, or from the web once the person asks, fitting the text's width.",
            Markdown::new("images", images())
                .base(env!("CARGO_MANIFEST_DIR"))
                .w(px(560.)),
            cx,
        ),
    ]
}
