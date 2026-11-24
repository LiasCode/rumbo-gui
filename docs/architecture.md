# Architecture

rumbo-gui is a front end of rumbo, and nothing more: rumbo's host owns the
sessions, the tools, the loop, and everything that is recorded. A host is
one session, so the window starts one for each session it opens, and asks
rumbo's command line for the sessions there are; it sends the hosts what
the person asks, and draws what they report. The only thing it keeps itself is the person's preferences for
the window, such as the theme, the sidebar, the size it is drawn at, and
the editor; it keeps no state it would need to restore, since the host
records everything as it happens. Besides the hosts, it reads two things
itself: what Git says of a project's changes and the history of its
branches, and the images a reply names; an image of the web it fetches
only once the person asks for it.

The contract between them is rumbo's Agent Frontend Protocol
(`docs/frontend-protocol.md` in rumbo), and what a front end must, can, and
must not do is in rumbo's `docs/use-cases.md`. Terms such as run, session,
steer, and provider mean what rumbo's `docs/glossary.md` says.

## Parts

```
 main ── config, preferences ── shell ─────────────── settings, catalog, navigation (pure; tested)
                                 │  │  one workspace per project, with a host per session open
                                 │  └───────────────── workspace ── conversation, usage, history,
                                 │ starts or attaches   │             commands, choices, outline, order (pure; tested)
                                 ▼                      │
                               host ── connection ◄─────┘
                                 │        │  one ordered channel of what the host sent
                                 ▼        ▼
                          rumbo serve (stdin/stdout), one for each session open
                          rumbo sessions -json, which lists them (sessions, catalog)
```

- **config** (`src/config.rs`) reads the command line once, at startup.
- **preferences** (`src/preferences.rs`) are what the person prefers of the
  window, read once at startup from a file of the window's own and written
  when one changes. Everything else is rumbo's to keep.
- **shell** (`src/shell/`) is the window's root view: the toolbar along the
  top, which is also the title bar and the only bar over the work, with
  where the window is, the session's panels and menu, the commands, and the
  settings; the projects open in the window, each
  with its sessions, in the sidebar; and the work of the project shown, or
  the settings in its place. Each project has a workspace of its own, and
  the host of the session it opens with, started in the background when
  the project is opened; its runs go on while another project is shown.
  The shell says why that host did not start, and starts it again when
  asked. The window can start
  with no project, and opens as many as the person asks, from a folder or
  from what rumbo holds; a new session asked for with no project open
  opens the person's home folder as one. Started by rumbo, it attaches to
  that rumbo instead, works only in its project, and starts the hosts of
  that project's other sessions with the rumbo `--rumbo` names. Ctrl-+ and Ctrl--
  draw the whole window bigger or smaller, and Ctrl-0 at its own size, by
  the window's rem size, which the preferences keep. When a run ends where
  the person may not see it, the shell says so: in a notification of the
  window when its session is not shown, and, if the person turned it on,
  in one of the desktop when the window is in the background, through
  GPUI's system notifications. Pressing that one shows its session.
- **navigation** (`src/navigation.rs`) keeps the places the window showed,
  as a browser keeps the pages: a session of a project, a new session
  included, or the settings. The toolbar's arrows, Alt-Left and Alt-Right,
  and the mouse's back and forward buttons move between them, passing over
  a place that is gone, such as a closed project or a deleted session.
  Going somewhere is a place; going back or forward is not, and neither is
  a new session getting its id from its first run, which stays the same
  place. A workspace says which is which when it shows another session,
  since only it knows why it did. The places last as
  long as the window, and start again once no project is open.
- **sessions** (`src/sessions.rs`) asks rumbo's command line what no host
  says: `rumbo sessions -json` lists the sessions of a folder, or of every
  one, and `rumbo sessions rm` deletes one. It runs on a background
  thread, and reads none of rumbo's files.
- **catalog** (`src/shell/catalog.rs`) is what rumbo holds in every folder:
  the projects that have sessions, and the sessions of each, from
  **sessions**. It is asked again when the window comes to the front or a
  project is closed.
- **sidebar** (`src/shell/sidebar.rs`) lists every project in one list,
  those open in the window and those rumbo holds, each with its sessions,
  narrowed by the search at its top, with what the host offers to do with
  each session in its menu. The projects the person pins come first, in
  the order they pinned them; the rest follow, most recently used first.
  A project's sessions stay in the order they were first listed, the
  newest first, as **order** (`src/order.rs`) keeps them, so a session a
  run works in does not jump to the top, nor does opening a project move
  them. Deleting the session shown leaves none shown and none marked,
  until the person chooses one or starts a new one.
  Its New Session starts one in the person's home folder, whatever project
  is shown. It can be dragged wider and put away; the window keeps both,
  and the pins.
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
  host's `initialize`, refusing a setting rumbo holds for the window, which
  has none there, sends requests and notifications, and delivers
  everything the host sends on one channel, in order. One order matters: a
  run's last events arrive before the answer that ends it.
