const api = globalThis.browser ?? globalThis.chrome;
const APP = "http://127.0.0.1:9614";
const $ = (id) => document.getElementById(id);
const NS = "http://www.w3.org/2000/svg";

// Still pictures for anyone who asked their system for less motion.
if (globalThis.matchMedia?.("(prefers-reduced-motion: reduce)").matches) {
  document.querySelectorAll("svg").forEach((s) => s.pauseAnimations?.());
}

// ---------------------------------------------------------------- small helpers

function size(n) {
  if (!n) return "";
  if (n >= 1073741824) return (n / 1073741824).toFixed(2) + " GB";
  if (n >= 1048576) return (n / 1048576).toFixed(1) + " MB";
  return Math.max(1, Math.round(n / 1024)) + " KB";
}

function speed(n) {
  return n > 0 ? size(n) + "/s" : "0 B/s";
}

/** An element with text (never HTML: what comes from the app or a page is only ever shown as text). */
function el(tag, attrs = {}, ...kids) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") e.className = v;
    else if (k === "text") e.textContent = v;
    else if (k.startsWith("on")) e.addEventListener(k.slice(2), v);
    else if (v !== false && v != null) e.setAttribute(k, v);
  }
  for (const c of kids.flat()) if (c != null) e.append(c);
  return e;
}

/** A small outline picture, from the paths of a 24 x 24 icon. */
function icon(paths, px = 18) {
  const svg = document.createElementNS(NS, "svg");
  for (const [k, v] of Object.entries({ viewBox: "0 0 24 24", width: px, height: px, fill: "none", stroke: "currentColor", "stroke-width": 2, "stroke-linecap": "round", "stroke-linejoin": "round", "aria-hidden": "true" })) svg.setAttribute(k, v);
  for (const d of paths) {
    const p = document.createElementNS(NS, "path");
    p.setAttribute("d", d);
    svg.append(p);
  }
  return svg;
}

const ICONS = {
  video: ["M6 4.5v15L19 12 6 4.5Z"],
  audio: ["M9 18V6l10-2v12", "M9 18a3 3 0 1 1-3-3 3 3 0 0 1 3 3Z", "M19 16a3 3 0 1 1-3-3 3 3 0 0 1 3 3Z"],
  file: ["M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z", "M14 3v5h5"],
  pause: ["M8 5v14", "M16 5v14"],
  play: ["M7 4.5v15l12-7.5z"],
};

/** The file name in a link, as a person would read it. */
function fileName(url) {
  let last = "";
  try {
    last = new URL(url).pathname.split("/").pop() || "";
    return decodeURIComponent(last) || "video";
  } catch {
    // A name with a stray % in it: show it as it is.
    return last || "video";
  }
}

// ---------------------------------------------------------------- tabs

const tabs = ["downloads", "page", "settings"];
function show(name) {
  for (const t of tabs) {
    const on = t === name;
    $("t-" + t).setAttribute("aria-selected", String(on));
    $("t-" + t).tabIndex = on ? 0 : -1;
    $("p-" + t).hidden = !on;
  }
  if (name === "page") void loadPage();
  try {
    void api.storage.local.set({ tab: name });
  } catch {
    // Not remembered: nothing lost.
  }
}
for (const t of tabs) {
  $("t-" + t).addEventListener("click", () => show(t));
  $("t-" + t).addEventListener("keydown", (e) => {
    const i = tabs.indexOf(t);
    const n = e.key === "ArrowRight" ? i + 1 : e.key === "ArrowLeft" ? i - 1 : null;
    if (n === null) return;
    e.preventDefault();
    const next = tabs[(n + tabs.length) % tabs.length];
    show(next);
    $("t-" + next).focus();
  });
}

// ---------------------------------------------------------------- the app

/** What the app says (and whether it answered). */
async function ask(path, init) {
  const stop = new AbortController();
  const timer = setTimeout(() => stop.abort(), 1500);
  try {
    const r = await fetch(APP + path, { ...init, signal: stop.signal });
    return r.ok ? r : null;
  } catch {
    return null;
  } finally {
    clearTimeout(timer);
  }
}

const control = (fields) => ask("/control", { method: "POST", body: new URLSearchParams(fields) });

