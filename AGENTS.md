# rumbo-gui: agent guide

Read [docs/architecture.md](docs/architecture.md) first. The contract is
rumbo's: `docs/frontend-protocol.md` says what goes over the wire, and the
front end section of `docs/use-cases.md` what a front end must, can, and
must not do. Both live in the rumbo repository.

## Rules

- **A front end like any other.** Reach the host only through the protocol,
  never through rumbo's code or files, and take what the window is attached
  to (folder, session, provider, model) from the host's answers.
- **Keep the front end's duties.** Ignore event types and fields you do not
  know. Never reuse the id of a run in progress; ids come from one counter.
  Answer every `ask`, with an error for a function this window is not
  following. Never show or keep a secret answer, not even in the history.
- **One order.** Everything the host sends arrives on one channel, in the
  order it was sent. Do not split answers from events.
- **Logic out of views.** What the window shows is built from events in
  plain modules that are tested without GPUI (`conversation`, `usage`,
  `history`, `commands`, `choices`, `navigation`). A view draws them and
  sends requests; a form says which lists it needs, and the workspace asks
  for them.
- **Nothing opens a file before `--attached` takes descriptors 3 and 4.**
  Whatever opens first can get those numbers.
- **Known by name only where the window gives it a place.** The sidebar
  calls the host's `projects`, `sessions`, `open`, `new`, `rename`, and
  `delete`; the session's menu and the person's messages `rename`,
  `branch`, `compact`, and `delete`; the details panel `status` and
  `skills`; the picker `use` and `session-set`; and the settings `config`,
  `system-set`, and the functions of the providers; their names are in
  `protocol.rs`. Each is
  offered only when `initialize` lists it, and called with the parameters
  it describes there. What it answers is shown as the host says it, never
  read for meaning. Every command comes from the descriptions the host and
  the provider give, and a parameter's values from the function it names;
  a function marked `lists` is there for those values, and is no command.
  A list a function makes may not hold every value a parameter takes, so
  a field can always be typed instead.
- **One host per project, and one that lists.** A host works only in its
  project's sessions, so the window starts one for each project it opens
  and closes it with the project; and one more in the person's home, the
  catalog's, which only lists every project and its sessions. Nothing reads
  rumbo's files.
- **The project's files are only read.** The changes panel runs `git` in
  the project's folder on a background thread and never writes there, and
  a reply's images are read from the paths it names; the window edits no
  file of the person's.
- **An image of the web shows only once the person asks.** A reply's
  image could carry what the model read to whoever serves it, so nothing
  is fetched until its Show is pressed.
- **The window keeps only its preferences**, in `preferences.rs`'s file,
  read once at startup. Anything rumbo can keep, rumbo keeps.
- **The window draws with `crates/ui`.** Its views use GPUI and the
  components of `crates/ui`, and nothing else draws; what a view needs
  that no component has becomes one there.
- **Lengths are in rems.** What a view or a component draws is measured
  with `ui::rems_from_px` or GPUI's lengths in rems, such as `p_4`, so
  that Ctrl-+ and Ctrl-- grow and shrink all of it; a length that meets
  pixels the window measured is turned into pixels with the window's rem
  size. Only what follows the pointer or is a hairline stays in `px`, such
  as a scrollbar, a border, or a divider's grip.
- **`crates/ui` draws with GPUI alone.** Its components know nothing of
  rumbo. A library may do a part's work for it, as syntect, latex-rust, and
  merman do for code, formulas, and diagrams, giving what GPUI draws; none
  draws itself. Check every GPUI API in the `gpui-pre-*` sources under
  `~/.cargo/registry/src/*/`, examples included, before using it, and do
  not translate from another framework or an older version. The window,
  `ui`, and `overview` pin the same `gpui-pre`, so one GPUI is built; they
  change together. What a component does without being drawn is a plain
  module tested without GPUI, as `text/edit.rs` is for every field. Each
  component has a page in `crates/overview`, and a new one gets its own.
- **UI copy is English.**
- When you compile this project utilize with flag -j 8

## Commands

