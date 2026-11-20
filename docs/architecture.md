# Architecture

rumbo-gui is a front end of rumbo, and nothing more: rumbo's host owns the
sessions, the tools, the loop, and everything that is recorded. The window
starts a host for each project it opens, and one more that only lists what
rumbo holds; it sends them what the person asks, and draws what they
report. The only thing it keeps itself is the person's
preferences for the window, such as the theme, the sidebar, and the
editor; it keeps no state it would need to restore, since the host records
everything as it happens. Besides the hosts, it reads one thing itself:
what Git says changed in a project's folder.

The contract between them is rumbo's Agent Frontend Protocol
(`docs/frontend-protocol.md` in rumbo), and what a front end must, can, and
must not do is in rumbo's `docs/use-cases.md`. Terms such as run, session,
steer, and provider mean what rumbo's `docs/glossary.md` says.

## Parts

```
 main ── config, preferences ── shell ─────────────── settings, catalog
                                 │  │  one workspace per project
                                 │  └───────────────── workspace ── conversation, usage, history,
                                 │ starts or attaches   │             commands, choices (pure; tested)
                                 ▼                      │
                               host ── connection ◄─────┘
                                 │        │  one ordered channel of what the host sent
                                 ▼        ▼
                          rumbo serve (stdin/stdout), one per project and one that lists
```

- **config** (`src/config.rs`) reads the command line once, at startup.
- **preferences** (`src/preferences.rs`) are what the person prefers of the
  window, read once at startup from a file of the window's own and written
  when one changes. Everything else is rumbo's to keep.
- **shell** (`src/shell/`) is the window's root view: the toolbar along the
  top, which is also the title bar; the projects open in the window, each
  with its sessions, in the sidebar; and the work of the project shown, or
  the settings in its place. Each project has a workspace and a host of its
  own, started in the background when the project is opened, and its runs
  go on while another project is shown. The shell says why a host did not
  start, and starts it again when its workspace asks. The window can start
  with no project, and opens as many as the person asks, from a folder or
  from what rumbo holds. Started by rumbo, it attaches to that rumbo
  instead, and works only in its project.
- **catalog** (`src/shell/catalog.rs`) is what rumbo holds in every project:
  the projects that have sessions, and the sessions of each, asked of a
  host the window starts in the person's home only to list them. It runs
  nothing, follows no function, and is asked again when the window comes
  to the front or a project is closed.
- **sidebar** (`src/shell/sidebar.rs`) lists the projects and their
  sessions, narrowed by a search, with what the host offers to do with
  each session in its menu. It can be dragged wider and put away; the
  window keeps both.
- **settings** (`src/shell/settings.rs`) show the window's preferences and
  keys, and rumbo's configuration. rumbo keeps it the same for every
  project, so it is asked of the host of the project shown: what is set and
  what applies, each value with where it comes from, with `config`, and what
  each provider has, with `provider-show`; what the host says is shown as it
  says it. A change goes through the form of `system-set` or `provider-set`,
  as `name=value`, and rumbo checks it before it keeps it.
- **host** (`src/host.rs`) starts `rumbo serve` in the folder and keeps what
  it writes to stderr, which is where rumbo explains a failure to start; or
  takes the pipes rumbo gives a registered front end on file descriptors 3
  and 4. It takes them first thing in `main`, before the window opens files
  that could get those numbers.
- **connection** (`src/connection.rs`) is the protocol: it answers the
  host's `initialize`, sends requests and notifications, and delivers
  everything the host sends on one channel, in order. One order matters: a
  run's last events arrive before the answer that ends it.
- **workspace** (`src/workspace/`) is one project attached to its host:
  the sessions the host lists, the conversation of the one shown with a
  header and the session's menu, the composer, a panel beside them, the
  status bar, and the command palette. The shell lists the
  sessions and opens them through it. It keeps a view of each session it
  has shown, and sends each event to the view of the session it belongs
  to: the session of the run it names, by the id of the run request, or the
  session it names. A run in a new session learns the session's id from its
  first event. A session shown for the first time asks the host for its
  history, which comes before anything that happens in it after. The
  composer starts with what the host recalls of the project's sessions. The
  model and the options belong to the session, which keeps them: the picker
  changes them with the host's `use` and `session-set`, and every run
  gets what its session has. The workspace follows the provider's function
  it called, answering the questions asked for it and refusing any other.
  What the window asks of the host for itself, as the settings do, goes
  through the workspace too, with what to do with the answer.
- **panel** (`src/workspace/panel.rs`) shows, beside the conversation,
  what changed in the project's folder, read with **git** (`src/git.rs`)
  on a background thread, or the session's details, which are the host's
  `status` and `skills` as it says them. rumbo keeps the conversation and
  not the files, so Git is how a person reviews what a run did to them;
  the window only reads.
- **The session's actions** rename, delete, branch, and compact through
  the host's functions. They act on the current session, so one asked for
  another session first makes it current with `open`, and goes on once the
  host has answered. Changing or asking again something the person wrote
  is a `branch` before that message, numbered as the host numbers branch
  points: the person's tasks, in order.
- **export** (`src/export.rs`) writes a session's conversation as Markdown
  from what the window shows, and **editor** (`src/editor.rs`) says which
  editor opens a project's folder: the person's choice, or one found on
  the PATH the hosts get.
- **form** (`src/workspace/form.rs`) is a form built from the parameters a
  function describes, or from what the picker changes. A parameter whose
  values a function lists shows them in a list, asked for with what was
  given for the parameters before it and asked again when that changes. Such
  a list need not hold every value, so the field can be typed instead,
  starting from the value chosen.
  The form says which lists it needs; the workspace sends the requests and
  hands the answers back, since everything the host says arrives in one
  order, on one channel.
- **conversation**, **usage**, **history**, **commands**, and **choices**
  hold what the workspace shows and asks, built from the protocol's
  messages alone and tested without GPUI: the items of a conversation, the
  tokens spent, what was entered in the composer and what the host
  recalls, the functions of the host and the provider as commands, by what
  they act on, with their parameters, the values those take, and their
  answers, and the changes the picker asks of the host.

## Projects

A host works in the sessions of one project, but lists those of any: its
`projects` lists the projects that have sessions, by their root, and its
`sessions` the sessions of any of them. So the sidebar shows two kinds of
project. Those open in the window have a host of their own, whose
workspace lists their sessions and marks those working. The others rumbo
holds come from the catalog, and opening one, or one of its sessions,
starts its host: with `-s` for a session, with `-c` for the project, and
with neither for a new session, since a host started with `-s` works in
that session's project wherever it starts. A project open in the window is
told from the catalog's by its folder, or by its sessions being listed
under a root, never by reading what the host says.

A host without `projects` lists nothing in the catalog, which then stops
it, and the sidebar shows only the projects open in the window.

## Threads

Each connection reads on a thread of its own. Each workspace receives on
the UI thread, in one task that applies each message in order and redraws.
Starting a host blocks until it speaks, so the shell does it on a
background executor and attaches the project's workspace when it is done.

## Ending

Closing the connection ends the host. The app closes a project's when the
project is closed, and every one when the window closes, and the operating
system does when the process exits; each host then cancels and records its
runs, stops its providers, and exits.
