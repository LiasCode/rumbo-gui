# rumbo-gui

A desktop front end for rumbo, the agent harness. It opens a
window on a project, starts `rumbo serve` there, and speaks rumbo's Agent
Frontend Protocol with it: you give tasks and watch the model answer, call
tools, and finish.

It is built with Rust, [GPUI](https://github.com/zed-industries/zed) and
[GPUI Kit](https://gpui-kit.com).

## Build

```sh
cargo build --release
```

The binary is `target/release/rumbo-gui`. rumbo itself must be installed
separately.

## Use

```sh
rumbo-gui [--rumbo PATH] [DIR] [-- FLAGS]
```

- `DIR` is the project to work in. Without it, the window asks for a
  folder.
- `FLAGS` go to `rumbo serve` as they are: `-c` continues the latest
  session, `-s ID` another one, `-m provider/model` chooses the model,
  `-single` keeps the host to one session at a time, and anything else is
  the first task.
- `--rumbo PATH` names the rumbo executable when it is not on `PATH`.

In the window:

| Key | What it does |
|---|---|
| Enter | Sends the task. During a run, tells the run what was typed, which it reads before its next request. |
| Shift-Enter | A new line. |
| Up, Down | Earlier and later entries, from the first and the last line of the input. |
| Escape | Cancels the run. |
| Ctrl-K | Opens the commands. |

A tool call opens with a click, or with the keyboard, to show the arguments
the model gave and the result it got back. The status bar shows what is
happening, the provider and the model, how full the model's context is,
and the tokens the session has spent, with the share of the input the
provider read from its cache.

The sidebar lists the project's sessions. Opening one shows what it holds,
as if the window had followed every run; a run in the session left behind
goes on in the background, marked in the sidebar, and its conversation is
whole when the session is shown again. The `+` starts a new session with
the next task.

The commands are the functions rumbo and the provider describe, found by
name: a form asks for their parameters, and one that cannot be undone asks
to be confirmed. Their answers show in the conversation, marked `[host]` or
`[provider …]`.

Closing the window ends the host, as rumbo's protocol says: a run in
progress is cancelled and recorded. If the host ends on its own, the window
offers to start it again in the same session.

rumbo-gui needs a rumbo whose host answers `history`, which tells a
session's earlier conversation.

## Limits

- A provider's function that asks the person something fails for now, and
  the model and its options are chosen with the `use` command; dialogs for
  both are still to come.

## Development

See [AGENTS.md](AGENTS.md) for the checks every change runs, and
[docs/architecture.md](docs/architecture.md) for how the parts fit.
