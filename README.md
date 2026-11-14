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

A tool call opens with a click, or with the keyboard, to show the arguments
the model gave and the result it got back. The status bar shows the
provider and the model, what is happening, the session, how full the
model's context is, and the tokens spent, with the share of the input the
provider read from its cache.

Closing the window ends the host, as rumbo's protocol says: a run in
progress is cancelled and recorded. If the host ends on its own, the window
offers to start it again in the same session.

## Limits

- The window shows what happens while it is open. A session continued with
  `-c` or `-s` starts with an empty conversation, since the protocol has no
  way to ask for an earlier one.
- One session at a time; sessions in the background, the command palette,
  the model picker, and provider functions that talk with the person are
  still to come.

## Development

See [AGENTS.md](AGENTS.md) for the checks every change runs, and
[docs/architecture.md](docs/architecture.md) for how the parts fit.
