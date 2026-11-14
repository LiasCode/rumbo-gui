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
  `history`). A view draws them and sends requests.
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

## Looking at it

The window has no automated test. After changing a view, run it and look at
it: a task, its streamed answer, a tool call opened, a steer during a run,
Escape to cancel, Up and Down in the composer, and closing the window,
which must leave no `rumbo serve` behind.

Run it against rumbo with a provider that needs no model, such as rumbo's
`examples/provider.py`, and with `RUMBO_DB` pointing at a scratch database,
so the person's own sessions and settings stay untouched:

```sh
export RUMBO_DB=/tmp/scratch/rumbo.db
rumbo provider add toy python3 /path/to/rumbo/examples/provider.py
rumbo-gui --rumbo /path/to/rumbo /path/to/a/git/repo -- -m toy/any "list the files"
```

On Hyprland, `grim -g` takes a screenshot of the window's geometry from
`hyprctl clients -j`, and Hyprland's Lua dispatchers send it keys and close
it:

```sh
hyprctl dispatch 'hl.dsp.send_shortcut({mods = "", key = "Return", window = "class:^(rumbo-gui)$"})'
hyprctl dispatch 'hl.dsp.window.close({window = "class:^(rumbo-gui)$"})'
```