The checks cover the whole workspace: the window, `ui`, and `overview`.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release --workspace
```

`environment`'s test starts an interactive login shell, which stops a test
run that shares a terminal's process group without owning the terminal, as
an agent's harness does: run the tests there with
`setsid cargo test --workspace`.

## Looking at it

The window has no automated test. After changing a view, run it and look at
it: a task, its streamed answer, a tool call opened, a steer during a run,
an image pasted with Ctrl-V into a task and into a steer, one taken away,
and one sent alone, Escape to cancel, Up and Down in the composer, bringing back what was asked
in another session, a session opened with its history, going back and
forward between sessions, projects, a new session, and the settings, with
the toolbar's arrows and with Alt-Left and Alt-Right, and past a session
deleted, a run left working in another session, a command from Ctrl-P with a form and one with a
confirmation, a parameter whose values depend on the one before (the models
`use` lists for the provider chosen), the model and options picker and a run
that uses what it chose, a provider's function that asks a secret, given
up with Escape, a second project opened beside the first with each one's
sessions under it, the projects rumbo holds listed with their sessions and
one opened from that list, a new session started from the sidebar, and
one started with no project open, which opens the home folder, a
project closed and listed again, the window with a rumbo that has no
`projects`, the settings with the theme and the conversation width changed
and kept for the next start, the window made bigger and smaller with
Ctrl-+ and Ctrl--, kept for the next start, and back with Ctrl-0, the
sidebar searched, dragged wider, and put away with Ctrl-B, a session
renamed and one deleted from its menu, a message changed and one asked
again in a branch, the changes panel on a folder with edits and on one
outside Git, a reply with code, formulas
among words and on their own, a Mermaid diagram, and images from a file
and from the web, the details panel, a session
exported, the project opened in the editor, the notification of a run that
ends in a session not shown and the mark it leaves, a system setting
changed and a provider's details, the window started with
no project, the window started by `rumbo -f gui`, and closing the window,
which must leave no `rumbo serve` behind.

Run it against rumbo with a provider that needs no model, such as rumbo's
`examples/provider.py`, which also offers a function that asks a question,
and with `RUMBO_CONFIG` and `RUMBO_DATA` pointing at a scratch file and
folder, so the person's own sessions and settings stay untouched, and with
`XDG_CONFIG_HOME` and `HOME`
pointing at scratch folders, so the window's own preferences do too and the
catalog's host does not start in the person's home. That provider
describes no model and no options; a scratch copy of it that describes
some is how the picker is looked at, and one that answers with Markdown is
how a reply's code, formulas, diagrams, and images are.

```sh
export RUMBO_CONFIG=/tmp/scratch/config.yaml RUMBO_DATA=/tmp/scratch/data XDG_CONFIG_HOME=/tmp/scratch/config HOME=/tmp/scratch/home
rumbo provider add toy python3 /path/to/rumbo/examples/provider.py
rumbo-gui --rumbo /path/to/rumbo /path/to/a/git/repo -- -p toy -m any "list the files"
```

On Hyprland, `grim -g` takes a screenshot of the window's geometry from
`hyprctl clients -j`, and Hyprland's Lua dispatchers send it keys and close
it. A list in a form opens with Enter once it has the keyboard, Tab moves
between the fields and the buttons, a button takes Space, and Enter runs a
dialog. The settings' pages are reached by typing in their search, which
Tab gives the keyboard and which shows the first page that matches. So
everything is driven without the mouse:

```sh
hyprctl dispatch 'hl.dsp.send_shortcut({mods = "", key = "Return", window = "class:^(rumbo-gui)$"})'
hyprctl dispatch 'hl.dsp.window.close({window = "class:^(rumbo-gui)$"})'
```

The desktop's keyboard has two layouts, and `send_shortcut` finds a key in
the one in use while the window reads it in the first, so a symbol can
arrive as another. Its `exclamdown` arrives as `=`, so Ctrl-= is `CTRL`
with `exclamdown` and Ctrl-+ is `CTRL SHIFT` with it, while `minus` and
`0` arrive as they are. Type a key in the composer first to see what it
arrives as.

The person may be using the desktop meanwhile, and may have a rumbo-gui
of their own open. Open the window on a workspace of its own without moving
them there, with `hl.dsp.exec_cmd(command, {workspace = "3 silent"})`; find
it by the process started, through `pgrep -x rumbo-gui` and the scratch
`RUMBO_CONFIG` in its environment, and send it keys and close it by its
`address:` from `hyprctl clients -j`, never by its class; and take
screenshots only while its workspace is the one shown, since a hidden window
does not draw. Never move the pointer or click: what is under it may be
another program by then. Run a terminal check in a tmux server of its own,
`tmux -L name`, never the person's.

The clipboard is the person's too: never copy to it to test pasting. A
Sway started with `WLR_BACKENDS=headless` has a clipboard and a screen of
its own, which nothing of the person's shares: `wl-copy` an image into it,
run the window in it, and drive it there with rumbo's computer tool, with
`WAYLAND_DISPLAY` and `SWAYSOCK` naming that Sway.

Sway is installed, and its headless Sway is also where the window is used
with the pointer: clicks, menus, hovers, and drags. rumbo's computer tool
acts on the desktop of the rumbo that runs it, as that rumbo started: its
`WAYLAND_DISPLAY`, and `HYPRLAND_INSTANCE_SIGNATURE` before `SWAYSOCK`. An
agent whose rumbo the person started on their desktop, as their own
rumbo-gui does, would see their screen and move their pointer with it, so
it never uses its own computer tool there. It drives the Sway through
another rumbo instead: one started with that Sway's `WAYLAND_DISPLAY` and
`SWAYSOCK` and without `HYPRLAND_INSTANCE_SIGNATURE`, with `computer=on` in
a scratch `RUMBO_CONFIG`, and with a provider of no model that turns each
call it is handed into a computer tool call, in one run held open for all
of them. Start that run before the window: a window started while the
Sway has no keyboard panics in GPUI when the tool's virtual keyboard comes.
`grim`, with the Sway's `WAYLAND_DISPLAY`, takes its screenshots at full
size.

```sh
# sway.conf: output HEADLESS-1 resolution 1280x800
env -u WAYLAND_DISPLAY -u HYPRLAND_INSTANCE_SIGNATURE -u DISPLAY \
  WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 sway -c sway.conf