- **workspace** (`src/workspace/`) is one project: the sessions rumbo
  lists in its folder, a host for each of them that is open
  (`src/workspace/hosts.rs`), the conversation of the one shown, the
  session's menu and the buttons of the panels, which the toolbar shows,
  the composer, with how full the model's context is and the tokens
  spent, a panel beside them, and the command palette. How a run stands
  shows in what the composer says it is for, and in Working, which turns
  over it while a run works in the session shown; what the person tells a
  run shows at once at the end of the conversation, faded, until the host
  answers that the run has it, and then stays where the run read it.
  Anything else the workspace has to say, such as what went wrong, shows
  in a notification. The
  shell lists the sessions and opens them through it. Showing a session
  no host of the window holds starts one for it, with `-s`, or with
  nothing for a new one, and the session shows once that host has spoken;
  the session left behind keeps its host, and its runs go on. The hosts
  left with nothing to do, neither shown, nor working, nor with a run
  that ended unseen, are closed past the two shown last, since each costs
  its own processes; what the window showed of such a session goes with
  its host, and is read again when it is shown again. What the hosts the
  window holds use, each asked with `processes`, shows in Rumbos, from the
  palette: one row for each session open in any project, and what they
  use together, each process counted once (`src/rumbos.rs`,
  `src/shell/rumbos.rs`). Once the session
  shown is deleted, the workspace shows none until the person opens a
  session or starts a new one. It keeps a view of each session a host is
  open for, and sends each event to the view of the session it belongs
  to: the session of the run it names, by the id of the run request,
  which comes from one counter for every host, or the session of the host
  that sent it. A run in a new session learns the session's id from its
  first event. A session shown for the first time asks its host for its
  history, which comes before anything that happens in it after. A host
  that ends by itself leaves its session as it was: its runs are told as
  failed, and the workspace offers to start it again when it is the one
  shown; the window ends with the rumbo that started it. The
  composer starts with what the host recalls of the project's sessions,
  and keeps the images pasted into it, which go with what is sent next;
  **picture** (`src/picture.rs`) carries them between the clipboard, the
  protocol, and the conversation, which shows them. The
  model and the options belong to the session, which keeps them: the picker
  changes them with the host's `use` and `session-set`, and every run
  gets what its session has. The workspace follows the function of a
  provider or of an extension it called, showing what it says and
  answering the questions asked for it, and answers what a tool of one of
  its runs asks, until that run ends; it refuses any other question
  (`src/workspace/talk.rs`).
  What the window asks of the host for itself, as the settings do, goes
  through the workspace too, with what to do with the answer.
  Consecutive tool calls share a collapsible conversation row. Grouping is
  a view of the transcript, so history and export still keep each call.
  The message marks beside the conversation navigate the person's messages
  in that same view, without leaving the session or changing its transcript.
- **panel** (`src/workspace/panel.rs`) shows, beside the conversation,
  the project's Git repository, its files, or the session's details. The
  Git panel shows what changed in the folder, or the commits of a branch:
  the one checked out, or another, local or remote, chosen without
  checking it out or asking a remote; or, with its Graph switch, those of
  every branch at once, drawn as `git log --oneline --decorate --all
  --graph` draws them, in the colors Git gives the graph's lines
  (`src/git/graph.rs`). It reads with **git** (`src/git.rs`)
  on a background thread. rumbo keeps the conversation and not the files,
  so Git is how a person reviews what a run did to them; the window only
  reads. The panel can be dragged wider and narrower, and the window keeps
  its width, the same for every project.
- **files** (`src/workspace/files.rs`) lists the project's folder beside
  the conversation, each folder read by **file_tree** (`src/file_tree.rs`)
  on a background thread as it is opened, without following links; a file
  pressed shows beside the tree, split from it by a divider that moves,
  its text read on a background thread too, and nothing is written. A file
  too large or not text says so instead, and the editor opens the file or
  the whole project from the panel. Each file and folder
  is marked with what **git** says of it: in the project's repository, and
  in any repository nested in it once its folder is opened, which speaks
  for its own files. A folder shows the most of what it holds. The marks,
  and the file shown, are read again, with the folders open, when the
  panel is refreshed and when a run ends.
- **details** (`src/workspace/details.rs`) draws what the window knows of
  the session shown to be read at a glance: from the host's answers, its
  id and how it stands, its folder and its model; and from its runs'
  events, how full the context is and the tokens spent, as the composer
  counts them. Under them are the skills, as the host's `skills` says
  them, and all its `status` says, which repeats much of the rest, when
  asked for. What the host says in words is laid out by **outline**
  (`src/outline.rs`) by its shape alone, keeping its words: headings,
  `label: value` lines as rows, and lines set in under another as items.
