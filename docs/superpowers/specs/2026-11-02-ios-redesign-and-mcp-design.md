# iOS-style redesign and MCP support: design

Status: Part A approved in the brainstorming session. Part B was drafted from the decisions made there
("both" modes, "on by default, full control", "live hand-over") and is flagged in the PR for review.

## Part A: iOS-style visual system (egui 0.37)

**Colours.** One flat palette per theme (`src/theme.rs`). No gradients, no drop shadows (egui shadows are blurred ramps);
surfaces are separated by flat fills and 1px hairlines.

| Token | Light | Dark |
| --- | --- | --- |
| Background | #F2F2F7 | #000000 |
| Card | #FFFFFF | #1C1C1E |
| Raised card | #FFFFFF | #2C2C2E |
| Separator | #C6C6C8 | #38383A |
| Label | #000000 | #FFFFFF |
| Secondary label | #8A8A8E | #98989F |
| Blue / green / orange / red | #007AFF #34C759 #FF9500 #FF3B30 | #0A84FF #30D158 #FF9F0A #FF453A |

**Mode.** System / Light / Dark, default System, following the OS live. Stored in settings with a serde default so old
settings files still load. Switched under View → Appearance. Switching cross-fades the palette over about 200 ms.

**Shape and type.** Radius 12 for windows and cards, 10 for toolbar tiles, 8 for list selection; buttons and progress bars
are capsules. 13pt body, 11pt caption, 17pt titles; system font.

**Widget kit** (colours from the palette, so dark mode is automatic): buttons (filled, tinted, plain), toggle switch,
segmented control, grouped card with inset rows, capsule progress bar, spinner wheel (12 fading spokes), wheel picker
(scheduler start/stop times), toast.

**Main window.** Monoline glyph toolbar; sidebar source list with a rounded selection pill that slides between rows;
download list with taller rows, no zebra stripes, hairline separators, status dot plus text, thin capsule progress bar;
per-connection strip with rounded ends.

**Dialogs.** Radius 14, hairline border, 20px padding. Option groups become grouped cards, checkboxes become switches,
the primary action is a filled capsule at the bottom right.

**Icons.** `icons.rs` redrawn as 1.5px monoline glyphs that take a tint.

**Animation** (ease-out, 120 to 220 ms, egui's `animate_*` helpers): button colour fades on hover and shrinks to 97% on
press; switch knob, segmented highlight and sidebar pill slide; progress bars glide; spinner while connecting, grabbing
links and checking SHA-256; windows fade in; toasts slide up and fade; drop hint fades. The app repaints only while
something is moving.

**Not doing:** blur/translucency, shadows, a custom font, a phone layout.

**Testing.** Unit tests: label-on-card contrast of at least 4.5:1 in both themes, palette blend endpoints, an old
settings file without a theme field still loads. `--screenshot` gains `--theme light|dark`. README shows both.

## Part B: MCP server

**Entry point.** `swift_download_manager --mcp` speaks MCP over stdio (newline-delimited JSON-RPC 2.0; protocol versions
2025-06-18, 2025-03-26 and 2024-11-05 are accepted). Nothing but JSON-RPC is written to stdout; logs go to stderr.
The protocol layer is hand-written on `serde_json`, so there is no new runtime dependency apart from `getrandom` for the token.

**One command layer, two backends.** All tool calls become a `Command` handled by `App::run_command`, the same code in both
modes.
- *App mode:* the open window owns the state. `--mcp` forwards each command to the window's control listener.
- *Headless mode:* no window is open. `--mcp` builds a real `App` without a window (`App::tick()` has no UI
  dependency) and drives it from a 200 ms loop, so queues, schedules, limits and resume behave exactly as in the window.

**Control listener** (`src/mcp/control.rs`), separate from the browser bridge on port 9614 (which allows any website):
127.0.0.1 only, ephemeral port published in `control.json` next to the settings; every request needs
`Authorization: Bearer <token>`; the token is 32 random bytes (hex) in `mcp-token`, created on first use (0600 on unix);
requests carrying an `Origin` header, a non-loopback `Host`, or a body over 64 KiB are rejected; no CORS headers.

**Single writer and live hand-over.** `state.lock` (exclusive `File::try_lock`, released by the OS on crash) names the
process that owns `downloads.json`.
- Window opens while headless owns the state: the window calls `POST /handoff` on the headless process's listener.
  Headless pauses running downloads, saves, releases the lock and drops to proxy mode (it now forwards to the window);
  the window takes the lock, loads the file and resumes the downloads that were running (`handoff_resume` flag).
- Window closes while `--mcp` is attached: the window saves on exit (new `on_exit`); the next tool call finds no window,
  takes the lock, loads the file and continues headless.
- If the lock cannot be obtained within 5 seconds the window starts with MCP control disabled and shows a toast.
  File writes stay atomic (temp file plus rename), so the worst case is a lost update, never a torn file.

**Tools.** Annotations follow the MCP spec so clients can prompt accordingly.

| Tool | Arguments | Hints |
| --- | --- | --- |
| `list_downloads` | `status?`, `query?`, `limit?` | read-only |
| `get_download` | `id` | read-only |
| `get_status` | none | read-only |
| `list_queues` | none | read-only |
| `add_download` | `url`, `name?`, `category?`, `queue?`, `start?` | open-world |
| `pause_download` / `resume_download` | `id` | idempotent |
| `start_queue` / `stop_queue` | `name` | idempotent |
| `remove_download` | `id` | destructive: removes the list entry only |

**Safety rules.** `add_download` accepts only http and https links, sanitises `name`, and takes no folder argument: the
destination is always the folder of the chosen category, so an assistant cannot write outside the download folders.
`remove_download` never deletes files from disk, including partial files. MCP is on by default with an in-app off switch
(Options → AI assistants, `mcp_enabled`, default true); when off, the control listener refuses and headless mode answers
every call with an explanatory error.

**Known limit.** Windows release builds use the GUI subsystem; `--mcp` relies on the client passing pipes for
stdin/stdout, which cannot be verified from this Linux machine.