function renderDownloads(s) {
  const list = $("downloads");
  const rows = s.downloads || [];
  $("summary").hidden = !rows.length && !s.active;
  $("sumSpeed").textContent = speed(s.speed || 0);
  $("sumText").textContent = `${s.active || 0} downloading · ${s.queued || 0} waiting · ${s.paused || 0} paused`;
  $("emptyDownloads").hidden = rows.length > 0;
  const n = (s.active || 0) + (s.queued || 0) + (s.paused || 0);
  $("countDownloads").hidden = !n;
  $("countDownloads").textContent = String(n);
  $("update").hidden = !s.update;
  $("update").textContent = s.update || "";
  const seen = new Set();
  for (const d of rows) {
    seen.add(String(d.id));
    let item = list.querySelector(`[data-id="${d.id}"]`);
    const pct = Math.round((d.progress || 0) * 100);
    const running = d.status === "downloading" || d.status === "connecting";
    if (!item) {
      const bar = el("i");
      item = el("div", { class: "item", "data-id": d.id },
        el("span", { class: "icon" }, icon(ICONS.file)),
        el("span", { class: "name" }, el("b"), el("small")),
        el("span", { class: "side" }),
        el("div", { class: "bartrack" }, bar));
      list.append(item);
    }
    item.className = "item " + d.status;
    const b = item.querySelector("b");
    b.textContent = d.name;
    b.title = d.name;
    item.querySelector("small").textContent = running ? `${pct}%${d.size ? " of " + size(d.size) : ""} · ${speed(d.speed)}` : d.status === "error" ? "Failed: open the app to see why" : `${pct}%${d.size ? " of " + size(d.size) : ""}`;
    item.querySelector(".bartrack i").style.width = pct + "%";
    const side = item.querySelector(".side");
    const label = running ? "Pause" : "Resume";
    side.replaceChildren(el("button", { type: "button", class: "round", title: label, "aria-label": label + " " + d.name, onclick: () => void control({ cmd: running ? "pause" : "resume", id: d.id }) }, icon(running ? ICONS.pause : ICONS.play)));
  }
  for (const item of list.querySelectorAll(".item")) if (!seen.has(item.dataset.id)) item.remove();
}

let alive = false;
async function refresh() {
  const r = await ask("/ping");
  alive = Boolean(r);
  $("status").className = "pill " + (alive ? "ok" : "bad");
  $("statusText").textContent = alive ? "Connected to the app" : "The app is not running";
  $("offline").hidden = alive;
  if (!alive) {
    $("summary").hidden = true;
    $("downloads").replaceChildren();
    $("emptyDownloads").hidden = false;
    return;
  }
  const s = await ask("/status");
  if (s) {
    try {
      renderDownloads(await s.json());
    } catch {
      // An answer that is not what was expected (an older app): the status line above still says it is connected.
    }
  }
}
// ask() never rejects, so nothing here can fail; `void` says the result is left alone on purpose.
void refresh();
// The app can be started or closed while this is open, and downloads move.
setInterval(() => void refresh(), 1000);

$("openApp").addEventListener("click", () => void control({ cmd: "open" }));
$("pauseAll").addEventListener("click", () => void control({ cmd: "pause-all" }));
$("resumeAll").addEventListener("click", () => void control({ cmd: "resume-all" }));

// ---------------------------------------------------------------- the page in front

/** Send something from the page to the app and show how it went on the button. */
async function sendButton(button, work) {
  const label = button.textContent;
  button.disabled = true;
  button.className = "btn sending";
  button.textContent = "Sending…";
  let ok = false;
  try {
    ok = Boolean(await work());
  } catch {
    ok = false;
  }
  button.className = "btn " + (ok ? "sent" : "failed");
  button.textContent = ok ? "Sent ✓" : "App not running";
  // A failed send can be tried again; a good one goes back to its label after a moment.
  button.disabled = ok;
  if (ok) setTimeout(() => { button.className = button.dataset.kind || "btn"; button.textContent = label; button.disabled = false; }, 2200);
}

function mediaRow(tab, m) {
  const isAudio = /^audio\//i.test(m.type || "") || /\.(mp3|m4a|aac|ogg|opus|wav|flac)(\?|$)/i.test(m.url);
  const name = fileName(m.url);
  const button = el("button", { type: "button", class: "btn", text: "Download" });
  button.addEventListener("click", () => void sendButton(button, async () => (await api.runtime.sendMessage({ type: "send", url: m.url, referer: tab.url, name }))?.ok));
  return el("div", { class: "item" },
    el("span", { class: "icon" }, icon(isAudio ? ICONS.audio : ICONS.video)),
    el("span", { class: "name" }, el("b", { text: name, title: m.url }), el("small", { text: [isAudio ? "Sound" : "Video", size(m.size)].filter(Boolean).join(" · ") })),
    button);
}

const KINDS = [
  ["all", "All", () => true],
  ["archive", "Archives", (e) => /^(zip|rar|7z|tar|gz|bz2|xz|tgz|zst|iso|img|dmg)$/.test(e)],
  ["program", "Programs", (e) => /^(exe|msi|deb|rpm|appimage|apk|pkg)$/.test(e)],
  ["video", "Video", (e) => /^(mp4|mkv|webm|mov|avi|wmv|flv|m4v)$/.test(e)],
  ["audio", "Audio", (e) => /^(mp3|m4a|aac|ogg|opus|wav|flac)$/.test(e)],
  ["doc", "Documents", (e) => /^(pdf|docx?|xlsx?|pptx?|epub|csv)$/.test(e)],
  ["image", "Images", (e) => /^(png|jpe?g|gif|webp|svg)$/.test(e)],
];

