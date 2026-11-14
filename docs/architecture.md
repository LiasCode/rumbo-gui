# Architecture

rumbo-gui is a front end of rumbo, and nothing more: rumbo's host owns the
sessions, the tools, the loop, and everything that is recorded. The window
starts a host, sends it what the person asks, and draws what the host
reports. It keeps no state it would need to restore, since the host records
everything as it happens.

The contract between them is rumbo's Agent Frontend Protocol
(`docs/frontend-protocol.md` in rumbo), and what a front end must, can, and
must not do is in rumbo's `docs/use-cases.md`. Terms such as run, session,
steer, and provider mean what rumbo's `docs/glossary.md` says.

## Parts

```
 main ── config ── launcher ──────────────── workspace
                     │                        │   │
                     │ starts                 │   └─ conversation, usage, history
                     ▼                        │        (pure; tested without a window)
                   host ── connection ◄───────┘
                     │        │  one ordered channel of what the host sent
                     ▼        ▼
              rumbo serve (stdin/stdout)
```

- **config** (`src/config.rs`) reads the command line once, at startup.
- **launcher** (`src/launcher.rs`) is the window's root view: it asks for a
  folder when none was named, starts the host in the background, says why
  one did not start, and starts it again when the workspace asks.
- **host** (`src/host.rs`) starts `rumbo serve` in the folder and keeps what
  it writes to stderr, which is where rumbo explains a failure to start.
- **connection** (`src/connection.rs`) is the protocol: it answers the
  host's `initialize`, sends requests and notifications, and delivers
  everything the host sends on one channel, in order. One order matters: a
  run's last events arrive before the answer that ends it.
- **workspace** (`src/workspace.rs`) is the window attached to a host: the
  conversation, the composer, and the status bar. It follows the run it
  started by the id of its run request, and answers what it cannot follow,
  such as a question for a provider function it never called, with an error.
- **conversation**, **usage**, and **history** hold what the workspace
  shows, built from events alone and tested without GPUI: the items of the
  conversation, the tokens spent, and what was entered in the composer.

## Threads

The connection reads on a thread of its own. The workspace receives on the
UI thread, in one task that applies each message in order and redraws.
Starting a host blocks until it speaks, so the launcher does it on a
background executor and attaches the window when it is done.

## Ending

Closing the connection ends the host. The app closes it when the window
closes, and the operating system does when the process exits; the host
then cancels and records its runs, stops its providers, and exits.
