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
  `history`, `commands`, `choices`). A view draws them and sends requests;
  a form says which lists it needs, and the workspace asks for them.
- **Nothing opens a file before `--attached` takes descriptors 3 and 4.**
  Whatever opens first can get those numbers.
- **Known by name only where the window gives it a place.** The sidebar
  calls the host's `projects`, `sessions`, `open`, `new`, `rename`, and
  `delete`; the session's menu and the person's messages `rename`,
  `branch`, `compact`, and `delete`; the details panel `status` and
  `skills`; the picker `use` and `session-options`; and the settings the
  functions of the system and the providers; their names are in
  `protocol.rs`. Each is
  offered only when `initialize` lists it, and called with the parameters
  it describes there. What it answers is shown as the host says it, never
  read for meaning. Every command comes from the descriptions the host and
  the provider give, and a parameter's values from the function it names.
- **One host per project, and one that lists.** A host works only in its
  project's sessions, so the window starts one for each project it opens
  and closes it with the project; and one more in the person's home, the
  catalog's, which only lists every project and its sessions. Nothing reads
  rumbo's database.
- **The project's files are only read, through Git.** The changes panel
  runs `git` in the project's folder on a background thread and never
  writes there; the window edits no file of the person's.
- **The window keeps only its preferences**, in `preferences.rs`'s file,
  read once at startup. Anything rumbo can keep, rumbo keeps.
- **GPUI Kit from its source.** Check every API in the crate sources under
  `~/.cargo/registry/src/*/` (`gpui-component-*`, `gpui-base-*`,
  `gpui-pre-*`) or at <https://gpui-kit.com/component/NAME.md> before using
  it. Do not translate from another framework or an older version.
- **UI copy is English.**

## Commands

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

`environment`'s test starts an interactive login shell, which stops a test
run that shares a terminal's process group without owning the terminal, as
an agent's harness does: run the tests there with `setsid cargo test`.

## Looking at it

The window has no automated test. After changing a view, run it and look at
it: a task, its streamed answer, a tool call opened, a steer during a run,
Escape to cancel, Up and Down in the composer, bringing back what was asked
in another session, a session opened with its history, a run left working
in another session, a command from Ctrl-P with a form and one with a
confirmation, a parameter whose values depend on the one before (the models
`use` lists for the provider chosen), the model and options picker and a run
that uses what it chose, a provider's function that asks a secret, given
up with Escape, a second project opened beside the first with each one's
sessions under it, the projects rumbo holds listed with their sessions and
one opened from that list, a new session started from the sidebar, a
project closed and listed again, the window with a rumbo that has no
`projects`, the settings with the theme and the conversation width changed
and kept for the next start, the sidebar searched, dragged wider, and put
away with Ctrl-B, a session renamed and one deleted from its menu, a
message changed and one asked again in a branch, the changes panel on a
folder with edits and on one outside Git, the details panel, a session
exported, the project opened in the editor, the notification of a run that
ends in a session not shown and the mark it leaves, a system setting
changed and a provider's details, the window started with
no project, the window started by `rumbo -f gui`, and closing the window,
which must leave no `rumbo serve` behind.

Run it against rumbo with a provider that needs no model, such as rumbo's
`examples/provider.py`, which also offers a function that asks a question,
and with `RUMBO_DB` pointing at a scratch database, so the person's own
sessions and settings stay untouched, and with `XDG_CONFIG_HOME` and `HOME`
pointing at scratch folders, so the window's own preferences do too and the
catalog's host does not start in the person's home. That provider
describes no model and no options; a scratch copy of it that describes
some is how the picker is looked at.

```sh
export RUMBO_DB=/tmp/scratch/rumbo.db XDG_CONFIG_HOME=/tmp/scratch/config HOME=/tmp/scratch/home
rumbo provider add toy python3 /path/to/rumbo/examples/provider.py
rumbo-gui --rumbo /path/to/rumbo /path/to/a/git/repo -- -p toy -m any "list the files"
```

On Hyprland, `grim -g` takes a screenshot of the window's geometry from
`hyprctl clients -j`, and Hyprland's Lua dispatchers send it keys and close
it. A list in a form opens with Enter once it has the keyboard, Tab moves
between the fields and the buttons, a button takes Space, and Enter runs a
dialog. The settings' pages are reached by typing in their search, which
shows the first page that matches. So everything is driven without the
mouse:

```sh
hyprctl dispatch 'hl.dsp.send_shortcut({mods = "", key = "Return", window = "class:^(rumbo-gui)$"})'
hyprctl dispatch 'hl.dsp.window.close({window = "class:^(rumbo-gui)$"})'
```

The person may be using the desktop meanwhile, and may have a rumbo-gui
of their own open. Open the window on a workspace of its own without moving
them there, with `hl.dsp.exec_cmd(command, {workspace = "3 silent"})`; find
it by the process started, through `pgrep -x rumbo-gui` and the scratch
`RUMBO_DB` in its environment, and send it keys and close it by its
`address:` from `hyprctl clients -j`, never by its class; and take
screenshots only while its workspace is the one shown, since a hidden window
does not draw. Never move the pointer or click: what is under it may be
another program by then. Run a terminal check in a tmux server of its own,
`tmux -L name`, never the person's.

Open Project… asks the desktop's folder dialog, which takes `/` and a path
typed, and Return; it completes a folder that holds a single folder into
that one, so a scratch project with no subfolder, not even `.git`, is the
one to open that way. Never take a screenshot of that dialog: it opens on
the person's recent files.

Launch only a binary built from the code being checked: `cargo clippy` and
`cargo test` do not rebuild it, and a build that fails leaves the old one.
Put a time limit on anything that can open a window and wait.
