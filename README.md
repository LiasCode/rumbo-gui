# rumbo-gui

A desktop window for [rumbo](https://github.com/LiasCode/rumbo), the agent
harness: every project and session in one place, each session as a
conversation, and rumbo's settings, without a terminal.

![The window: a toolbar with back and forward, the session shown, the commands, and the settings; a sidebar with every project and its sessions; the conversation of the session shown, with tool calls; and the composer, with the model, the context, and the tokens.](docs/window.svg)

## How it works

![The window speaks rumbo's Agent Frontend Protocol with one rumbo serve per open project, and with one more that only lists every project; the hosts reach the providers, and keep everything in rumbo's files: what you set in config.yaml, and a folder for each session.](docs/how-it-works.svg)

- **Each open project has its own `rumbo serve`.** Its runs go on while
  you look at another project or session.
- **Everything is rumbo's.** Sessions, settings, and providers live in
  rumbo, and the settings show them and change them through rumbo, which
  checks a change before it keeps it. The window keeps only
  its preferences (theme, conversation width, the sidebar, the editor, and
  whether to say when a run ends elsewhere, and from the desktop, and the
  projects pinned), in
  `rumbo-gui/preferences.json` under `~/.config` (or `$XDG_CONFIG_HOME`).
- **The files are yours.** rumbo keeps the conversation, not the files, so
  the Changes panel shows what Git sees changed in the project's folder,
  and Open in Editor hands it to your editor. The window only reads them.
- **Nothing is hard-coded.** Commands, forms, and pickers come from what
  rumbo and the provider describe, so a new function shows up by itself.

## What you can do

- **Work in sessions.** Search them, open one with Ctrl-K or Ctrl-1 … 9,
  rename, branch, compact, export as Markdown, or delete it from its menu.
  Every project is in the sidebar, its sessions newest first, each staying
  where it is when you use it; pin the projects you use most to its top.
  New Session there starts one in your home folder.
- **Go back and forward.** As in a browser, between the sessions, projects,
  and settings you went to, with the arrows in the toolbar, Alt-Left and
  Alt-Right, or the mouse's back and forward buttons.
- **Follow a run.** The model's answer as Markdown, each tool call with
  what it changed in a file, and how long the run took; tell it something
  while it works, or cancel it.
- **Show the model a picture.** Ctrl-V pastes the image in the clipboard
  into the composer, and it goes with the task, or with what you tell a
  run; the conversation shows it where you gave it.
- **Change your mind.** Edit a message, or ask it again, in a branch that
  starts before it; the session you were in stays as it was.
- **Review the work.** The Changes panel shows the diff of the project's
  folder; the Files panel, its folders and files, each marked with what Git
  says of it and opening in your editor; the Details panel, how full the
  model's context is, the tokens spent, and the skills the model is told
  of.
- **Leave runs working.** Runs go on in other sessions and projects; the
  window tells you when one ends, and marks its session. Turn on Notify
  from the desktop in the settings, and the desktop tells you when one ends
  while you work in another window; pressing it shows the session.

## Install

```sh
cargo build --release      # the binary is target/release/rumbo-gui
install -Dm644 rumbo-gui.desktop ~/.local/share/applications/rumbo-gui.desktop
                           # with Exec set to the binary's path if it is not on PATH
```

rumbo must be installed apart: a version later than 0.5, whose
configuration is in `config.yaml` and whose host offers `config`,
`system-set`, `session-set`, and `provider-set`. With an earlier one, the
window still works, without the settings it does not offer. The projects
rumbo holds, besides those open in the window, need a rumbo whose host
offers `projects`; without it, the sidebar lists only the projects open in
the window.

## Use

```sh
rumbo-gui [--rumbo PATH] [DIR] [-- FLAGS]
```

| Argument | |
|---|---|
| `DIR` | The first project. Without it, the window starts with none. |
| `FLAGS` | Passed to `rumbo serve`: `-c`, `-s ID`, `-p`, `-m`, `-o name=value`, `-single`, or a first task. |
| `--rumbo PATH` | The rumbo executable, when it is not on `PATH`. |

| Key | Does |
|---|---|
| Enter / Shift-Enter | Send the task, or tell the run in progress something / new line |
| Up, Down | What was asked before, in any session of the project |
| Escape | Give up a provider's function in progress, else cancel the run; leave the settings |
| Ctrl-K, Ctrl-P | Commands, and the project's sessions to show one |
| Ctrl-N | New session; with no project open, in your home folder |
| Ctrl-1 … Ctrl-9 | The project's sessions, in the order the sidebar lists them |
| Alt-Left, Alt-Right | Back and forward between what was shown |
| Ctrl-B | Show or put away the sidebar |
| Ctrl-F | Search the sessions |
| Ctrl-Shift-G | What changed in the project's folder, as Git sees it |
| Ctrl-Alt-E | Explore project files |
| Ctrl-Alt-B | The session's details: its context, tokens, and skills |
| Ctrl-Shift-E | Open the project in your editor |
| Ctrl-O | Open a project |
| Ctrl-, | Settings |

The settings' Keyboard page lists them too.

Closing the window ends every `rumbo serve` it started; a run in progress
is cancelled and recorded.

## As rumbo's front end

rumbo can start the window itself, as it starts its terminal UI. The window
then works only in that rumbo's project, and closes with it.

```sh
rumbo frontend add gui /path/to/rumbo-gui --attached
rumbo -f gui "a task"      # or: rumbo system set frontend gui
```

## Development

[AGENTS.md](AGENTS.md) has the checks every change runs, and
[docs/architecture.md](docs/architecture.md) how the parts fit. Built with
Rust and [GPUI](https://github.com/zed-industries/zed), with the
components in `crates/ui`; `cargo run --release -p overview` shows them.
