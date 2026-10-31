# FigCraft control protocol

The desktop app can be driven by other programs (tests, scripts, `figcraft-cli mcp --attach`,
sibling Crafting Apps) through a loopback socket:

```sh
cargo run -p figcraft -- --control 7979 --automation-write-root /work/out [file.figcraft|file.svg|image]
```

The app listens on `127.0.0.1:<port>` only, and refuses to start `--control` (or
`FIGCRAFT_CONTROL_PORT`) without at least one `--automation-read-root` or `--automation-write-root`:
the channel never reads or writes files anywhere on the disk. See
[Confining file access](#confining-file-access).

## Authentication

At startup the app makes a random session token (32 bytes from the system's secure random numbers,
64 hex digits), prints it on stderr and writes it, without a newline, to `control-token-<port>` in
its configuration folder (`%APPDATA%\FigCraft`, `~/Library/Application Support/FigCraft`,
`$XDG_CONFIG_HOME/FigCraft` or `~/.config/FigCraft`). The file is written under a temporary name
(`control-token-<port>.tmp`) and renamed, so a reader never sees a half-written token; on Unix it is
readable by its owner only. Starting again replaces the token, and the app removes the file when it
exits normally. If the system has no secure random numbers the channel does not start.

The **first line** of every connection must be exactly `{"auth": "<token>"}`, and must arrive within
**5 s** of the connection opening. When it matches, the app answers one acknowledgement line,
`{"id": null, "ok": true, "result": "authenticated"}`, and the connection then takes requests as below;
a client reads that line before its first request (it may already have written the request: the app
reads it only after the acknowledgement). Any other first line (a command, a wrong token, an HTTP
request) gets `{"id": null, "ok": false, "error": "authentication failed: …"}` and the connection is
closed without reading or running anything else. The token is compared in constant time, and the
first line is limited to 1 KiB (a longer one is refused, whatever it holds).

```text
{"auth": "<token>"}
{"id": null, "ok": true, "result": "authenticated"}
{"id": 1, "method": "ui.inspect", "params": {}}
```

After the handshake, each connection carries **newline-delimited JSON**: one request object per
line, one reply line per request, in order.

```text
request:  {"id": 1, "method": "engine.execute", "params": {...}}
reply:    {"id": 1, "ok": true, "result": ...}
          {"id": 1, "ok": false, "error": "message"}
```

Requests are queued and run on the UI thread at the start of the next frame, so replies reflect the
real UI state. A request that the UI doesn't answer within 60 s gets a timeout error. A line that is
not JSON gets `{"id": null, "ok": false, "error": "bad JSON: …"}` and ends the connection (so an
HTTP request a web page makes the browser send here never runs). The implementation is in
`crates/ui-egui/src/control.rs` (methods) and `apps/figcraft/src/control_server.rs` (socket).

A request line is limited to **32 MiB before the newline**, including whitespace. The reader stops
at the first excess byte, replies `{"id": null, "ok": false, "error": "message too large"}`, then
closes the connection without consuming the rest of the frame. The server accepts at most **8
active connections**; an additional connection is answered
`{"id": null, "ok": false, "error": "too many connections"}` (when the client is still there to read
it) and closed immediately. Waiting for a request line has a **60 s deadline in total**, fixed when the wait starts and not restarted by
each byte, so a client that trickles bytes cannot hold a worker; a write that makes no progress for
60 s also ends the connection.
Clients can reconnect after an idle timeout (the attached MCP backend already does).

Both the socket queue and the UI queue hold at most **8 requests** each, including requests waiting
for another window's frame. A full socket queue replies with `"control request queue is full"`
instead of dispatching the new request. These limits apply to the opt-in loopback control channel;
requests still queued after the UI timeout are discarded, including requests deferred to a hidden
window. A command already running is not interrupted.

A minimised or covered window (the main one or another, see Windows below) draws no frames, but
still answers every method that acts on the app's state (`engine.execute`, `document.inspect`,
`node.get`, `ui.inspect`, `ui.pointer`, `ui.key`, `ui.type`, `ui.set`, `ui.render`, `window.list`,
…), in the file that window shows, so an attached agent can edit, inspect and save while the app is
out of sight. The app is not drawn meanwhile and only wakes for requests, at most ten times a
second while minimised, so a request there takes up to about 0.1 s. A request that needs a drawn
frame (`ui.screenshot`, `ui.input`) is kept queued while the window is hidden and asks it to paint
(`RequestPaintWhileHidden`; native eframe honours that). A screenshot whose window reports no
visibility at all is parked the same way with its deadline, so it cannot wait forever in the
queue. A screenshot settles on three completed, presented UI frames (a discarded sizing pass does
not count), then is issued, and expires once at its 60 s deadline if no paint arrives. `window.new`
remains an error at once (`window 0 is minimised or hidden: …`) instead of waiting for the window
to show; `ui.render` draws the document without it. Under Wayland the app can't tell that its
window is minimised: where the compositor then stops drawing it, a request waits for the window
and gets the 60 s timeout error. The browser's shared eframe runner does not implement
`RequestPaintWhileHidden` yet, so hidden-tab screenshots and injected input are unqualified there.