let links = [];
let present = KINDS;
let kind = "all";
const picked = new Set();
let pageTab = null;

function renderLinks() {
  const test = KINDS.find((k) => k[0] === kind)[2];
  const shown = links.filter((l) => test(l.ext));
  $("links").replaceChildren(...shown.map((l) => {
    const box = el("input", { type: "checkbox", "aria-label": "Choose " + l.name });
    box.checked = picked.has(l.url);
    box.addEventListener("change", () => { if (box.checked) picked.add(l.url); else picked.delete(l.url); updateBar(shown); });
    return el("label", { class: "item" }, box,
      el("span", { class: "name" }, el("b", { text: l.name, title: l.url }), el("small", { text: l.text && l.text !== l.name ? l.text : l.url.replace(/^https?:\/\//, "") })),
      el("span", { class: "ext", text: l.ext }));
  }));
  $("emptyLinks").hidden = shown.length > 0;
  $("linksText").replaceChildren(el("b", { text: links.length ? "Nothing of that kind" : "No files linked on this page" }));
  $("linkBar").hidden = !shown.length;
  updateBar(shown);
}

function updateBar(shown) {
  const n = shown.filter((l) => picked.has(l.url)).length;
  $("sendLinks").disabled = !n;
  $("sendLinks").textContent = n ? `Download ${n} file${n === 1 ? "" : "s"}` : "Download";
  $("pickAll").checked = shown.length > 0 && n === shown.length;
  $("pickAll").indeterminate = n > 0 && n < shown.length;
}

$("pickAll").addEventListener("change", () => {
  const test = KINDS.find((k) => k[0] === kind)[2];
  for (const l of links.filter((x) => test(x.ext))) if ($("pickAll").checked) picked.add(l.url); else picked.delete(l.url);
  renderLinks();
});
$("sendLinks").dataset.kind = "btn primary";
$("sendLinks").addEventListener("click", () => {
  const urls = links.filter((l) => picked.has(l.url)).map((l) => l.url);
  void sendButton($("sendLinks"), async () => {
    const res = await api.runtime.sendMessage({ type: "send-many", urls, referer: pageTab?.url });
    if (res?.ok) picked.clear();
    return res?.ok;
  }).then(() => renderLinks());
});

async function loadPage() {
  const [tab] = await api.tabs.query({ active: true, currentWindow: true });
  if (!tab) return;
  pageTab = tab;
  // Media the page played.
  const key = "media-" + tab.id;
  const store = api.storage.session ?? api.storage.local;
  const found = (await store.get({ [key]: [] }))[key];
  $("media").replaceChildren(...found.slice().reverse().map((m) => mediaRow(tab, m)));
  $("emptyMedia").hidden = found.length > 0;
  $("countPage").hidden = !found.length;
  $("countPage").textContent = String(found.length);
  // Files the page links to.
  const res = await api.runtime.sendMessage({ type: "links", tabId: tab.id, onlySelection: false });
  links = res?.links || [];
  present = KINDS.filter((k) => k[0] === "all" || links.some((l) => k[2](l.ext)));
  $("filters").hidden = links.length === 0 || present.length < 3;
  renderFilters();
  renderLinks();
}


// ---------------------------------------------------------------- settings

const DEFAULTS = { catchAll: true, minSizeMB: 0, excludeHosts: "", onlyTypes: "", tab: "downloads" };
let savedTimer = null;

async function loadSettings() {
  let s = DEFAULTS;
  try {
    s = await api.storage.local.get(DEFAULTS);
  } catch {
    // Defaults.
  }
  $("catchAll").checked = s.catchAll;
  $("minSize").value = s.minSizeMB;
  $("onlyTypes").value = s.onlyTypes;
  $("excludeHosts").value = s.excludeHosts;
  return s;
}

async function saveSettings() {
  const minSizeMB = Math.max(0, Math.min(100000, Number($("minSize").value) || 0));
  try {
    await api.storage.local.set({ catchAll: $("catchAll").checked, minSizeMB, onlyTypes: $("onlyTypes").value.trim(), excludeHosts: $("excludeHosts").value.trim() });
    $("saved").hidden = false;
    clearTimeout(savedTimer);
    savedTimer = setTimeout(() => { $("saved").hidden = true; }, 1500);
  } catch {
    // Not saved: show what is really in force.
    await loadSettings();
  }
}
for (const id of ["catchAll", "minSize", "onlyTypes", "excludeHosts"]) $(id).addEventListener("change", () => void saveSettings());

void loadSettings().then((s) => show(tabs.includes(s.tab) ? s.tab : "downloads"));

function renderFilters() {
  $("filters").replaceChildren(...present.map(([id, label]) => el("button", { type: "button", "aria-pressed": String(id === kind), text: label, onclick: () => { kind = id; renderFilters(); renderLinks(); } })));
}