- **The session's actions** rename, branch, and compact through the
  functions of the session's host, which act on its own session, so one
  asked for another session shows it first, and goes on once it is shown.
  A branch is another session, which its host answers with and the
  workspace shows in a host of its own. Changing or asking again
  something the person wrote is a `branch` before that message, numbered
  as the host numbers branch points: the person's tasks, in order.
  Deleting is the window's: no host deletes its session, so the workspace
  closes the session's host and asks `rumbo sessions rm`, once the person
  agrees; the session of the rumbo that started the window is held by it,
  and cannot be deleted from there.
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
- **conversation**, **usage**, **history**, **commands**, **choices**,
  **outline**, and **order** hold what the workspace shows and asks, built
  from the protocol's messages alone and tested without GPUI: the items of a
  conversation, and what waits for a run after them, the tokens spent, what
  was entered in the composer and what the host recalls, the functions of
  the host, the provider, and the extensions as commands,
  by what they act on, with their parameters, the values those take, and
  their answers, the changes the picker asks of the host, the shape of
  what the host says in words, and the order a project's sessions are
  listed in.

## Components

The window draws with components of its own, in `crates/ui`. They are built
on GPUI alone and know nothing of rumbo: a view gives them what to show and
hears what the person did through events and callbacks. Their lengths are in
rems, written as the pixels they are at the default size
(`crates/ui/src/size.rs`), so everything grows and shrinks with the rem size
a window sets, as a page does when a browser zooms; only what follows the
pointer or is a hairline, such as a scrollbar or a border, stays in pixels.
A theme in GPUI's globals gives them their colors, light or dark, and their
typeface: Montserrat or Shantell Sans, which `crates/ui` bundles and gives
GPUI at startup, or the desktop's. Every field's edits go through one buffer
(`crates/ui/src/text/edit.rs`), which a state entity wraps for the keys, the
pointer, and the platform's input methods. Enter in a field of one line goes
on to what holds it, so that a select's list chooses and a dialog answers.
The surfaces that float over the rest (a floating window or a dialog, a
popover or a menu, a select's list) are drawn in GPUI's deferred layers, in
that order from below, so each opens over the one it is in, and a window's
toasts over all of them. A window's dialogs and toasts come from the one
`Dialogs` and `Toasts` its view keeps and hands to what opens them: the
shell keeps the window's, and gives them to each workspace. A reply's
Markdown is read into blocks apart from drawing them
(`crates/ui/src/markdown/blocks.rs`). Selectable text shares one selection
across the reply, so it can be copied across paragraphs and code blocks.
What a reply holds besides words is drawn by libraries made for it: code
colored by syntect, with the grammars bat ships; formulas in
TeX typeset by latex-rust; and Mermaid diagrams laid out by merman, the Rust
Mermaid that Zed uses. Formulas and diagrams come as SVG, which GPUI draws
as images, and so does an image a reply names: from the text itself, from a
file, or from the web, through the `ImageClient` an application gives GPUI,
and only once the person asks, since an image a model names could carry what
it read to whoever serves it. A diff is read from Git's patch apart from
drawing it too (`crates/ui/src/diff/patch.rs`), and draws only the rows in
view, as a conversation does, and so does a file's text in a code view
(`crates/ui/src/code_view.rs`), colored by the same highlighter as a
reply's code, off the UI thread. Whatever the pointer drags (a scrollbar's
thumb, a slider's, the divider between panels, the edge of a table's column)
goes through one press that follows the pointer until the button comes up
(`crates/ui/src/drag.rs`). A window that draws its own title bar draws its
frame too, in a `WindowFrame`, where the desktop leaves the frame to it.
`crates/overview` shows each component on a page of its own, which is where
one is looked at before a view uses it.

## Projects

A project is a folder: the root of the sessions started in it, which is
how rumbo lists them and how the window groups them. The sidebar's one
list holds two kinds of project. Those open in the window have a
workspace, which lists their sessions, marks those working, and holds a
host for each one open. The others rumbo holds come from the catalog, and
opening one, or one of its sessions, opens the project with the host of
that session: started with `-s` for a session, with `-c` for the
project's latest, and with neither for a new session. A project open in
the window is told from the catalog's by its folder.

When rumbo's command line cannot list, the sidebar says why, and shows
only the projects open in the window.

## Threads

Each connection reads on a thread of its own. Each workspace receives on
the UI thread, in one task that applies each message in order and redraws.
Starting a host blocks until it speaks, so the shell does it on a
background executor and attaches the project's workspace when it is done,
and so does a workspace for the host of each other session.

## Ending

Closing the connection ends the host. The app closes a session's when the
session is left unused or deleted, a project's when the project is
closed, and every one when the window closes, and the operating
system does when the process exits; each host then cancels and records its
runs, stops its providers, and exits.