Coordinates are page units, y down. `x`/`y` in `node.set`/`node.create` are relative to the parent
frame (groups have no coordinate space of their own).

**Windows.** The desktop app can show files in several windows (`window.new`). Every method takes
an optional `window` (an id from `window.list`; `0` is the main window) and then runs in that
window, with its tabs, panels and canvas: `engine.execute` acts on the file it shows,
`ui.screenshot` captures it, `ui.input` reaches it. Without `window`, a request goes to the window
last in front (the one the user clicked last, or the last `window.new` / `window.focus`). An
unknown id is an error.

## Confining file access

```sh
figcraft --control 7979 --automation-read-root /work/project --automation-write-root /work/project/out
```

At least one of the two flags is required with `--control`; without it the app prints a message
naming the three flags and exits with status 2.

`--automation-read-root <dir>` and `--automation-write-root <dir>` (each repeatable, `--flag=<dir>`
works too; the same flags as `figcraft-cli mcp`) confine the files the app reads and writes for
whoever asks: every control request (`engine.execute`, `ui.render`, `ui.screenshot`, `window.new`,
`ui.key`, `ui.menu.invoke`…) and every frame of every window, so input injected with `ui.input`
and what a person does in the window are held to the same folders. Files are read only inside a
read or write root and written only inside a write root; anything else is an ordinary error reply
(`automation path rejected: …`), and recent files outside the roots aren't listed. Paths are
resolved as `docs/mcp.md` › Confining file access describes. A root that isn't an existing folder
stops the app before its window opens, and so does any option the app doesn't know (a mistyped
flag such as `--automation-write-roots` exits with `unknown option`, never starts unconfined).
Without `--control`, running without the flags changes nothing. The flags arrived in FigCraft 0.7.0: 0.6.0 and earlier ignore
them and confine nothing.

Not confined: opening the files given on the command line when the app starts and the documents
the system hands over (Finder, Open With; saving them later is confined like any write), and the
app's own folders (preferences, recovery copies, plugins installed from a plugin directory). Run `figcraft-cli mcp --attach PORT` without the flags against
such an app (it refuses them: the app does the confining). `ui.screenshot` and `ui.render` without a
`path` return the PNG as data, which is how the attached MCP server takes them, so it writes no
temporary files.

## Methods

### `engine.execute {command, params?}`

Run any command (the same ids as the menus, shortcuts, Actions palette, CLI and MCP). Programmatic
calls never open dialogs: dialog requests are dropped. Returns the command's result (creation
commands return `{"id": N}`). Paints (`fills`, `strokes`, `paint`) take a colour, the full paint
JSON or the gradient shorthand `{"type":"linearGradient","stops":["#hex","#hex"],"from"?,"to"?}`
(as in `docs/mcp.md`).

```json
{"id":1,"method":"engine.execute","params":{"command":"shape.rectangle","params":{"x":10,"y":10,"width":100,"height":50}}}
{"id":1,"ok":true,"result":{"id":2}}
{"id":2,"method":"engine.execute","params":{"command":"node.set","params":{"ids":[2],"props":{"fill":"#FF6B6B","cornerRadius":8}}}}
{"id":3,"method":"engine.execute","params":{"command":"command.batch","params":{"label":"Tidy","commands":[{"command":"align.left","params":{"ids":[2,3]}},{"command":"distribute.vertical","params":{"ids":[2,3]}}]}}}
```

