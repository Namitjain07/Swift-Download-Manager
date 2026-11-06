const api = globalThis.browser ?? globalThis.chrome;
const APP = "http://127.0.0.1:9614";
const $ = (id) => document.getElementById(id);

const statusPill = $("status");
const statusText = $("statusText");
const offline = $("offline");
const catchAll = $("catchAll");
const mediaBox = $("media");

// Still pictures for anyone who asked their system for less motion.
if (globalThis.matchMedia?.("(prefers-reduced-motion: reduce)").matches) {
  document.querySelectorAll("svg").forEach((s) => s.pauseAnimations?.());
}

// ---------------------------------------------------------------- the app

/** True when the app answers on this computer. */
async function appIsRunning() {
  const stop = new AbortController();
  const timer = setTimeout(() => stop.abort(), 1500);
  try {
    const r = await fetch(APP + "/ping", { signal: stop.signal });
    return r.ok;
  } catch {
    return false;
  } finally {
    clearTimeout(timer);
  }
}

async function refreshStatus() {
  const running = await appIsRunning();
  statusPill.className = "pill " + (running ? "ok" : "bad");
  statusText.textContent = running ? "Connected to the app" : "The app is not running";
  offline.hidden = running;
}
refreshStatus();
// The app can be started or closed while this is open.
setInterval(refreshStatus, 3000);

// ---------------------------------------------------------------- the switch

api.storage.local.get({ catchAll: true }).then((v) => {
  catchAll.checked = v.catchAll;
});
catchAll.addEventListener("change", () => api.storage.local.set({ catchAll: catchAll.checked }));

// ---------------------------------------------------------------- videos and sounds

function size(n) {
  if (!n) return "";
  if (n >= 1073741824) return (n / 1073741824).toFixed(2) + " GB";
  if (n >= 1048576) return (n / 1048576).toFixed(1) + " MB";
  return Math.max(1, Math.round(n / 1024)) + " KB";
}

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

const NS = "http://www.w3.org/2000/svg";

/** A small outline picture: a play triangle for video, two notes for sound. */
function kindIcon(isAudio) {
  const svg = document.createElementNS(NS, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", "20");
  svg.setAttribute("height", "20");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "2");
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("stroke-linejoin", "round");
  svg.setAttribute("aria-hidden", "true");
  const parts = isAudio
    ? ["M9 18V6l10-2v12", "M9 18a3 3 0 1 1-3-3 3 3 0 0 1 3 3Z", "M19 16a3 3 0 1 1-3-3 3 3 0 0 1 3 3Z"]
    : ["M6 4.5v15L19 12 6 4.5Z"];
  for (const d of parts) {
    const p = document.createElementNS(NS, "path");
    p.setAttribute("d", d);
    svg.append(p);
  }
  return svg;
}

function row(tab, m) {
  const isAudio = /^audio\//i.test(m.type || "") || /\.(mp3|m4a|aac|ogg|opus|wav|flac)(\?|$)/i.test(m.url);
  const name = fileName(m.url);

  const item = document.createElement("div");
  item.className = "item";

  const icon = document.createElement("span");
  icon.className = "icon";
  icon.append(kindIcon(isAudio));

  const text = document.createElement("span");
  text.className = "name";
  const title = document.createElement("b");
  title.title = m.url;
  title.textContent = name;
  const detail = document.createElement("small");
  detail.textContent = [isAudio ? "Sound" : "Video", size(m.size)].filter(Boolean).join(" · ");
  text.append(title, detail);

  const button = document.createElement("button");
  button.type = "button";
  button.className = "btn";
  button.textContent = "Download";
  button.addEventListener("click", async () => {
    button.disabled = true;
    button.className = "btn sending";
    button.textContent = "Sending…";
    let ok = false;
    try {
      const res = await api.runtime.sendMessage({ type: "send", url: m.url, referer: tab.url, name });
      ok = Boolean(res && res.ok);
    } catch {
      ok = false;
    }
    button.className = "btn " + (ok ? "sent" : "failed");
    button.textContent = ok ? "Sent ✓" : "App not running";
    // A failed send can be tried again.
    button.disabled = ok;
  });

  item.append(icon, text, button);
  return item;
}

async function showMedia() {
  const [tab] = await api.tabs.query({ active: true, currentWindow: true });
  if (!tab) return;
  const key = "media-" + tab.id;
  const store = api.storage.session ?? api.storage.local;
  const list = (await store.get({ [key]: [] }))[key];
  if (!list.length) return;
  // Newest first: what the page has just started playing is what you want.
  mediaBox.replaceChildren(...list.slice().reverse().map((m) => row(tab, m)));
}
showMedia();
