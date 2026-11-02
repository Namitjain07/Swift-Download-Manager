# iOS-style Redesign and MCP Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restyle the egui app in a flat, minimal iOS look with light and dark mode and smooth animation, and add an MCP server (`--mcp`) that works with the open window or headless.

**Architecture:** Part A adds `src/theme.rs` (palettes, blend, mode) and turns `src/widgets.rs` into a widget kit under `src/widgets/`; existing views and dialogs are restyled on top of it. Part B adds `src/mcp/` (protocol, control listener, stdio server, lock and hand-over) around one `App::run_command` that serves both the window and a windowless `App` driven by a timer.

**Tech Stack:** Rust 2024, eframe/egui 0.37, serde_json; new dependency: `getrandom` 0.3 (already in `Cargo.lock`).

**Spec:** `docs/superpowers/specs/2026-11-02-ios-redesign-and-mcp-design.md`

## Global Constraints

- Work in `source/`. Branch `feature/ios-redesign-and-mcp`; one small commit per task; every commit passes `cargo build --locked` and `cargo test --locked`; the 20 existing tests stay green.
- Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`; the PR body ends with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
- No gradients, no shadows (`Shadow::NONE` everywhere), no blur or translucency.
- Palette hex values exactly as in the spec table. Radius 12 windows/cards (dialogs 14), 10 toolbar tiles, 8 list selection; buttons and progress bars are capsules. Text 13 body, 11 caption, 17 title.
- Animations ease-out, 120 to 220 ms (palette cross-fade 200 ms), via `ctx.animate_*`; repaint only while an animation is unfinished.
- New settings fields are `#[serde(default)]` so existing `downloads.json` files load.
- Rust stdout in `--mcp` mode carries only JSON-RPC; logs go to stderr.
- MCP never deletes files from disk and accepts no folder argument.

## Review Focus

- Old settings file (no `theme`, no `mcp_enabled`) must still load: pinned in Task 3 and Task 20.
- `add_download` with `file://`, `javascript:`, `../../x` names or a `folder` argument must not write outside the download folders: pinned in Task 16.
- Control listener must reject a missing/wrong token, an `Origin` header, a foreign `Host` and an oversized body: pinned in Task 17.
- Hand-over while a download is running must keep its progress and resume byte-exact: pinned in Task 19.
- Garbage on stdin (invalid JSON, wrong types, unknown tool, notification without id) must not kill the server or pollute stdout: pinned in Task 18.

---

## Part A: visual redesign

### Task 1: Spec and plan documents

**Files:** Create `docs/superpowers/specs/2026-11-02-ios-redesign-and-mcp-design.md`, `docs/superpowers/plans/2026-11-02-ios-redesign-and-mcp.md` (both already written).

- [ ] **Step 1: Commit**

```bash
git add docs/superpowers && git commit -m "docs: add design spec and implementation plan for the redesign and MCP"
```

### Task 2: Palette, blend and contrast (`src/theme.rs`)

**Files:** Create `source/src/theme.rs`; modify `source/src/main.rs` (`mod theme;`).

**Interfaces:**
- Produces: `pub struct Palette { bg, card, raised, separator, label, secondary, blue, green, orange, red: Color32 }` (`Copy`); `Palette::LIGHT`, `Palette::DARK` consts; `Palette::lerp(&self, other: &Palette, t: f32) -> Palette`; `pub fn contrast_ratio(a: Color32, b: Color32) -> f32` (WCAG relative luminance); `#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Default)] pub enum ThemeMode { #[default] System, Light, Dark }`.

