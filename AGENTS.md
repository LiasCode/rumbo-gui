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
  `history`, `commands`, `choices`, `navigation`, `outline`, `order`). A
  view draws them and sends requests; a form says which lists it needs,
  and the workspace asks for them.
- **Nothing opens a file before `--attached` takes descriptors 3 and 4.**
  Whatever opens first can get those numbers.
- **Known by name only where the window gives it a place.** The sidebar
  calls the host's `rename`; the session's menu and the person's messages
  `rename`, `branch`, and `compact`; the details panel `status` and
  `skills`; the picker `use` and `session-set`; and the settings `config`,
  `system-set`, and the functions of the providers; their names are in
  `protocol.rs`. Each is
  offered only when `initialize` lists it, and called with the parameters
  it describes there. What it answers is shown as the host says it, never
  read for meaning: the details panel lays out what `status` and `skills`
  say by its shape alone, with `outline.rs` (headings, `label: value`
  lines, and lines set in under another), and keeps every word, so other
  words still show as words. Every command comes from the descriptions the host and
  the provider give, and a parameter's values from the function it names;
  a function marked `lists` is there for those values, and is no command.
  A list a function makes may not hold every value a parameter takes, so
  a field can always be typed instead.
- **One host per session.** A rumbo host is one session, so the window
  starts one for each session it opens, with `rumbo serve`, and never asks
  a host about another session. A workspace holds the hosts of its
  project's sessions (`src/workspace/hosts.rs`), keeps those that work or
  that the person has not seen the end of, closes the others past the two
  shown last, and closes all of them with the project. What no host says,
  the sessions there are and deleting one, is asked of rumbo's command
  line, `rumbo sessions -json` and `rumbo sessions rm`, in
  `src/sessions.rs` alone. Nothing reads rumbo's files. The ids of the
  requests of every host come from one counter, so one names its request
  among them all.
- **The project's files are only read.** The changes panel runs `git` in
  the project's folder on a background thread and never writes there, the
  files panel lists its folders on a background thread too, as each is
  opened, and reads the file it shows there as well, and a reply's images
  are read from the paths it names; the window edits no file of the
  person's.
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
it: a task, its streamed answer, the context and the tokens under the
composer in K and M with every token in their tooltip, a tool call opened,
a steer during a run, which shows at once, faded, until the run reads it,
and what the composer says while it waits, Working over the composer
while a run works, and none once it ends,
an image pasted with Ctrl-V into a task and into a steer, one taken away,
and one sent alone, Escape to cancel, Up and Down in the composer, bringing back what was asked
in another session, a session opened with its history, going back and
forward between sessions, projects, a new session, and the settings, with
the toolbar's arrows and with Alt-Left and Alt-Right, and past a session
deleted, a run left working in another session, each session opened in a
`rumbo serve` of its own (`pgrep -af "[r]umbo serve"`), the oldest of
those left unused closed once there are more than two, and its session
read again when it is shown again, a command from Ctrl-P with a form and
one with a confirmation, a parameter whose values depend on the one before (the models
`use` lists for the provider chosen), the model and options picker and a run
that uses what it chose, a provider's function that asks a secret, given
up with Escape, a second project opened beside the first with each one's
sessions under it, every project in one list, those rumbo holds with
their sessions and one opened from it, a session of one, chosen in the
sidebar, which goes from the press to the session without the list folding
or the screen of a new session flashing, filmed with `grim` in a loop and
with a host slowed to start and to answer `history`, a project pinned and
one unpinned,
kept for the next start, a search that finds a session under a project
folded, a new session started from the sidebar, which opens the home
folder whatever project is shown, and Ctrl-N, which starts one in the
project shown, a
project closed and listed again, the window with a rumbo that cannot
list its sessions (a `--rumbo` that is no rumbo), which says why in the
sidebar, the settings with the theme and the conversation width changed
and kept for the next start, the window made bigger and smaller with
Ctrl-+ and Ctrl--, kept for the next start, and back with Ctrl-0, the
sidebar searched, dragged wider, and put away with Ctrl-B, a session
renamed and one deleted from its menu, the session shown deleted, which
leaves none shown and none marked, and a new one started from there with
Ctrl-N, a project's sessions after a run in an older one and after the
project is opened, which stay in their order, a message changed and one asked
again in a branch, the changes panel on a folder with edits and on one
outside Git, its history of the branch checked out and of another chosen,
local and remote, and of every branch as a graph, with a merge, a tag, a
remote's branch, and the stash, against what `git log --oneline --decorate
--all --graph` prints, and with more commits loaded, the panel beside the
conversation dragged wider and
narrower, kept for the next start, the files panel with a folder opened
inside another and a file shown beside the tree, its syntax colored in
both themes, a long line wrapped, one too large, one not text, and an
empty one, the divider between them dragged, the file read again after it
changed and closed, it and the project opened in the editor (an editor
set in the scratch preferences to a script that writes down what it is
given, so no editor of the person's opens), the panel opened from its button and with Ctrl-Alt-E while the
toolbar has the keyboard, its files and folders marked as Git sees them
(modified, added, untracked, deleted, and in a repository nested in the
project), and marked again after a refresh and after a run, a reply with
code, formulas
among words and on their own, a Mermaid diagram, and images from a file
and from the web, the details panel (a session with runs and a new one,
the skills with one left out, and everything rumbo reports opened), a session
exported, the project opened in the editor, the notification of a run that
ends in a session not shown and the mark it leaves, the desktop's
notification of one that ends while the window is in the background, with
it turned on and off, and one pressed to show its session, a system setting
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

A notification of the desktop goes to the D-Bus of the session the window
runs in, which shows it on the person's desktop. Give the window and the
Sway a bus of their own instead, with `DBUS_SESSION_BUS_ADDRESS` naming it,
and run mako on it, which draws the notifications in the Sway, where they
are seen and pressed; `dbus-monitor` on that bus shows what was sent.
Switching the Sway to an empty workspace puts the window in the
background.

```sh
dbus-daemon --session --nofork --address=unix:path=/tmp/scratch/bus
DBUS_SESSION_BUS_ADDRESS=unix:path=/tmp/scratch/bus mako   # with the Sway's WAYLAND_DISPLAY
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

What moves, such as a button's face giving under a press and springing
back, is over before the computer tool looks, since it looks once an
action has settled: start the action in the background and take `grim`
screenshots of the part that moves in a loop meanwhile, and look at them
side by side.

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
