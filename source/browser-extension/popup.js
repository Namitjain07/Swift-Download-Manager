const api = globalThis.browser ?? globalThis.chrome;
const status = document.getElementById("status");
const box = document.getElementById("catchAll");
const mediaBox = document.getElementById("media");

fetch("http://127.0.0.1:9614/ping")
  .then((r) => r.text())
  .then(() => { status.textContent = "The app is running."; status.className = "ok"; })
  .catch(() => { status.textContent = "The app is not running. Downloads stay in the browser."; status.className = "bad"; });

api.storage.local.get({ catchAll: true }).then(({ catchAll }) => { box.checked = catchAll; });
box.addEventListener("change", () => api.storage.local.set({ catchAll: box.checked }));

function size(n) {
  if (!n) return "";
  return n > 1048576 ? (n / 1048576).toFixed(1) + " MB" : Math.round(n / 1024) + " KB";
}

async function showMedia() {
  const [tab] = await api.tabs.query({ active: true, currentWindow: true });
  if (!tab) return;
  const key = "media-" + tab.id;
  const store = api.storage.session ?? api.storage.local;
  const list = (await store.get({ [key]: [] }))[key];
  if (!list.length) return;
  mediaBox.textContent = "";
  mediaBox.className = "";
  for (const m of list) {
    const row = document.createElement("div");
    row.className = "media";
    const name = decodeURIComponent(new URL(m.url).pathname.split("/").pop() || "video");
    const label = document.createElement("span");
    label.title = m.url;
    label.textContent = name;
    const small = document.createElement("small");
    small.textContent = size(m.size);
    const btn = document.createElement("button");
    btn.textContent = "Download";
    btn.addEventListener("click", async () => {
      btn.disabled = true;
      const res = await api.runtime.sendMessage({ type: "send", url: m.url, referer: tab.url, name });
      btn.textContent = res && res.ok ? "Sent ✓" : "App not running";
    });
    row.append(label, small, btn);
    mediaBox.append(row);
  }
}
showMedia();