- [ ] **Step 1: Write failing tests** in `theme.rs`: `label_on_card_has_4_5_contrast_in_both_themes` (`contrast_ratio(LIGHT.label, LIGHT.card) >= 4.5`, same for DARK); `secondary_label_on_card_is_at_least_3` (floor; #8A8A8E on white is about 3.4, so 4.5 is not achievable with the specified colour); `lerp_endpoints_match` (`lerp(&b, 0.0) == a`, `lerp(&b, 1.0) == b` for every field); `palette_matches_spec_hex` (`LIGHT.bg == from_rgb(0xF2,0xF2,0xF7)`, `DARK.card == from_rgb(0x1C,0x1C,0x1E)`, `DARK.blue == from_rgb(0x0A,0x84,0xFF)`).
- [ ] **Step 2: Run** `cargo test --locked theme::` → FAIL (items missing).
- [ ] **Step 3: Implement** the items above; `lerp` blends in gamma space per channel (`Color32::lerp_to_gamma`).
- [ ] **Step 4: Run** `cargo test --locked` → all pass.
- [ ] **Step 5: Commit** `feat(theme): iOS light and dark palettes with blend and contrast helpers`.

### Task 3: Theme mode in settings, cross-fade, Appearance menu, `--theme` flag

**Files:** Modify `source/src/app.rs` (`Settings.theme: ThemeMode`, `App.theme_override`, `App::new`), `source/src/app/view.rs` (View menu), `source/src/main.rs` (`--theme light|dark`), `source/src/theme.rs`.

**Interfaces:**
- Consumes: Task 2 items.
- Produces: `pub fn theme::apply(ctx: &egui::Context, mode: ThemeMode)` called once per frame from `App::ui`; it resolves dark/light (`Light`/`Dark` fixed, `System` → `ctx.system_theme()` falling back to `Light`), animates `t` with `ctx.animate_value_with_time(Id::new("theme_t"), target, 0.2)`, and writes the blended palette into egui's `Visuals` (panel/window/faint fills, widget fills and strokes, selection, hyperlink, text colour, `dark_mode`) only while `t` is moving or the target changed. `Settings.theme` is `#[serde(default)]`.

- [ ] **Step 1: Write failing test** `old_settings_without_theme_still_load` in `app.rs` tests: `serde_json::from_str::<Settings>(r#"{"connections":4}"#)` succeeds with `theme == ThemeMode::System` and `connections == 4`.
- [ ] **Step 2: Run** `cargo test --locked old_settings` → FAIL.
- [ ] **Step 3: Implement** the field, `theme::apply`, a View → Appearance submenu using `selectable_value` for the three modes (sets `dirty`), and `--theme` parsing in `main.rs` that passes `Some(ThemeMode)` through `App::new` (forces the mode for that run without saving it).
- [ ] **Step 4: Run** `cargo test --locked` → pass; `cargo run -- --screenshot $SCRATCH/dark.png --theme dark` and `--theme light`; open both PNGs and check backgrounds are #000000 and #F2F2F7.
- [ ] **Step 5: Commit** `feat(theme): follow the system theme with a 200 ms cross-fade and a View > Appearance switch`.

### Task 4: Flat global style

**Files:** Modify `source/src/app.rs` (replace `setup_style`), `source/src/theme.rs`.

**Interfaces:** Produces `pub fn theme::setup(ctx: &egui::Context)` (called from `App::new`): spacing (`item_spacing` 8×6, `button_padding` 14×6), `window_corner_radius` 14, `menu_corner_radius` 10, widget corner radius 8, `window_shadow = popup_shadow = Shadow::NONE`, `striped = false`, text styles Body 13, Small 11, Heading 17, hairline `window_stroke`.

- [ ] **Step 1: Implement**; remove `setup_style`.
- [ ] **Step 2: Verify** build, tests, both screenshots look flat (no shadows around the menu or windows).
- [ ] **Step 3: Commit** `feat(theme): flat iOS spacing, radii and type scale with no shadows or stripes`.

### Task 5: Widget kit part 1: animation helper, buttons, switch

**Files:** Move `source/src/widgets.rs` → `source/src/widgets/mod.rs` (git mv, re-export existing functions unchanged); create `source/src/widgets/anim.rs`, `button.rs`, `switch.rs`.

**Interfaces:**
- Produces: `anim::ease_out(t: f32) -> f32` (cubic); `anim::fade(ctx, id: Id, target: f32, secs: f32) -> f32`; `pub enum ButtonKind { Filled, Tinted, Plain }`; `pub fn button(ui: &mut Ui, kind: ButtonKind, tint: Color32, label: &str) -> Response` (capsule, hover fade 120 ms, 97% scale while pressed, disabled state via `ui.is_enabled()`); `pub fn switch(ui: &mut Ui, on: &mut bool) -> Response` (51×31, knob slides 180 ms, green when on).

- [ ] **Step 1: Write failing test** `ease_out_is_monotonic_with_fixed_endpoints` (`ease_out(0.0)==0.0`, `ease_out(1.0)==1.0`, increasing on 11 samples).
- [ ] **Step 2: Run** `cargo test --locked ease_out` → FAIL; implement `anim.rs`; run → PASS.
- [ ] **Step 3: Implement** `button` and `switch` (colours from `theme::current(ctx)`, a helper returning the live blended `Palette`; add it to `theme.rs`).
- [ ] **Step 4: Verify** builds and tests pass.
- [ ] **Step 5: Commit** `feat(widgets): animated capsule buttons and toggle switch`.

### Task 6: Widget kit part 2: segmented control and grouped card

**Files:** Create `source/src/widgets/segmented.rs`, `card.rs`.

**Interfaces:**
- Produces: `pub fn segmented<T: PartialEq + Copy>(ui, options: &[(T, &str)], value: &mut T) -> Response` (sliding highlight, 180 ms); `pub fn card<R>(ui, add: impl FnOnce(&mut Ui) -> R) -> R` (Card fill, radius 12, hairline, no padding between rows) and `pub fn card_row<R>(ui, last: bool, add: impl FnOnce(&mut Ui) -> R) -> R` (44 px min height, inset separator unless `last`).

- [ ] **Step 1: Implement** both.
- [ ] **Step 2: Verify** build and tests.
- [ ] **Step 3: Commit** `feat(widgets): segmented control and grouped card with inset rows`.

### Task 7: Widget kit part 3: capsule progress and spinner wheel

**Files:** Create `source/src/widgets/progress.rs`, `spinner.rs`; move `mini_bar` and `segment_bar` there.

**Interfaces:**
- Produces: `pub fn capsule_bar(ui, id: Id, frac: f32, tint: Color32, height: f32, text: Option<&str>)` (displayed value glides toward `frac` via `animate_value_with_time(…, 0.22)`); `pub fn spinner(ui, size: f32)` (12 spokes, fading alpha, one revolution per second; requests a repaint every frame only while shown); `segment_bar` gets rounded ends and palette colours.
- Keep `mini_bar` and `segment_bar` call sites compiling (`mini_bar(ui, frac, col, text)` becomes a thin wrapper over `capsule_bar`).

- [ ] **Step 1: Write failing test** `spoke_alpha_fades_around_the_wheel`: `spinner::spoke_alpha(i, phase) -> u8`; the leading spoke is 255, the one before it is lower, all values ≥ 40.
- [ ] **Step 2: Run** → FAIL; implement; run → PASS.
- [ ] **Step 3: Commit** `feat(widgets): gliding capsule progress bar and spinner wheel`.

### Task 8: Widget kit part 4: wheel picker

**Files:** Create `source/src/widgets/wheel.rs`.

**Interfaces:**
- Produces: `pub fn wheel(ui, id: Id, items: &[String], index: &mut usize, width: f32) -> Response` (3 visible rows, mouse wheel and drag change `index` with a 160 ms glide, selected row centred between two hairlines, rows fade with distance); `pub fn time_wheels(ui, id: Id, hhmm: &mut String) -> bool` (hour 00-23 and minute 00-59 wheels; returns true when the value changed; uses `app::actions::normalize_time` output format `HH:MM`).

- [ ] **Step 1: Write failing test** `time_wheels_round_trip`: `wheel::parse_hhmm("07:05") == (7, 5)`, `wheel::format_hhmm(7, 5) == "07:05"`, `parse_hhmm("garbage") == (0, 0)`.
- [ ] **Step 2: Run** → FAIL; implement; run → PASS.
- [ ] **Step 3: Commit** `feat(widgets): wheel picker for times`.

### Task 9: Monoline icons

**Files:** Modify `source/src/icons.rs`.

**Interfaces:** `icons::draw(p, rect, icon, tint: Color32, enabled: bool)` (tint replaces fixed colours; disabled uses `tint.gamma_multiply(0.35)`); `icons::folder(p, rect, col)` becomes an outline; `icons::file_badge(p, rect, cat)` uses the card fill instead of `Color32::WHITE`. All strokes 1.5 px, round caps, built from `line_segment`/`circle_stroke`/`PathShape`. The 12 `Icon` variants and `category_color` keep their names.

- [ ] **Step 1: Implement**; update the one call site in `widgets/mod.rs::tool_button` to pass a tint.
- [ ] **Step 2: Verify** build and tests; both screenshots show the 12 glyphs crisp in dark and light.
- [ ] **Step 3: Commit** `feat(icons): 1.5px monoline glyphs that take a tint`.

### Task 10: Toolbar, menu bar and status bar

**Files:** Modify `source/src/widgets/mod.rs` (`tool_button`, `toolbar_sep`), `source/src/app/view.rs` (`toolbar`, `status_bar`, `menu_bar`).

**Interfaces:** `tool_button` keeps its signature; it draws a 10-radius tile that fades in on hover (120 ms), shrinks to 97% on press, label in Caption. Panels use `theme::current(ctx)` fills with a hairline instead of the hard-coded RGB values; status bar drops `ui.separator()` bars for spacing plus a status dot, and the speed readout uses the Secondary label colour.

- [ ] **Step 1: Implement**; replace every hard-coded `Color32::from_rgb` in these functions with palette lookups.
- [ ] **Step 2: Verify** build, tests, screenshots in both themes.
- [ ] **Step 3: Commit** `feat(ui): glyph toolbar tiles, flat panels and a calmer status bar`.

### Task 11: Sidebar source list with sliding pill

**Files:** Modify `source/src/widgets/mod.rs` (`cat_row`), `source/src/app/view.rs` (`categories_panel`).

**Interfaces:** `cat_row` takes the row's rect only and no longer paints selection; `categories_panel` paints one pill (radius 8, `blue` at 14% alpha) at a y-position animated with `animate_value_with_time(Id::new("pill_y"), target_y, 0.18)` toward the selected row's top, and a hover fill (separator at 40%) per row. Section headers ("Categories", "Queues") use Caption in the Secondary label colour. Running queues show a green dot instead of "▶".

- [ ] **Step 1: Implement** (collect row rects while laying out, paint the pill behind them with `painter.add` on a lower layer index or by reserving a `Shape::Noop` slot and `ui.painter().set(idx, …)`).
- [ ] **Step 2: Verify** build, tests; run the app with `--demo` and click rows: pill glides.
- [ ] **Step 3: Commit** `feat(ui): sidebar selection pill that slides between rows`.

### Task 12: Download list

**Files:** Modify `source/src/app/view.rs` (`table`).

**Interfaces:** `TableBuilder` with `.striped(false)`, row height 34, header 28 in Caption/Secondary, hairline between rows, selected row painted with the 8-radius pill colour, Status column = 8 px dot (green downloading, orange paused, red error, grey queued, blue complete) plus text, with a `capsule_bar` (height 6) under the percentage text for active rows; empty-state label uses Secondary colour and 17pt title.

- [ ] **Step 1: Implement**; keep sorting, selection and context-menu behaviour byte-for-byte (they live outside the cell closures).
- [ ] **Step 2: Verify** build, tests, screenshots in both themes.
- [ ] **Step 3: Commit** `feat(ui): taller download rows with status dots and capsule progress`.

### Task 13: Dialog frame, switches and primary capsule

**Files:** Create `source/src/widgets/dialog.rs`; modify `source/src/app/dialogs.rs`, `tools.rs`, `grabber.rs`.

**Interfaces:**
- Produces: `pub fn dialog_window(title: &str) -> egui::Window<'static>` (radius 14, hairline, 20 px inner margin, `collapsible(false)`, fade-in via `ctx.animate_bool_with_time(Id::new(title), true, 0.18)` applied as `ui.multiply_opacity`); `pub fn primary_button(ui, label) -> Response` (filled capsule, blue); `pub fn toggle_row(ui, label: &str, on: &mut bool, last: bool) -> Response` (card row with `switch`).
- Every `egui::Window::new(…)` in the three files becomes `dialog_window(…)`; every `ui.checkbox(&mut x, "…")` becomes a `toggle_row` inside a `card`; `radio_value`/`selectable_value` groups with two or three options become `segmented`; "Start Download", "Save", "OK" style actions become `primary_button`.

- [ ] **Step 1: Implement** window by window (add, batch, scheduler, queues, options, delete, properties, refresh, grabber, complete, browser, about, progress).
- [ ] **Step 2: Verify** build, tests; open each dialog in the demo and check both themes.
- [ ] **Step 3: Commit** in two commits: `feat(ui): iOS dialog frame with fade-in and capsule primary action` then `feat(ui): switches, segmented controls and grouped cards in dialogs`.

### Task 14: Wheels, spinners, toast and drop hint

**Files:** Modify `source/src/app/tools.rs` (scheduler), `grabber.rs`, `dialogs.rs`, `view.rs` (`toast_area`, `drop_files`).

**Interfaces:** Scheduler start/stop times use `wheel::time_wheels`; a `spinner` shows beside the status text while `Status::Connecting`, while the grabber is fetching, and while `Item::is_checking`; toast slides up 12 px and fades in/out over 200 ms from `Instant` age (replaces the hard cut at 4 s); drop hint card fades with `animate_bool_with_time(…, hovering, 0.15)` and is drawn only while its opacity > 0.

- [ ] **Step 1: Implement**.
- [ ] **Step 2: Verify** build and tests; manual run of the scheduler dialog.
- [ ] **Step 3: Commit** `feat(ui): time wheels, spinners, sliding toast and fading drop hint`.

### Task 15: Screenshots, idle CPU check, docs for Part A

**Files:** Modify `README.md` (root), `source/README.md`, `CHANGELOG.md`; replace `docs/images/screenshot.png`, add `docs/images/screenshot-dark.png`.

- [ ] **Step 1: Capture** `cargo run --release -- --screenshot ../docs/images/screenshot.png --theme light` and `…screenshot-dark.png --theme dark`; open both and check them.
- [ ] **Step 2: Idle check:** run the demo with nothing animating for 10 s and confirm `top -b -n 2 -d 5 -p <pid>` shows under 3% CPU.
- [ ] **Step 3: Docs:** README shows both screenshots and the Appearance switch; CHANGELOG gets an "Unreleased" section.
- [ ] **Step 4: Commit** `docs: new screenshots in light and dark, changelog for the redesign`.

---

## Part B: MCP support

### Task 16: MCP protocol layer and command set

**Files:** Create `source/src/mcp/mod.rs`, `protocol.rs`, `command.rs`; modify `source/src/main.rs` (`mod mcp;`).

**Interfaces:**
- Produces in `protocol.rs`: `pub const SUPPORTED_VERSIONS: [&str; 3]`; `pub fn handle_line(line: &str, run: &mut dyn FnMut(Command) -> Result<serde_json::Value, String>) -> Option<String>` (returns the JSON-RPC response line, or `None` for notifications); implements `initialize` (echoes a supported client version, else the newest; capabilities `{"tools":{}}`; `serverInfo` name `swift-download-manager`), `ping`, `tools/list` (9 tools with JSON-Schema `inputSchema` and `annotations` from the spec table), `tools/call` (maps to `Command`, tool failures return `isError: true` with text, not JSON-RPC errors); parse failure → error `-32700`, unknown method → `-32601`, bad params → `-32602`.
- Produces in `command.rs`: `#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)] pub enum Command { ListDownloads{status: Option<String>, query: Option<String>, limit: Option<usize>}, GetDownload{id: u64}, GetStatus, ListQueues, AddDownload{url: String, name: Option<String>, category: Option<String>, queue: Option<String>, start: Option<bool>}, Pause{id: u64}, Resume{id: u64}, StartQueue{name: String}, StopQueue{name: String}, Remove{id: u64} }`.
- Consumes nothing yet; `App::run_command` comes in Task 17.

- [ ] **Step 1: Write failing tests** in `protocol.rs`: `initialize_negotiates_version` (request `2024-11-05` → same back; request `1999-01-01` → `2025-06-18`); `tools_list_has_nine_tools_with_hints` (`remove_download` has `destructiveHint: true`, `list_downloads` has `readOnlyHint: true`, `add_download` has `openWorldHint: true`); `notification_gets_no_reply` (`notifications/initialized` → `None`); `garbage_lines_get_json_rpc_errors` (`"{nope"` → code `-32700`; `{"jsonrpc":"2.0","id":1,"method":"nope"}` → `-32601`; `tools/call` with `{"name":"get_download","arguments":{"id":"x"}}` → `-32602`); `tool_error_is_not_a_protocol_error` (runner returns `Err("boom")` → result has `isError: true` and text `boom`).
- [ ] **Step 2: Run** `cargo test --locked mcp::` → FAIL.
- [ ] **Step 3: Implement** `Command`, `handle_line`, and the tool table.
- [ ] **Step 4: Run** `cargo test --locked` → pass.
- [ ] **Step 5: Commit** `feat(mcp): JSON-RPC protocol layer with tool schemas and annotations`.

### Task 17: `App::run_command`, windowless App, save on exit

**Files:** Create `source/src/app/mcp_cmds.rs`; modify `source/src/app.rs` (`App::build`, `App::headless`, `Settings.mcp_enabled`), `source/src/app/actions.rs` (`forget_items`), `source/src/app/view.rs` (`on_exit`), `source/src/main.rs`.

**Interfaces:**
- Consumes: `mcp::command::Command`.
- Produces: `App::run_command(&mut self, cmd: Command) -> Result<serde_json::Value, String>`; `App::build(ctx: egui::Context, screenshot: Option<PathBuf>, demo: bool, start_url: Option<String>, theme: Option<ThemeMode>) -> App` (what `App::new` now calls after `setup`); `App::headless() -> App` (own `egui::Context::default()`, loads `downloads.json`, no browser listener or clipboard thread, no style setup); `App::forget_items(&mut self, ids: &[u64])` (removes list entries, stops them, leaves every file including `.part` on disk); `eframe::App::on_exit` calls `self.save()`; `Settings.mcp_enabled: bool` (`#[serde(default = "yes")]`, default true).
- `run_command` rules: `mcp_enabled == false` → `Err("MCP is turned off in Swift Download Manager (Options > AI assistants)")`; `AddDownload` allows only `http`/`https` (`url::Url`), runs `engine::sanitize` on `name`, picks the folder with `folder_for(category)`, rejects unknown category/queue names with a message listing the valid ones, and starts immediately unless `start == Some(false)`; `Remove` calls `forget_items`; results are compact JSON (id, name, url, status, progress 0..1, size, speed, queue, category).

- [ ] **Step 1: Write failing tests** (in `mcp_cmds.rs`, using `App::headless_for_test(dir)` that points the config path at a temp dir via an env override `SDM_CONFIG_DIR`, added to `config_file()`): `add_download_rejects_non_http_links` (`file:///etc/passwd`, `javascript:alert(1)`, `ftp://x/y` → Err); `add_download_cannot_escape_the_download_folder` (name `../../x.bin` → item name has no separators and `item.folder` equals the category folder); `remove_never_touches_files` (create a completed item pointing at a real temp file plus a `.part` file; after `Remove`, both files still exist); `disabled_mcp_refuses_everything`; `list_downloads_filters_by_status_and_query`.
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement** the signatures above; `ListDownloads` default `limit` 100, max 500.
- [ ] **Step 4: Run** `cargo test --locked` → pass; run the app once (`--demo`) to confirm the window still starts.
- [ ] **Step 5: Commit** `feat(mcp): command layer shared by the window and a windowless engine, plus save on exit`.

### Task 18: Control listener, token, and stdio server

**Files:** Create `source/src/mcp/control.rs`, `source/src/mcp/server.rs`; modify `source/src/main.rs` (`--mcp`), `source/Cargo.toml` (`getrandom = "0.3"`), `source/Cargo.lock`.

**Interfaces:**
- Produces in `control.rs`: `pub fn token(dir: &Path) -> io::Result<String>` (creates `mcp-token`, 32 random bytes hex, 0600 on unix, returns existing otherwise); `pub struct ControlInfo { pub port: u16, pub pid: u32, pub mode: String }` stored as `control.json`; `pub fn start(dir: &Path, tx: Sender<(Command, Sender<Result<Value, String>>)>, ctx: egui::Context) -> io::Result<ControlInfo>` (ephemeral loopback port; routes `POST /rpc`, `GET /info`; checks `Authorization: Bearer`, rejects `Origin` header, non-loopback `Host`, body over 64 KiB; 401/403/413 respectively; no CORS headers); `pub fn call(dir: &Path, cmd: &Command) -> Result<Value, String>` (client used by the proxy; error text distinguishes "no app running" from tool errors).
- Produces in `server.rs`: `pub fn run_stdio() -> i32` (reads stdin lines, forwards via `protocol::handle_line`; runner = `control::call` when a window answers `/info`, otherwise an in-process headless `App` driven on the main thread by a 200 ms `tick()` loop; writes responses to stdout only; exit 0 on stdin EOF after `save()`); in the window, `App` drains the control channel inside `tick()` and replies with `run_command`.
- Consumes: Task 16 `handle_line`, Task 17 `App::headless`/`run_command`.

- [ ] **Step 1: Write failing tests** in `control.rs` against a real listener on an ephemeral port with a temp dir: `rejects_missing_or_wrong_token` (401), `rejects_browser_origin` (403 even with the right token), `rejects_foreign_host_header` (403 for `Host: evil.example`), `rejects_oversized_body` (413), `accepts_valid_call` (`GetStatus` → Ok), `token_is_stable_and_private` (second call returns the same value; on unix mode is `0o600`). In `server.rs`: `stdio_session_round_trip` (feed `initialize`, `notifications/initialized`, `tools/list`, a malformed line, `tools/call get_status` into `run_session(reader, writer, runner)`; assert exactly four response lines, valid JSON each, ids matching) and `stdout_only_has_json` (every written line parses as JSON).
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement**; the window starts `control::start` in `App::new` when `settings.mcp_enabled` and not demo; headless starts it too (for hand-over, Task 19).
- [ ] **Step 4: Run** `cargo test --locked` → pass; manual check:

```bash
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}' '{"jsonrpc":"2.0","method":"notifications/initialized"}' '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | cargo run -q -- --mcp
```

Expected: two JSON lines, the second listing 9 tools.
- [ ] **Step 5: Commit** in two commits: `feat(mcp): authenticated loopback control listener with a private token` then `feat(mcp): stdio server (--mcp) that uses the open window or a headless engine`.

### Task 19: Single-writer lock and live hand-over

**Files:** Create `source/src/mcp/lock.rs`; modify `source/src/mcp/control.rs` (`POST /handoff`), `source/src/mcp/server.rs`, `source/src/app.rs` (window start-up), `source/src/app/actions.rs` (`SavedItem.handoff_resume`).

**Interfaces:**
- Produces: `pub struct StateLock(File)`; `StateLock::try_acquire(dir: &Path) -> io::Result<Option<StateLock>>` (`File::try_lock`; `None` when held elsewhere; released on drop or process death); `pub fn request_handoff(dir: &Path, timeout: Duration) -> Result<(), String>` (reads `control.json`, calls `POST /handoff` with the token); `App::prepare_handoff(&mut self)` (marks running items `handoff_resume = true`, stops them, waits up to 3 s for engines to go idle, saves). `SavedItem.handoff_resume: bool` is `#[serde(default)]`; on load the window resumes items with the flag and clears it.
- Flow: window start-up → `try_acquire`; if `None` → `request_handoff(5 s)` then retry once; if still `None` → start with control disabled and `toast("MCP engine is busy; AI control is off for this session")`. Headless on `/handoff` → `prepare_handoff`, drop lock, delete `control.json`, switch the runner to `control::call`. Headless next call with no window answering → `try_acquire`, `App::headless()` reload.

- [ ] **Step 1: Write failing tests** in `lock.rs`: `second_acquire_is_refused_until_first_drops`; and an integration test in `mcp/mod.rs` using the local test server pattern from `engine_tests.rs`: `handoff_keeps_progress_and_resumes_byte_exact` (headless app downloads a slow 5 MB file, `prepare_handoff` after about half, then a second `App` loads the same temp config dir, resumes via the flag, and the finished file's SHA-256 equals the source's); `handoff_with_nothing_running_is_instant` (under 1 s).
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test --locked` → pass. Manual: start `--mcp` headless with a long download, launch the window; it takes over and the download continues.
- [ ] **Step 5: Commit** `feat(mcp): single-writer state lock and live hand-over between headless and window`.

### Task 20: In-app switch and status chip

**Files:** Modify `source/src/app/dialogs.rs` (Options window: "AI assistants" card with `toggle_row`), `source/src/app/view.rs` (status bar chip: green dot "AI: connected/ready", grey "AI: off"), `source/src/app/mcp_cmds.rs`.

- [ ] **Step 1: Write failing test** `old_settings_default_mcp_on`: `serde_json::from_str::<Settings>("{}")` has `mcp_enabled == true`; `turning_the_switch_off_stops_the_listener_answering` (after `App::set_mcp_enabled(false)`, a `/rpc` call returns the "turned off" error).
- [ ] **Step 2: Run** → FAIL; implement `App::set_mcp_enabled(&mut self, on: bool)` and the UI; run → PASS.
- [ ] **Step 3: Commit** `feat(mcp): Options switch and status-bar chip for AI assistants`.

### Task 21: Docs and pull request

**Files:** Modify `README.md`, `source/README.md`, `CHANGELOG.md`, `SECURITY.md`, `source/src/app/tools.rs` (the "Browser integration" window gets an "AI assistants" tab with the config snippet), `.github/workflows/ci.yml` (add a `--mcp` smoke test step on Linux).

- [ ] **Step 1: Docs:** README "Use it from an AI assistant" with the Claude Code (`claude mcp add swift-download-manager -- /path/to/swift_download_manager --mcp`) and Claude Desktop (`mcpServers` JSON) snippets, the tool list, safety rules, the Windows limit; SECURITY.md describes the token, loopback-only binding and the no-file-deletion rule.
- [ ] **Step 2: CI:** after the tests step, on ubuntu only, pipe the `initialize` + `tools/list` lines (Task 18) into `target/debug/swift_download_manager --mcp` and assert `grep -q '"tools"'`.
- [ ] **Step 3: Verify:** `cargo build --locked && cargo test --locked`, `cargo clippy` clean for new code if clippy is installed.
- [ ] **Step 4: Commit** `docs: MCP usage, security notes and a CI smoke test`.
- [ ] **Step 5: Push and open the PR** (`gh pr create`) with a body listing both parts, the Part B decisions that need review (tool list, no folder argument, secondary-label contrast floor of 3:1, Windows `--mcp` unverified), and the two screenshots.
