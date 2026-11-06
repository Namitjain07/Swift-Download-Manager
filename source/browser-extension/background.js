// Sends downloads, right-clicked links and videos to Swift Download Manager,
// which listens only on this computer (127.0.0.1:9614).
const APP = "http://127.0.0.1:9614";
const api = globalThis.browser ?? globalThis.chrome;

// The site's cookies go along, so downloads that need you to be logged in work.
async function cookiesFor(url) {
  try {
    const list = await api.cookies.getAll({ url });
    return list.map((c) => `${c.name}=${c.value}`).join("; ");
  } catch (e) {
    return "";
  }
}

async function sendToApp(url, referer, name) {
  const body = new URLSearchParams({
    url,
    referer: referer || "",
    name: name || "",
    cookies: await cookiesFor(url),
    ua: navigator.userAgent,
  });
  const r = await fetch(APP + "/add", { method: "POST", body });
  return r.ok;
}

function baseName(path) {
  return (path || "").split(/[\\/]/).pop();
}

api.runtime.onInstalled.addListener(() => {
  api.contextMenus.create({
    id: "sdm-download",
    title: "Download with Swift Download Manager",
    contexts: ["link", "image", "video", "audio"],
  });
});

api.contextMenus.onClicked.addListener((info) => {
  const url = info.linkUrl || info.srcUrl;
  if (url) sendToApp(url, info.pageUrl).catch(() => {});
});

// Catch downloads the browser starts and hand them to the app instead.
// If the app is not running, the browser simply keeps the download.
api.downloads.onCreated.addListener(async (item) => {
  const { catchAll } = await api.storage.local.get({ catchAll: true });
  if (!catchAll) return;
  const url = item.finalUrl || item.url;
  if (!/^https?:\/\//i.test(url)) return;
  try {
    if (await sendToApp(url, item.referrer, baseName(item.filename))) {
      await api.downloads.cancel(item.id);
      await api.downloads.erase({ id: item.id });
    }
  } catch (e) {
    // App not reachable: leave the download to the browser.
  }
});

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
    rememberMedia(d.tabId, { url: d.url.replace(/([?&])(range|bytestart|byteend)=[^&]*/gi, "$1").replace(/[?&]$/, ""), type, size });
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
  console.info("Swift Download Manager: found media", m.url, m.type, m.size);
  api.action.setBadgeText({ tabId, text: String(Math.min(got.length, 99)) });
  api.action.setBadgeBackgroundColor({ tabId, color: "#e31c5f" });
}

// A new page in the tab: forget what the old one played.
api.tabs.onUpdated.addListener((tabId, change) => {
  if (change.status === "loading" && change.url) {
    const store = api.storage.session ?? api.storage.local;
    store.remove("media-" + tabId);
    api.action.setBadgeText({ tabId, text: "" });
  }
});

// The pop-up asks us to send a file.
api.runtime.onMessage.addListener((msg, _sender, reply) => {
  if (msg && msg.type === "send") {
    sendToApp(msg.url, msg.referer, msg.name).then((ok) => reply({ ok }), () => reply({ ok: false }));
    return true;
  }
});
