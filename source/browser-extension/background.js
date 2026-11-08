// Sends downloads, links and videos from the browser to Swift Download Manager,
// which listens only on this computer (127.0.0.1:9614) and only answers this add-on.
const APP = "http://127.0.0.1:9614";
const api = globalThis.browser ?? globalThis.chrome;

/** What the pop-up lets the person choose (see popup.js). */
const DEFAULTS = {
  // Send the downloads the browser starts to the app.
  catchAll: true,
  // Leave smaller files to the browser (in megabytes; 0 = catch everything).
  minSizeMB: 0,
  // Sites that are left to the browser, one per line (a site includes its subdomains).
  excludeHosts: "",
  // Only these file types are caught, separated by spaces (empty = all of them).
  onlyTypes: "",
};

const settings = () => api.storage.local.get(DEFAULTS);

// ------------------------------------------------------------------ talking to the app

// The site's cookies go along, so downloads that need you to be logged in work.
async function cookiesFor(url) {
  try {
    const list = await api.cookies.getAll({ url });
    return list.map((c) => `${c.name}=${c.value}`).join("; ");
  } catch (e) {
    return "";
  }
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

// Pauses between attempts (milliseconds) when the app does not answer at all. Closing the app's window to the tray
// hands the port from the window to the background process, and for a moment nobody has it: a download that is sent in
// that moment must wait for the next owner, not fall back to the browser.
const QUICK = [150, 300, 600, 1000]; // about 2 s: a click that someone is waiting on
const PATIENT = [150, 300, 600, 1000, 1500, 2000]; // about 5.5 s: a download the browser has already started

async function post(path, fields, pauses = QUICK) {
  for (let attempt = 0; ; attempt++) {
    try {
      const r = await fetch(APP + path, { method: "POST", body: new URLSearchParams(fields) });
      return r.ok;
    } catch (e) {
      // A refused connection: nothing was received, so sending again cannot add the link twice.
      if (attempt >= pauses.length) throw e;
      await sleep(pauses[attempt]);
    }
  }
}

async function sendToApp(url, referer, name, pauses) {
  return post("/add", { url, referer: referer || "", name: name || "", cookies: await cookiesFor(url), ua: navigator.userAgent }, pauses);
}

/** Several links found on one page: they share the page's cookies and referrer. */
async function sendMany(urls, pageUrl) {
  if (!urls.length) return false;
  return post("/add-many", { urls: urls.join("\n"), referer: pageUrl || "", cookies: await cookiesFor(pageUrl || urls[0]), ua: navigator.userAgent });
}

/** A short tick on the toolbar button, so it is clear the link went to the app. */
function confirm(tabId) {
  try {
    void api.action.setBadgeText({ tabId, text: "✓" });
    void api.action.setBadgeBackgroundColor({ tabId, color: "#16a34a" });
    setTimeout(() => void api.action.setBadgeText({ tabId, text: "" }), 2500);
  } catch (e) {
    // No tab to show it on: not important.
  }
}

function baseName(path) {
  return (path || "").split(/[\\/]/).pop();
}

// ------------------------------------------------------------------ the right-click menu

function makeMenus() {
  api.contextMenus.removeAll(() => {
    api.contextMenus.create({ id: "sdm-download", title: "Download with Swift Download Manager", contexts: ["link", "image", "video", "audio"] });
    api.contextMenus.create({ id: "sdm-selection", title: "Download the selected links with Swift Download Manager", contexts: ["selection"] });
    api.contextMenus.create({ id: "sdm-page", title: "Download all the files on this page…", contexts: ["page"] });
  });
}
api.runtime.onInstalled.addListener(makeMenus);
// Some browsers drop the menu when the add-on is updated or the worker restarts: make sure it is there.
if (api.runtime.onStartup) api.runtime.onStartup.addListener(makeMenus);

api.contextMenus.onClicked.addListener(async (info, tab) => {
  if (info.menuItemId === "sdm-download") {
    const url = info.linkUrl || info.srcUrl;
    if (url && (await sendToApp(url, info.pageUrl).catch(() => false)) && tab) confirm(tab.id);
  } else if (info.menuItemId === "sdm-selection" && tab) {
    const links = await collectLinks(tab.id, true);
    if (links.length && (await sendMany(links.map((l) => l.url), tab.url).catch(() => false))) confirm(tab.id);
  } else if (info.menuItemId === "sdm-page" && tab) {
    // The pop-up has the list to choose from.
    try {
      await api.action.openPopup();
    } catch (e) {
      // Some browsers only open it from a click on the button: take everything that looks like a file.
      const links = await collectLinks(tab.id, false);
      if (links.length && (await sendMany(links.map((l) => l.url), tab.url).catch(() => false))) confirm(tab.id);
    }
  }
});

// ------------------------------------------------------------------ catching downloads

// A link that was Alt+clicked is left to the browser (the page script in bypass.js tells us), for a short while.
const bypass = new Map();
function bypassed(url) {
  const now = Date.now();
  for (const [u, until] of bypass) if (until < now) bypass.delete(u);
  return bypass.has(url);
}

/** Whether a download the browser started should go to the app, given what the person chose. */
function wanted(item, s) {
  const url = item.finalUrl || item.url;
  let host = "";
  try {
    host = new URL(url).hostname.toLowerCase();
  } catch (e) {
    return false;
  }
  const blocked = s.excludeHosts
    .split(/\s+/)
    .map((h) => h.trim().toLowerCase().replace(/^\*?\./, ""))
    .filter(Boolean);
  if (blocked.some((h) => host === h || host.endsWith("." + h))) return false;
  const types = s.onlyTypes.toLowerCase().split(/[\s,;.]+/).filter(Boolean);
  if (types.length) {
    const ext = (baseName(item.filename) || new URL(url).pathname).split("?")[0].split(".").pop().toLowerCase();
    if (!types.includes(ext)) return false;
  }
  const size = item.fileSize > 0 ? item.fileSize : item.totalBytes > 0 ? item.totalBytes : 0;
  if (s.minSizeMB > 0 && size > 0 && size < s.minSizeMB * 1048576) return false;
  return true;
}

// Catch downloads the browser starts and hand them to the app instead.
// The browser keeps going with its own copy while the app is tried (for a few seconds, see PATIENT); if the app does not
// answer in that time, because it is not running, the browser simply keeps the download.
api.downloads.onCreated.addListener(async (item) => {
  const s = await settings();
  if (!s.catchAll) return;
  const url = item.finalUrl || item.url;
  if (!/^https?:\/\//i.test(url) || bypassed(item.url) || bypassed(url) || !wanted(item, s)) return;
  try {
    if (await sendToApp(url, item.referrer, baseName(item.filename), PATIENT)) {
      // A small file may have been finished by the browser while the app was starting: then it is left alone.
      const [now] = await api.downloads.search({ id: item.id });
      if (now && now.state === "complete") return;
      await api.downloads.cancel(item.id);
      await api.downloads.erase({ id: item.id });
      confirm(undefined);
    }
  } catch (e) {
    // App not reachable: leave the download to the browser.
  }
});

// ------------------------------------------------------------------ videos and sounds on a page

// Spot video and audio files that pages play, so the pop-up can offer them.
const MEDIA_EXT = /\.(mp4|webm|mkv|mov|m4v|flv|avi|mp3|m4a|aac|ogg|opus|wav|flac)(\?|$)/i;
api.webRequest.onHeadersReceived.addListener(
  (d) => {
    if (d.tabId < 0 || !/^https?:/i.test(d.url)) return;
    const h = Object.fromEntries((d.responseHeaders || []).map((x) => [x.name.toLowerCase(), x.value || ""]));
    const type = h["content-type"] || "";
    const size = parseInt((h["content-range"] || "").split("/")[1] || h["content-length"] || "0", 10);
    const isMedia = /^(video|audio)\//i.test(type) || MEDIA_EXT.test(new URL(d.url).pathname);
    // Skip tiny pieces and streaming playlists (those are split into many small parts).
    if (!isMedia || /mpegurl|dash\+xml/i.test(type) || (size && size < 300 * 1024)) return;
    void rememberMedia(d.tabId, { url: d.url.replace(/([?&])(range|bytestart|byteend)=[^&]*/gi, "$1").replace(/[?&]$/, ""), type, size });
  },
  { urls: ["<all_urls>"], types: ["media", "xmlhttprequest", "other", "object"] },
  ["responseHeaders"]
);

async function rememberMedia(tabId, m) {
  const key = "media-" + tabId;
  const store = api.storage.session ?? api.storage.local;
  const got = (await store.get({ [key]: [] }))[key];
  if (got.some((x) => x.url === m.url)) return;
  got.push(m);
  await store.set({ [key]: got.slice(-20) });
  void api.action.setBadgeText({ tabId, text: String(Math.min(got.length, 99)) });
  void api.action.setBadgeBackgroundColor({ tabId, color: "#e31c5f" });
}

// A new page in the tab: forget what the old one played.
api.tabs.onUpdated.addListener((tabId, change) => {
  if (change.status === "loading" && change.url) {
    const store = api.storage.session ?? api.storage.local;
    void store.remove("media-" + tabId);
    void api.action.setBadgeText({ tabId, text: "" });
  }
});

// ------------------------------------------------------------------ links on a page

/** Runs inside the page: the links to files (and media) it holds, or only those in the selection. */
function pageLinks(onlySelection) {
  const FILE = /\.(zip|rar|7z|tar|gz|bz2|xz|tgz|zst|iso|img|dmg|pkg|deb|rpm|appimage|exe|msi|apk|pdf|docx?|xlsx?|pptx?|epub|csv|mp3|m4a|aac|ogg|opus|wav|flac|mp4|mkv|webm|mov|avi|wmv|flv|m4v|png|jpe?g|gif|webp|svg|torrent)$/i;
  const sel = window.getSelection();
  const found = new Map();
  const add = (el, raw, label) => {
    let u;
    try {
      u = new URL(raw, location.href);
    } catch (e) {
      return;
    }
    if (!/^https?:$/.test(u.protocol)) return;
    const ext = (u.pathname.match(FILE) || [])[1];
    if (!ext && !el.hasAttribute?.("download")) return;
    if (!found.has(u.href)) found.set(u.href, { url: u.href, name: decodeURIComponent(u.pathname.split("/").pop() || u.hostname), text: (label || "").trim().slice(0, 80), ext: (ext || "").toLowerCase() });
  };
  for (const a of document.querySelectorAll("a[href]")) {
    if (onlySelection && !(sel && !sel.isCollapsed && sel.containsNode(a, true))) continue;
    add(a, a.getAttribute("href"), a.textContent);
  }
  if (!onlySelection) {
    for (const m of document.querySelectorAll("video[src], audio[src], source[src]")) add(m, m.getAttribute("src"), "");
  }
  return [...found.values()].slice(0, 500);
}

async function collectLinks(tabId, onlySelection) {
  try {
    const res = await api.scripting.executeScript({ target: { tabId, allFrames: false }, func: pageLinks, args: [!!onlySelection] });
    return (res && res[0] && res[0].result) || [];
  } catch (e) {
    // A page the browser does not let add-ons read (its own pages, the web store).
    return [];
  }
}

// ------------------------------------------------------------------ the pop-up and the page script ask

api.runtime.onMessage.addListener((msg, sender, reply) => {
  if (!msg) return;
  if (msg.type === "send") {
    sendToApp(msg.url, msg.referer, msg.name).then((ok) => reply({ ok }), () => reply({ ok: false }));
    return true;
  }
  if (msg.type === "send-many") {
    sendMany(msg.urls || [], msg.referer).then((ok) => reply({ ok }), () => reply({ ok: false }));
    return true;
  }
  if (msg.type === "links") {
    collectLinks(msg.tabId, !!msg.onlySelection).then((links) => reply({ links }), () => reply({ links: [] }));
    return true;
  }
  if (msg.type === "bypass" && typeof msg.url === "string") {
    // Alt+click: the browser keeps this one. (Only this add-on's own page script may say so.)
    if (sender.id === api.runtime.id) bypass.set(msg.url, Date.now() + 15000);
  }
});
