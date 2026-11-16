# rumbo-gui

A desktop front end for rumbo, the agent harness. It opens a window on a
project and speaks rumbo's Agent Frontend Protocol with it: you give tasks
and watch the model answer, call tools, and finish. It starts `rumbo serve`
itself, or rumbo starts it as a registered front end.

It is built with Rust, [GPUI](https://github.com/zed-industries/zed) and
[GPUI Kit](https://gpui-kit.com).

## Build

```sh
cargo build --release
```

The binary is `target/release/rumbo-gui`. rumbo itself must be installed
separately.

To find it among the desktop's applications, install the desktop entry,
with `Exec` changed to the binary's full path when it is not on `PATH`:

```sh
install -Dm644 rumbo-gui.desktop ~/.local/share/applications/rumbo-gui.desktop
```

## Use

```sh
rumbo-gui [--rumbo PATH] [DIR] [-- FLAGS]
```

- `DIR` is the first project to work in. Without it, the window opens with
  none, and Open Project… asks for a folder.
- `FLAGS` go to `rumbo serve` as they are: `-c` continues the latest
  session, `-s ID` another one, `-p PROVIDER`, `-m MODEL`, and `-o
  NAME=VALUE` choose the session's provider, model, and options, `-single`
  keeps the host to one session at a time, and anything else is the first
  task.
- `--rumbo PATH` names the rumbo executable when it is not on `PATH`.

In the window:

| Key | What it does |
|---|---|
| Enter | Sends the task. During a run, tells the run what was typed, which it reads before its next request. |
| Shift-Enter | A new line. |
| Up, Down | Earlier and later entries, from the first and the last line of the input, starting with what was asked in the project's sessions. |
| Escape | Gives up a provider's function in progress, else cancels the run; in the settings, goes back. |
| Ctrl-P, Ctrl-K | Opens the commands. |
| Ctrl-O | Opens a project. |
| Ctrl-, | Opens the settings. |

A tool call opens with a click, or with the keyboard, to show the arguments
the model gave and the result it got back. The status bar shows what is
happening, the provider and the model, how full the model's context is,
and the tokens the session has spent, with the share of the input the
provider read from its cache.

The toolbar along the top says what is shown, and opens the commands and
the settings.

The sidebar lists the projects open in the window, each with its sessions,
and under Recent every other project rumbo holds, with its sessions; with
no project open, the window offers them too. Choosing one of those opens
it: in the session chosen, or in its latest, and its `+` with a new one.
Each open project has a rumbo of its own, started when it is opened, and
its runs go on while another project is shown. Opening a session shows what it
holds, as if the window had followed every run; a run in the session left
behind goes on in the background, marked in the sidebar, and its
conversation is whole when the session is shown again. A project's `+`
starts a new session with the next task, and its menu closes it, which
ends its rumbo; its sessions stay, under Recent. The window lists them with
one more rumbo, started in your home only to ask; a rumbo without
`projects` lists none, and the window shows only the projects open in it.

The commands are the functions rumbo and the provider describe, found by
name, rumbo's by what they act on: the system, the session, and the
providers. A form asks for their parameters, and one that cannot be undone
asks to be confirmed. A parameter whose values the host or the provider
lists offers them to choose from, given what was chosen before it: `open`
lists the sessions, and `use` the models of the provider chosen. Their
answers show in the conversation, marked `[host]` or `[provider …]`. A
provider's function can talk with the person while it works, as a sign-in
does: what it says shows in the conversation, and what it asks opens a
dialog, where a secret answer is not shown and not kept.

The settings hold the window's theme, which follows the desktop or stays
light or dark, and rumbo's system settings and providers, as the rumbo of
the project shown says them: a setting changes, or goes back to its
default, and a provider shows what it offers, and changes its options and
configuration, through rumbo's own functions. The theme is kept in
`rumbo-gui/preferences.json` under `$XDG_CONFIG_HOME`, or `~/.config`;
everything else, rumbo keeps.

The provider and the model in the status bar open the session's provider,
model, and options, with the values the host and the provider list for
them. The session keeps what is chosen there, as `use` and
`session-options` do, and its tasks use it from then on; a task in
progress goes on with what it started with. Another provider has other
options, which can be chosen once the session uses it.

Closing the window ends every project's rumbo, as rumbo's protocol says: a
run in progress is cancelled and recorded. If one ends on its own, the
window offers to start it again in the same session.

rumbo-gui follows the protocol of rumbo 0.4, and needs a host that answers
`history`, which tells a session's earlier conversation. What an older host
does not offer, such as `recall`, the groups of its functions, or
`projects`, the window goes on without.

## As rumbo's front end

rumbo can start rumbo-gui itself, as it starts its terminal UI, giving it
the protocol on file descriptors 3 and 4:

```sh
rumbo frontend add gui /path/to/rumbo-gui --attached
rumbo -f gui "a task"            # or `rumbo system set frontend gui`
```

The window then belongs to that rumbo, and works in its project only:
closing it ends rumbo, and when rumbo ends, the window closes.

## Development

See [AGENTS.md](AGENTS.md) for the checks every change runs, and
[docs/architecture.md](docs/architecture.md) for how the parts fit.
