use gpui::{
    AnyElement, AppContext as _, Context, Entity, ParentElement as _, Styled as _, Window, div, px,
};
use ui::{Diff, DiffFile, DiffState};

use super::{Demos, section};
use crate::Overview;

const PATCH: &str = "\
diff --git a/src/main.rs b/src/main.rs
index 3b18e51..a0b1c2d 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,7 +1,10 @@
 use std::process::ExitCode;
 
 fn main() -> ExitCode {
-    let config = config::parse(std::env::args());
+    let config = match config::parse(std::env::args().skip(1)) {
+        Ok(config) => config,
+        Err(problem) => return say(problem),
+    };
 
-    run(config)
+    run(config).unwrap_or_else(say)
 }
@@ -40,6 +41,6 @@ fn run(config: Config) -> Result<ExitCode, String> {
     let host = Host::start(&config)?;
 
-    // Wait for the window to close.
+    // Waits for the window to close, which ends the host with it.
     host.wait()
 }
diff --git a/docs/notes.md b/docs/notes.md
new file mode 100644
index 0000000..e69de29
--- /dev/null
+++ b/docs/notes.md
@@ -0,0 +1,3 @@
+# Notes
+
+What the window keeps, and where.
diff --git a/old.txt b/old.txt
deleted file mode 100644
index e69de29..0000000
--- a/old.txt
+++ /dev/null
@@ -1,2 +0,0 @@
-Nothing here
-is needed now.
diff --git a/assets/logo.png b/assets/logo.png
index 1111111..2222222 100644
Binary files a/assets/logo.png and b/assets/logo.png differ
";

pub struct Demo {
    diff: Entity<DiffState>,
}

impl Demo {
    pub fn new(cx: &mut Context<Overview>) -> Self {
        Demo {
            diff: cx.new(|cx| DiffState::new(DiffFile::parse(PATCH), cx)),
        }
    }
}

pub fn render(demos: &mut Demos, _: &mut Window, cx: &mut Context<Overview>) -> Vec<AnyElement> {
    vec![section(
        "The changes in a folder",
        "A file changed, one added, one deleted, and one binary. A press on a file's header, or Enter with the keyboard on it, folds it.",
        div()
            .w(px(640.))
            .h(px(460.))
            .child(Diff::new("changes", &demos.diff.diff)),
        cx,
    )]
}
