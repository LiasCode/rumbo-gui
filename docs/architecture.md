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
                     │ starts or attaches     │   └─ conversation, usage, history,
                     ▼                        │      commands, choices (pure; tested)
                   host ── connection ◄───────┘
                     │        │  one ordered channel of what the host sent
                     ▼        ▼
              rumbo serve (stdin/stdout)
```

- **config** (`src/config.rs`) reads the command line once, at startup.
- **launcher** (`src/launcher.rs`) is the window's root view: it asks for a
  folder when none was named, starts the host in the background, says why
  one did not start, and starts it again when the workspace asks. Started
  by rumbo, it attaches to that rumbo instead, and has nothing to start.
- **host** (`src/host.rs`) starts `rumbo serve` in the folder and keeps what
  it writes to stderr, which is where rumbo explains a failure to start; or
  takes the pipes rumbo gives a registered front end on file descriptors 3
  and 4. It takes them first thing in `main`, before the window opens files
  that could get those numbers.
- **connection** (`src/connection.rs`) is the protocol: it answers the
  host's `initialize`, sends requests and notifications, and delivers
  everything the host sends on one channel, in order. One order matters: a
  run's last events arrive before the answer that ends it.
- **workspace** (`src/workspace/`) is the window attached to a host: the
  sessions in a sidebar, the conversation of the one shown, the composer,
  the status bar, and the command palette. It keeps a view of each session
  it has shown, and sends each event to the view of the session it belongs
  to: the session of the run it names, by the id of the run request, or the
  session it names. A run in a new session learns the session's id from its
  first event. A session shown for the first time asks the host for its
  history, which comes before anything that happens in it after. It keeps
  the model and options chosen for the window, which go with every run, and
  follows the provider's function it called, answering the questions asked
  for it and refusing any other.
- **conversation**, **usage**, **history**, **commands**, and **choices**
  hold what the workspace shows and asks, built from the protocol's
  messages alone and tested without GPUI: the items of a conversation, the
  tokens spent, what was entered in the composer, the functions of the host
  and the provider as commands, with their parameters and answers, and the
  model and options chosen for the window.

## Threads

The connection reads on a thread of its own. The workspace receives on the
UI thread, in one task that applies each message in order and redraws.
Starting a host blocks until it speaks, so the launcher does it on a
background executor and attaches the window when it is done.

## Ending

Closing the connection ends the host. The app closes it when the window
closes, and the operating system does when the process exits; the host
then cancels and records its runs, stops its providers, and exits.