```

Open Project… asks the desktop's folder dialog, which takes `/` and a path
typed, and Return; it completes a folder that holds a single folder into
that one, so a scratch project with no subfolder, not even `.git`, is the
one to open that way. Never take a screenshot of that dialog: it opens on
the person's recent files. Export as Markdown… asks the desktop's save
dialog, which opens on the person's workspace, not the window's, and takes
their keyboard: answer it at once by its address, with Return to keep the
name it offers in the scratch project, and say so.

Launch only a binary built from the code being checked: `cargo clippy` and
`cargo test` do not rebuild it, and a build that fails leaves the old one.
Put a time limit on anything that can open a window and wait.

## Looking at the components

After changing a component of `crates/ui`, look at its page of the
overview, in both themes and in each typeface, and use it with the
keyboard: Tab to it, what its keys do, Escape where it closes.

```sh
cargo build --release -p overview
target/release/overview [PAGE] [--dark] [--font TYPEFACE]   # such as select --font shantell-sans
```

The same rules hold as for the window: a workspace of its own, found by the
process started, keys and closing by its address, and never the pointer.
When every monitor shows a workspace of the person's, a headless output
gives the overview one that is shown without moving them. Put it far from
their monitors so the pointer cannot reach it, tall enough that a page
needs no scrolling (Tab does not scroll a page to the control it reaches),
and remove it when done:

```sh
hyprctl output create headless RUMBOUI
hyprctl eval 'hl.monitor({output = "RUMBOUI", mode = "1600x2400@60", position = "20000x0", scale = 1})'
grim -o RUMBOUI shot.png
hyprctl output remove RUMBOUI
```

Keys sent to a window that does not have the desktop's keyboard make GPUI
take the window as left after them, so a field tells `Blur` once they stop.
Removing the output gives the monitor focus to another of the person's
monitors; say so, since giving it back would move the pointer.

What needs the pointer, such as dragging a scrollbar, is tested in GPUI's
test context instead: `ui` has `gpui` with `test-support` as a
dev-dependency, which gives a window in the test's own process that
`simulate_mouse_down`, `simulate_mouse_move`, and `simulate_mouse_up`
drive, as `scroll.rs`'s tests do. Its `simulate_keystrokes` sends only
the key going down, and GPUI presses a control with Enter or Space when
the key comes up, so a test of that sends a `KeyDownEvent` and a
`KeyUpEvent`, as `table`'s do. What a person sees under the pointer, such
as a hover, is for them to look at.