Inside `command.batch`, a step can name itself with `"as"` and later steps use its result:
`{"$ref": "name"}` in a step's params stands for the `id` in that step's result, and `{"$step": n}`
for the `id` in step `n`'s result (zero-based). Either takes `"field"` for another key or a slash
path into the result, such as `ids/0` for the first copy `edit.duplicate` returned. Any object with a
`$ref` or `$step` key is read as a reference, names are unique within a batch, and a nested
`command.batch` step keeps its own references to its own steps (see [MCP: Step
references](mcp.md#step-references) for the rules and errors). So one batch can create a frame, fill
it and duplicate it:

```json
{"id":4,"method":"engine.execute","params":{"command":"command.batch","params":{"commands":[{"command":"node.create","as":"card","params":{"type":"frame","width":200,"height":100}},{"command":"node.create","params":{"type":"rectangle","parent":{"$ref":"card"},"width":20,"height":20}},{"command":"edit.duplicate","as":"copy","params":{"ids":[{"$ref":"card"}]}},{"command":"node.set","params":{"ids":[{"$ref":"copy","field":"ids/0"}],"props":{"name":"Card copy"}}}]}}}
```

#### Library projects and the capture service

The `project.*` commands of [MCP: Library projects](mcp.md#library-projects) run through `engine.execute` like any
other command. `project.settings`, `project.openFolder` and `project.newFolder` only ask the interface for a window
or a dialog, which a programmatic call never opens. `project.addSource {package, version?}`,
`project.addSources {line}`, `project.setSourceVersion {package, version}` or `{packages, version}`, `project.reinstallSources {}` and `project.removeSource {package}` do not wait for npm: they return
`{job}` at once, the app's capture service installs or removes the package on its own thread, and the app applies
the finished job (it writes `uigen.json`) on its next frame. Poll `engine.execute {command: "capture.status",
params: {job?}}` until the job is `done` or `failed` (`error` carries the service's message), then read the result
with `project.info`. Without a capture service (no Node, or the browser build) the three commands answer `bad`. The
service and its protocol are described in [UIGEN: Service de capture](uigen/plan-import.md#service-de-capture).

`uigen.importComponent {package, component, styleProp?, sizeProp?, styles?, sizes?, states?, themes?, parent?, x?, y?, stage?, animations?}`
([MCP: Component import](mcp.md#component-import)) works the same way: it returns `{job}` (the `inspect` job) at once,
the renders follow as the app pumps the capture service, and the component set is written in one undo step when
the last one is back. Poll `capture.status {job}` until that job is `done` or `failed` (`error` names the step that
failed, e.g. `inspect: component "Nope" not found in @mui/material`), then find the new component set with
`document.inspect` (it is selected). `uigen.importDom {snapshot, parent?, x?, y?, full?}` takes a snapshot you
hold yourself and answers `{root, warnings}` directly; `full: true` asks for the complete translation.

The playground commands (`playground.open`, `playground.refresh`, `playground.close`, `playground.template`,
`playground.resize`, `playground.state`, `playground.input`, `playground.set`, `playground.undo`, `playground.redo`,
`playground.revert`, `playground.random`, `playground.saveTheme`, `playground.code`; [MCP: Playground](mcp.md#playground))
also run through `engine.execute`. `playground.open` and `playground.refresh` return `{job}` at once and export the
applied theme into `<project>/.uigen/playground/`, which must be inside `--automation-write-root`; the page then opens in
the capture service on the app's next frames. Playground jobs are not listed by `capture.status`: poll `engine.execute
{command: "playground.state"}` until `status` is `ready` (or `failed`, with `error`). The other commands are answered at
once and need a ready playground. No command returns the page's image; the app shows it in the Playground tab (it asks
the service for an image about thirty times a second while the tab is in front), and `ui.screenshot` captures the window.
Without a capture service, or in the browser build, `playground.open` answers `bad`.

`uigen.listComponents {package, refresh?}` ([MCP: Component import](mcp.md#component-import)) is a capture job too: it
returns `{job}` (an `inspect` job) and the components of the source come back as the event of that job, which
`capture.status {job}` shows `done` or `failed`; the answer is kept in the session, so a second call returns
`{cached: true, …}` with no job. `uigen.importWindow {package?}` only asks the interface for the import window,
which a programmatic call never opens. `uigen.imported {}` and `uigen.placeImported {set, page?}`
([MCP: Component import](mcp.md#component-import)) answer directly, with no job: the first lists the component sets of
the import pages by source, the second copies one onto a page of the library in one undo step. `uigen.themes {}` and
`uigen.applyTheme {theme}` ([MCP: Component import](mcp.md#component-import)) answer directly too: the first lists the
token themes of the file, the second makes the library components follow one in one undo step.

`uigen.compare {set?, theme?, variants?}`, `uigen.sourceDiff {copy}`, `uigen.applyDifferences {copy, ids?}` and
`panel.compare {}` ([MCP: Component import](mcp.md#component-import)) answer directly as well: the first measures an imported
set against the images its import kept (read only inside `.uigen/captures`), the third changes the copy in one undo step,
the last only asks the interface to open the Compare tab. `uigen.updateSource {package, components?, roleNames?}` is a capture job
like `uigen.importComponent`: it returns `{jobs, components}` at once, and each job ends with its own event, which
`capture.status {job}` shows `done` or `failed`.

### `engine.commands {}`

Every command: `[{id, label, params, menu, shortcut, undoable}]`.

### `document.inspect {depth?, allPages?}`

The active document as JSON: name, path, dirty, currentPage, selection, pages (the current page's
layer tree to `depth`, default 8; others summarised unless `allPages`), styles, variable count,
images, the colour profile (`colorProfile`: `srgb` or `displayP3`), the text being edited and the
undo history.

### `node.get {id, depth?}`

One layer (children to `depth`, default 1). Fields: id, name, type, x, y, width, height, rotation,
absoluteBounds, visible, locked, opacity, blend, fills, strokes, strokeWeight, effects, cornerRadius,
layout (including grid `columnTracks` / `rowTracks`), sizing, `minWidth` / `maxWidth` /
`minHeight` / `maxHeight`, `absolute`, constraints (children of plain frames and absolute
children), `gridCell` (a grid auto layout child's column, row and spans), clip, text properties,
component/instance links, overrides, `slot` (a slot layer's property name), boundVariables, interactions, exports,
`devStatus` (handoff status), `devChanged` (changed since it was marked ready for dev),
`annotations` (Dev Mode notes: `[{label, properties, category?}]`), `codeLinks` (a component's
links to code: `[{language, importPath, name, template}]`), `devResources` (links for developers,
its own and inherited: `[{name, url, index?, from?}]`), `measurements` (Dev Mode measurements
starting from the layer: `[{to} | {side}]`), `pluginData` (layer data written with
`node.set {props: {pluginData: {key: value}}}`, pages included: `{key: value}`, values as strings;
only when there is some). The auto layout props
that `node.set` accepts are listed in [mcp.md](mcp.md#conventions-agents-must-know).

### `ui.inspect {}`

UI state: panels and tabs (`panels`: the `left` and `right` panels' widths as asked for and whether the left one is closed, `leftCollapsed`), the cursor the canvas shows (`cursor`: a CSS cursor name such as `default`, `crosshair`, `ew-resize`, `grab`, `copy`, `not-allowed`, `zoom-in`, or one FigCraft draws: `rotate <degrees>`, `pen`, `pen-close`, `pen-add`, `pen-bend`, `eyedropper`, `comment`; `null` while the pointer is elsewhere or over something with a cursor of its own, a ruler guide or a gradient handle; the engine's own hint is `tool.overlay.cursor`), current tool (`tool`: its id, overlay hints and, for a selected
ellipse, its arc handles `arcHandles`, for a selected rectangle, frame, polygon or star, its corner radius and ratio handles `shapeHandles`; see [mcp.md](mcp.md#measuring-the-scale-tool-arc-handles-and-selecting)), canvas rect, dialogs, view, timings (`frameMs`,
`renderMs`), frames drawn since start or the last `ui.set {resetFrameStats: true}` (`framesDrawn`;
an idle window draws none) with the most frequent repaint requests behind them (`repaintCauses`:
`[{at: "file:line", count}]`, every request made in the frame before each one, delayed ones such as
the autosave deadline included; egui's `context.rs` begin-pass line means input events) and what the last canvas render had to do (`canvas.renderWork`: `full`, `partial` with
the `pixels` re-rendered after a pan or an edit, or `reused`), whether the canvas shows a coarse
zoom preview or the refined image (`canvas.quality`: `preview` or `refined`) and the time from
asking for a render to its preview and its refined image being handed to egui as the canvas
texture (`canvas.previewLatencyMs`, `canvas.refineLatencyMs`, on a monotonic clock, rendering
included in the browser too; the GPU upload and presenting the frame come after; `null` until
there was one). `importReport` holds the Import report window's content (`{name, pages, layers, images, appended, placed, warnings, warningCount}`,
`null` while none is open). The window opens only for a file the user opened or imported in the app
(File ▸ Open or Import, Open recent, a recent file on the home screen, a drop, a paste, the system's open event).
Emulated user input counts as the user's action: `ui.input` and `ui.key` (for example a Cmd+V paste or a click
on a menu item) can open it. Commands sent through this channel with `engine.execute`, MCP, `window.new {path}`
and files named on the command line never open it.
`file.open` and `file.import` return the counts as `summary`. While a prototype plays, `presenter`
(full window) and `preview` (the preview pane) hold the player's state: the `current` frame and its
`name`, `start`, `history`, open `overlays`, how many delayed mouse triggers are `waiting`, the screen's `scroll` offset and `scrollRange`, the
`device` (and whether it is turned to `landscape`, its `deviceColor`) and the `viewport` of the frame
its screen shows, the presented frame's `frameSize` and, for a responsive prototype, the screen it
was resized to (`responsive`: `[w, h]`, else `null`), the size option (`scale`:
`fit|fitWidth|fill|actual`, `deviceFrame`), the `zoom` it shows at, how
far the view is panned (`pan`, `panRange`), whether a scroll is still coasting (`flinging`), the
active screen's `hotspots` (`[{id, rect: [x0, y0, x1, y1]}]` in window points, bottom-most first),
whether a click where nothing reacts flashes them (`hotspotHints`) and they are flashing now
(`hotspotFlash`), and whether the Inspect panel shows (`inspect`) with the layer it shows
(`inspected`), and the link an Open link notice waits to open (`linkNotice`, else `null`) (`null`
when nothing plays). `presentOptions` holds the options the next presentation
starts with (`scale`, `deviceFrame`, `comments`, `hotspotHints`, `inspect`).
`textPreview` is true while a live text preview shows on the canvas (a hovered text style or named
instance, a dragged axis slider; see `text.preview` in [mcp.md](mcp.md)).
`focus` is keyboard navigation's state: the `region` with the keyboard (`toolbar`, `layers`,
`canvas` or `properties`), whether `keyboard` mode is on (F6 or Tab moved the focus since the last
pointer press, so the focus ring shows) and the name of the `control` with the keyboard (what a
screen reader announces; empty when nothing has it). `reduceMotion` is true when the preference or
the system's accessibility setting turns the interface's animations off. `uiScale` is the interface
size: the `choice` (relative to the system's display scale, `pref.uiScale`), the `zoom` drawn
(relative too), what that is in physical pixels per point (`drawn`) and whether the window is too
small for the choice (`limited`, so `zoom` is smaller). `live` is the live
session of the window's active file as the window shows it (sessions are per file: another tab
shows its own, or `status: "off"`): `session` (its id, null when the file isn't in one),
`sessions` (how many the app has running), `status`, `role`, the number of `peers`, who you `follow`, the
peers whose cursors are drawn on the canvas (`cursorsShown`), whether the Join and Publish to
server dialogs are open (`joinDialog`, `publishDialog`; Escape closes them), the join code the
Share popover shows (`code`, with `share: {via, access}`; null when the file isn't shared with
one), what is typed in the popover's and the home screen's join field (`joinField`), whether the
popover has Local network chosen and Advanced open (`local`, `advanced`), and why the active
document can't be edited now (`readOnly`: the "View only" or "Publishing" reason, else null); the
session itself is `engine.execute {command: "collab.peers"}` (sharing with a join code, hosting,
joining and publishing are `collab.share`, `collab.host`, `collab.join` and `collab.publish`, opening invite links `collab.registerLinks`,
see [mcp.md](mcp.md#live-sessions)). The app also joins the invite link it
was started with (`figcraft "figcraft://join?…"`). `online` is the home screen's Online section:
whether it shows (`open`), your online files as its rows read (`files: [{file, name, detail,
shared, state, hasOwnerToken}]`; never a token or a link of yours), whether the servers are being
asked (`checking`), how many couldn't say (`problems`), and the file whose removal is being
confirmed or renamed (`confirmRemove`, `rename`; Escape closes them); the list itself is
`engine.execute {command: "collab.files"}`. `tabs` lists the open files as the tab strip shows
them: `[{name, live}]`, `live` being `"shared"` (in a live session others get into), `"private"`
(a file of yours on a server, open only to you) or null.

### `ui.pointer {kind, x, y | sx, sy, to?, steps?, shift?, alt?, cmd?, ctrl?, pressure?, pressureTo?}`

A mouse gesture with the current tool. `kind`: `down`, `move`, `up`, `click`, `double`, `hover`,
`drag` (needs `to: [x, y]`; `steps` intermediate moves, default 8). Position in page coordinates
(`x`, `y`) or screen pixels (`sx`, `sy`). `pressure` (0–1) is pen pressure (the Pencil records
it); a drag's pressure goes from `pressure` to `pressureTo`. Returns `{selection, tool}`. With the Move and Scale
tools, a press on a frame's or section's name label (as the canvas last drew it, along a rotated
frame's top edge) selects and drags that frame; a double-click there renames the frame in place in
the desktop app (the name field opens over the label).

```json
{"id":4,"method":"engine.execute","params":{"command":"tool.set","params":{"tool":"ellipse"}}}
{"id":5,"method":"ui.pointer","params":{"kind":"drag","x":200,"y":200,"to":[300,260],"shift":true}}
```

### `ui.key {key, shift?, alt?, cmd?, ctrl?}`

A key as if typed on the canvas (in vector edit mode the edit keys come first: `V` `P` `B` `Q` `X` `Shift+E` `Shift+W` tools, arrows nudge points, `Delete`/`Shift+Delete`, `Cmd+A`, `Cmd+J`; with the shape builder, `Enter` finishes): a menu shortcut (`{"key":"G","cmd":true}` groups), a tool key,
`Escape`, `Delete`/`Backspace`, `Enter`, number keys (opacity; two quick digits make an exact percentage), arrows (nudge; shift = big nudge; auto layout children move one place instead: along the flow, between wrapped rows, to the next grid cell, or into the neighbouring nested auto layout frame; with `cmd` they resize the selection). With the Comment
tool, `ArrowUp`/`ArrowDown` open the previous/next thread of the Comments panel's list instead
(returns `{"comment": id}`). While text is being edited, the text shortcuts (Bold, alignment,
size steps, Save…) run and other keys go to the text editor. Shortcuts follow the platform the app
runs on (`cmd` is Cmd on macOS and Ctrl elsewhere; on macOS `ctrl` is the separate Control key).
Returns `{"command": id}` for shortcuts.

### `ui.type {text}`

Type into the text being edited (start editing with a double-click, `text.edit.begin {id}`, or the
text tool).

### `ui.input {events}`

Raw window input for the next frame, as if from the mouse and keyboard: it reaches the panels,
popovers and fields (`ui.pointer` and `ui.key` drive the canvas tools and shortcuts instead).
`events` is a list of `{move: [x, y]}`, `{down: [x, y], button?: "secondary"}`,
`{up: [x, y], button?}`, `{key, shift?, alt?, cmd?}` (pressed and released; egui key names such as
`ArrowLeft`, `Escape`, `A`), `{text}` (typed text), `{scroll: [dx, dy], shift?, alt?, cmd?}` (a
mouse wheel at the pointer: the content moves by that many points, so a negative `dy` scrolls down)
`{leave: true}` (the pointer left the window), `{hold: {alt?, shift?, cmd?}}` (keep those modifier
keys down for the frames that follow, e.g. Alt while dragging a component from the Assets panel onto
an instance to swap it; all false releases them) and `{gamepad: "A" | "Gamepad A" | 0}` (a
controller button pressed in the prototype playing, the presenter's or else the preview pane's: a
short name, a key/gamepad trigger's name or a standard layout index 0–16; an error when no prototype
plays); positions are window points. Events of one call
arrive in the same frame, so send a press and its release (or a double-click's two clicks) as
separate calls. Returns `{queued}`.

```json
{"id":6,"method":"ui.input","params":{"events":[{"move":[1000,315]}]}}
{"id":7,"method":"ui.input","params":{"events":[{"down":[1000,315]}]}}
{"id":8,"method":"ui.input","params":{"events":[{"up":[1000,315]}]}}
```

### `ui.menu.invoke {id}`

Invoke a menu item exactly like clicking it (this may open a dialog). Returns `{ok, lastError}`.

### `ui.set {rightTab?, leftTab?, theme?, home?, homeSection?, homeSearch?, homeRecentsView?, homeRecentsSort?, homeRecentsOrder?, palette?, mainMenu?, focusRegion?, colorPicker?, variables?, layerFilter?, collapseLayers?, exportPreview?, preview?, presentScroll?, colorProfileDialog?, stylePicker?, stylePickerEdit?, stylePickerView?, iconGallery?, protoEditor?, contextMenu?, contextSelectLayer?, commentOpen?, commentDraft?, commentFilter?, commentSort?, commentSearch?, commentReply?, commentEdit?, presentComments?, devCompare?, devCompareMode?, devCompareSplit?, annotationFilter?, resetFrameStats?, pluginsWindow?, pluginsTab?, pluginConfirm?, pluginForm?, devCompareVersion?}`

Switch panels and modes: `rightTab` `design|prototype|inspect`, `leftTab` `file|components` (`assets` is accepted as an alias of `components`), `theme`,
`home` (bool), `homeSection` (`recents`, `online` or `templates`: which section the home screen shows, as `ui.inspect` reports it; opening
`online` asks each server once what became of your online files), `homeRecentsView` (`grid|list`), `homeRecentsSort` (`modified|name|created`) and `homeRecentsOrder` (`desc|asc`): how the home screen shows its recent files (kept in the preferences; `ui.inspect` reports them under `homeRecents` with each file and whether its picture is in), `palette` (bool: the Actions palette), `mainMenu` (bool, or a top-level menu such
as `"Plugins"` to open the main menu with it open), `homeSearch` (text, at most 256 characters: filters locally known open, recent and online files plus starter templates in Home; an empty string returns to the selected section),
`pluginsWindow` (bool: Plugins ▸ Manage
plugins…), `pluginsTab` (`installed|browse`: that window's tab), `pluginConfirm` (a plugin id of the
directory listed last: its install confirmation shows, as its Install button would; `null` closes
it), `pluginForm` (`{id, command?}`: start that plugin command like its menu item, opening
its parameters form when it has parameters; `null` closes the form), `focusRegion`
(`toolbar|layers|canvas|properties`: give that region the keyboard as F6 would, on the next frame),
`iconGallery` (bool:
every UI icon at 12, 16, 24 and 32 px over the window, for reviewing the icon set), `colorPicker`
(`{fills?, index?}`: open the colour picker on the selection's paint, which notes a Display P3 colour
outside sRGB; `null` closes it), `variables` (bool,
or a collection id or name to show, or `{library, collection?}` for an enabled library's collection: the
Variables editor), `layerFilter` (string:
open the layers search with that filter; `null` closes it), `collapseLayers` (true: collapse the
layers tree), `exportPreview` (bool: the Design panel Export section's preview), `preview` (true or a
frame id: open the prototype preview pane like `proto.preview`; false or `null` closes it),
`presentScroll` (`[x, y]`: scroll the presented screen, the full window's else the pane's, clamped
to what it can scroll), `colorProfileDialog` (`srgb|displayP3`: open File ▸ Color profile with that
profile chosen; `null` closes it), `stylePicker` (`fill|stroke|effect|grid|text`: open the style
picker for the selection, `text` being the text style browser of the Typography section; `null`
closes it) with `stylePickerEdit` (a style id or name: open its editor), `stylePickerView`
(`list|grid`), `protoEditor` (`[layer id, interaction index]`: open that interaction's editor in the
Prototype panel; `null` closes it), `contextMenu` (`[sx, sy]`: right-click the canvas at that screen
point, selecting what is under it (with `cmd: true`, the nested layer) and opening the context menu;
`null` closes it), `contextSelectLayer` (bool: open the context menu's Select layer submenu, which
lists every layer under the click). Comments: `commentOpen` (a thread id opens its popover, `null`
closes it), `commentDraft` (`{x, y}`: start a new comment there), `commentFilter`
(`open|resolved|mine|unread|all`: what the Comments panel and the pins show), `commentSort`
(`date|page`), `commentSearch` (the panel's search text), `commentReply` (the open thread's reply
text, focused with the cursor at its end: an `@` there shows the mention picker), `commentEdit` (a
message index of the open thread to edit, 0 for the first; `null` stops), `presentComments` (bool:
the presenter shows the presented frame's comments, also Shift+C there). Dev Mode: `devCompare` (a
frame or section id opens the Compare changes view, like `dev.compare {id, show:
true}`; `null` closes it) with `devCompareVersion` (a version id to compare with; `null`: when it
was marked ready for dev), `devCompareMode` (`sideBySide|overlay`) and `devCompareSplit` (the
overlay slider, 0–1), `annotationFilter` (`all`, `none` for uncategorised, or a category id or name:
the annotations the canvas shows). `resetFrameStats` (true: start `framesDrawn` and
`repaintCauses` over). Returns `ui.inspect` (which reports `stylePicker`,
`stylePickerView`, `comments`: `open`, `draft`, `filter`, `sort`, `search`, `editing` and the
`mentions` the picker offers, `devCompare` (`{id, mode, split, version}` or `null`), `devFocus` (the frame the
focus view shows, or `null`) and `annotationFilter`).

### `ui.screenshot {path?}`

PNG of the whole window (`window` picks one), UI included (an idle one draws the frame to capture;
a hidden one, or one whose platform visibility is not reported, is asked to paint and the request
is parked with its deadline until it does). Returns
`{path, width, height}`, without `path` `{format: "png", width, height, data}` (base64). Requests
are bounded to four pending, 128 MiB of pixels per image and 256 MiB in flight or
completed-but-undrained. The reservation is the larger of the encoder phase (readback +
straight-RGBA copy + PNG) and the reply phase (readback + PNG + base64); the encoder copy is
dropped before base64 is built, and an oversized encoded reply is refused before base64 is
allocated. The reply carries the request's own id, and the capture belongs to the document and
window it was admitted for; cancellation, a tab switch, a resize/DPI/zoom between issuance and
arrival (retried twice, then an error), an unrepresentable current surface (fail closed), and the
60 s deadline never write late files. A hidden window whose pixels arrived is asked to paint the
pass that drains them, so a completed readback is not left to time out.

### `ui.render {path?, scale?, ids?}`

PNG of the document without UI chrome (works without a window). Without `ids` it exports the
selection, else the page's top-level layers. Returns the export result: written to `path`, or
`{format: "png", data}` (base64) without one.

### `window.new {path?, index?}`

Open a window: with open file `index` moved into it (from whichever window has it), the file at
`path`, or a new file. Returns `{window}` (its id); it comes to the front. The browser build and a
running screen reader keep one window (only the main window has an accessibility tree): the file
opens as a tab there and the reply is an error saying so. The `window.new` command does the same
for menus, the Actions palette and `engine.execute`.

### `window.list {}`

Every window: `[{id, main, title, files, active, focused}]` (`files` are its tabs' names, `active`
the one it shows, `focused` the window requests go to). `ui.inspect` reports it as `windows`, plus
`window` (the window it ran in).

### `window.focus {id}`

Bring a window to the front; requests without `window` go to it from now on. The `window.focus`
command does the same.

Closing a window closes its files, asking first when one has unsaved changes; the main window
closing while others are open takes over one of them, and closing the last window quits.

### `app.quit {}`

Close the app (every window, without asking about unsaved changes).

## Example session (shell)

```sh
port=7979
uigen --control $port --automation-write-root /tmp &
sleep 2   # the token file appears once the channel is listening
token=$(cat "${XDG_CONFIG_HOME:-$HOME/.config}/FigCraft/control-token-$port")
exec 3<>/dev/tcp/127.0.0.1/$port
echo "{\"auth\":\"$token\"}" >&3
read -r ack <&3; echo "$ack"   # {"id":null,"ok":true,"result":"authenticated"}
echo '{"id":1,"method":"engine.execute","params":{"command":"frame.create","params":{"preset":"iPhone 16","x":0,"y":0}}}' >&3
read -r reply <&3; echo "$reply"
echo '{"id":2,"method":"ui.screenshot","params":{"path":"/tmp/figcraft.png"}}' >&3
read -r reply <&3; echo "$reply"
```

For agents, prefer the MCP server (`figcraft-cli mcp --attach 7979`, see `docs/mcp.md`): it wraps
these methods in documented tools and returns images inline.

## In the browser: `window.figcraft`

The web build has no TCP server (a page can't listen on a socket), so it offers the same methods to the
page itself, as promises on `window.figcraft` (`apps/figcraft-web/src/api.rs`). An agent driving FigCraft in a
browser tab, or a host app showing it in an `<iframe>`, uses the commands it would send to the desktop
app. It is opt-in: the page installs `window.figcraft` only when its URL has a `control` query entry
(`index.html?control`, which can be combined with other flags: `?webgl&control`).

| Call | |
|---|---|
| `figcraft.request(method, params)` | any control method of this document → its `result` |
| `figcraft.execute(command, params)` | `engine.execute` |
| `figcraft.commands()` / `figcraft.inspect()` | `engine.commands` / the inspect method |
| `figcraft.screenshot(params)` | `ui.screenshot` (no `path`: the PNG comes back inline) |
| `figcraft.openFile(name, bytes)` | runs `file.open` with the bytes as `data` and resolves with its result |
| `figcraft.captureDownloads(on)` | while on, a save or export is kept in the page instead of downloaded |
| `figcraft.files()` / `figcraft.readFile(name)` | the kept files (`{path, size}`) / one of them as a `Uint8Array` |

Errors reject with an `Error` carrying the method's message. Requests run on the UI's next frame, so a
hidden tab (whose frames the browser pauses) answers when it is shown again.
