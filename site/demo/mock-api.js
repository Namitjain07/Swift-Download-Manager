/* A pretend server for the live preview of the web interface on the website.
 *
 * build.sh copies the page of the web interface (source/src/web/ui/index.html, as it is in each release)
 * into demo/ and adds this file before the page's own script. The page then talks to this code instead
 * of to a Swift Download Manager server: nothing is downloaded, nothing leaves the browser, and a reload
 * starts the demonstration again. Only same-origin /api/ and /files/ requests are answered.
 */
(function () {
  "use strict";
  var MB = 1048576, GB = 1073741824;
  var CATEGORIES = ["Compressed", "Documents", "Music", "Programs", "Video", "General"];
  var BY_EXT = { zip: "Compressed", gz: "Compressed", iso: "Compressed", "7z": "Compressed", tar: "Compressed", pdf: "Documents", docx: "Documents", txt: "Documents",
    mp3: "Music", flac: "Music", wav: "Music", exe: "Programs", msi: "Programs", deb: "Programs", dmg: "Programs", mp4: "Video", mkv: "Video", mov: "Video" };

  var state = {
    signedIn: true,
    nextId: 20,
    last: Date.now(),
    limitKbps: 0,
    net: { enabled: true, automatic: true, chosen: [], off: [] },
    adapters: [
      { name: "Wi-Fi", kind: "Wi-Fi", address: "192.168.1.24", weight: 0.46 },
      { name: "Ethernet", kind: "Ethernet", address: "10.0.0.7", weight: 0.36 },
      { name: "Pixel 8", kind: "USB tethering", address: "192.168.42.129", weight: 0.18 }
    ],
    tokens: [{ id: 1, label: "Laptop add-on", created: Math.floor(Date.now() / 1000) - 86400 * 9 }],
    nextToken: 2,
    users: [
      { name: "maya", admin: true, downloads: 9, downloaded_bytes: 11 * GB, tokens: 1 },
      { name: "omar", admin: false, downloads: 4, downloaded_bytes: 2.4 * GB, tokens: 0 },
      { name: "priya", admin: false, downloads: 6, downloaded_bytes: 5.1 * GB, tokens: 2 }
    ],
    items: [
      item(1, "ubuntu-24.04-desktop-amd64.iso", "Compressed", "downloading", 5.69 * GB, 0.41, 4.4 * MB),
      item(2, "project-report-final.pdf", "Documents", "complete", 2.34 * MB, 1, 0),
      item(3, "holiday-video-4k.mp4", "Video", "downloading", 1.2 * GB, 0.73, 2.05 * MB),
      item(4, "conference-keynote.mkv", "Video", "queued", 840 * MB, 0, 0),
      item(5, "lofi-study-mix.flac", "Music", "paused", 212 * MB, 0.35, 0),
      item(6, "rustup-init.exe", "Programs", "complete", 12.6 * MB, 1, 0),
      item(7, "dataset-2026.zip", "Compressed", "downloading", 3.1 * GB, 0.12, 3.2 * MB),
      item(8, "backup-archive.tar.gz", "Compressed", "error", 1.8 * GB, 0.58, 0, "The server closed the connection. Press resume to try again."),
      item(9, "design-assets.zip", "Compressed", "complete", 148 * MB, 1, 0)
    ]
  };

  function item(id, name, category, status, size, frac, speed, error) {
    return { id: id, name: name, category: category, status: status, size: size, done: size * frac, base: speed, speed: speed, error: error || "", queue: "Main" };
  }
  function view(i) {
    return { id: i.id, name: i.name, category: i.category, status: i.status, progress: i.size ? i.done / i.size : 0, downloaded_bytes: Math.round(i.done),
      size_bytes: Math.round(i.size), speed_bytes_per_sec: Math.round(i.status === "downloading" ? i.speed : 0), error: i.error, queue: i.queue };
  }

  // Moves the pretend downloads on by the time since the last look.
  function tick() {
    var now = Date.now(), dt = Math.min(3, (now - state.last) / 1000);
    state.last = now;
    var running = state.items.filter(function (i) { return i.status === "downloading"; });
    state.items.forEach(function (i) { if (i.status === "connecting" && (i.since = (i.since || 0) + dt) > 1) { i.status = "downloading"; i.since = 0; } });
    // A free slot (three at a time) goes to the first waiting download.
    for (var k = running.length; k < 3; k++) {
      var next = state.items.filter(function (i) { return i.status === "queued"; })[0];
      if (!next) break;
      next.status = "connecting"; next.since = 0;
    }
    var total = running.reduce(function (s, i) { return s + i.base; }, 0);
    var cap = state.limitKbps ? state.limitKbps * 1024 : Infinity;
    var scale = total > cap ? cap / total : 1;
    running.forEach(function (i) {
      i.speed = Math.max(0, i.base * scale * (1 + 0.14 * Math.sin(now / 900 + i.id * 2.1) + 0.05 * Math.sin(now / 230 + i.id)));
      i.done = Math.min(i.size, i.done + i.speed * dt);
      if (i.done >= i.size) { i.status = "complete"; i.speed = 0; i.base = 0; }
    });
  }
  function speedNow() { return state.items.reduce(function (s, i) { return s + (i.status === "downloading" ? i.speed : 0); }, 0); }
  function count(st) { return state.items.filter(function (i) { return st.indexOf(i.status) >= 0; }).length; }
  function status() {
    return { downloads: state.items.length, active: count(["downloading", "connecting"]), queued: count(["queued"]), paused: count(["paused"]), complete: count(["complete"]),
      failed: count(["error"]), downloaded_bytes: Math.round(state.items.reduce(function (s, i) { return s + i.done; }, 0)), speed_bytes_per_sec: Math.round(speedNow()),
      speed_limit_kbps: state.limitKbps };
  }
  function networks() {
    var on = state.adapters.filter(function (a) { return state.net.automatic ? state.net.off.indexOf(a.name) < 0 : state.net.chosen.indexOf(a.name) >= 0; });
    var sum = on.reduce(function (s, a) { return s + a.weight; }, 0) || 1, speed = speedNow(), busy = speed > 0;
    return { enabled: state.net.enabled, automatic: state.net.automatic, chosen: state.net.chosen, off: state.net.off,
      available: state.adapters.map(function (a) { return { name: a.name, kind: a.kind, address: a.address }; }),
      live: on.map(function (a) { return { name: a.name, state: state.net.enabled && busy ? "working" : "resting", speed_bytes_per_sec: Math.round(state.net.enabled ? speed * a.weight / sum : 0), connections: busy ? Math.max(1, Math.round(10 * a.weight / sum)) : 0 }; }) };
  }
  function nameOf(url) {
    var u; try { u = new URL(url); } catch (e) { return null; }
    if (u.protocol !== "http:" && u.protocol !== "https:") return null;
    var last = decodeURIComponent((u.pathname.split("/").filter(Boolean).pop() || u.hostname)).replace(/[^\w.\- ()]/g, "_");
    return last.length > 70 ? last.slice(0, 70) : last;
  }
  function add(url, name, category) {
    var n = name || nameOf(url);
    if (!n) return null;
    var ext = (n.split(".").pop() || "").toLowerCase();
    var size = (40 + (state.nextId * 97) % 860) * MB;
    var it = item(state.nextId++, n, category || BY_EXT[ext] || "General", "queued", size, 0, (1.6 + (state.nextId % 4) * 0.7) * MB);
    state.items.push(it);
    return it;
  }
  function byId(id) { return state.items.filter(function (i) { return i.id === Number(id); })[0]; }

  function route(method, path, body) {
    var q = path.indexOf("?"), url = q < 0 ? path : path.slice(0, q), m;
    if (url === "/api/login" && method === "POST") { state.signedIn = true; return [200, { user: (body && body.user) || "maya", admin: true }]; }
    if (url === "/api/logout" && method === "POST") { state.signedIn = false; return [200, {}]; }
    if (!state.signedIn) return [401, { error: "Please sign in", shared: true }];
    if (url === "/api/me") return [200, { user: "maya", admin: true, shared: true, tokens: true }];
    if (url === "/api/queues") return [200, { queues: [{ name: "Main" }, { name: "Night" }], categories: CATEGORIES }];
    if (url === "/api/status") { tick(); return [200, status()]; }
    if (url === "/api/downloads" && method === "GET") { tick(); return [200, { downloads: state.items.map(view) }]; }
    if (url === "/api/downloads" && method === "POST") {
      var it = body && body.url ? add(String(body.url).trim(), body.name, body.category) : null;
      return it ? [201, view(it)] : [400, { error: "That does not look like a web address (http or https)" }];
    }
    if (url === "/api/downloads/batch" && method === "POST") {
      var results = [], added = 0, failed = 0;
      String((body && body.text) || "").split(/\r?\n/).map(function (l) { return l.trim(); }).filter(function (l) { return l && l[0] !== "#"; }).slice(0, 200).forEach(function (l) {
        var made = add(l, null, body.category);
        if (made) { added++; results.push({ ok: true, id: made.id }); } else { failed++; results.push({ ok: false, error: "Not a web address: " + l.slice(0, 40) }); }
      });
      return [200, { added: added, failed: failed, results: results }];
    }
    if (url === "/api/downloads/pause-all") { var p = 0; state.items.forEach(function (i) { if (["downloading", "connecting", "queued"].indexOf(i.status) >= 0) { i.status = "paused"; p++; } }); return [200, { paused: p }]; }
    if (url === "/api/downloads/resume-all") { var r = 0; state.items.forEach(function (i) { if (["paused", "error"].indexOf(i.status) >= 0) { i.status = "queued"; i.error = ""; r++; } }); return [200, { resumed: r }]; }
    if (url === "/api/downloads/clear-completed") { var before = state.items.length; state.items = state.items.filter(function (i) { return i.status !== "complete"; }); return [200, { removed: before - state.items.length }]; }
    if ((m = /^\/api\/downloads\/(\d+)\/(pause|resume)$/.exec(url))) {
      var d = byId(m[1]); if (!d) return [404, { error: "No such download" }];
      if (m[2] === "pause") d.status = "paused"; else { d.status = "connecting"; d.since = 0; d.error = ""; if (!d.base) d.base = 2.4 * MB; }
      return [200, view(d)];
    }
    if ((m = /^\/api\/downloads\/(\d+)$/.exec(url)) && method === "DELETE") {
      if (!byId(m[1])) return [404, { error: "No such download" }];
      state.items = state.items.filter(function (i) { return i.id !== Number(m[1]); }); return [200, {}];
    }
    if (url === "/api/networks") {
      if (method === "POST" && body) ["enabled", "automatic", "chosen", "off"].forEach(function (k) { if (body[k] !== undefined) state.net[k] = body[k]; });
      return [200, networks()];
    }
    if (url === "/api/speed-limit" && method === "POST") { state.limitKbps = Math.max(0, Math.min(1000000, Number(body && body.kbps) || 0)); return [200, { speed_limit_kbps: state.limitKbps }]; }
    if (url === "/api/tokens" && method === "GET") return [200, { tokens: state.tokens }];
    if (url === "/api/tokens" && method === "POST") {
      var label = String((body && body.label) || "").trim() || "Untitled token";
      state.tokens.push({ id: state.nextToken++, label: label.slice(0, 40), created: Math.floor(Date.now() / 1000) });
      return [201, { token: "sdm_demo_" + Math.random().toString(36).slice(2, 10) + "_not_a_real_key" }];
    }
    if ((m = /^\/api\/tokens\/(\d+)$/.exec(url)) && method === "DELETE") { state.tokens = state.tokens.filter(function (t) { return t.id !== Number(m[1]); }); return [200, {}]; }
    if (url === "/api/users" && method === "GET") return [200, { users: state.users }];
    if (url === "/api/users" && method === "POST") {
      var nm = String((body && body.name) || "").trim();
      if (!nm || String((body && body.password) || "").length < 8) return [400, { error: "Give a name and a password of 8 or more characters" }];
      var u = state.users.filter(function (x) { return x.name === nm; })[0];
      if (u) u.admin = !!body.admin; else state.users.push({ name: nm, admin: !!body.admin, downloads: 0, downloaded_bytes: 0, tokens: 0 });
      return [200, {}];
    }
    if ((m = /^\/api\/users\/(.+)$/.exec(url)) && method === "DELETE") { state.users = state.users.filter(function (x) { return x.name !== decodeURIComponent(m[1]); }); return [200, {}]; }
    return [404, { error: "This preview does not have that" }];
  }

  var realFetch = window.fetch ? window.fetch.bind(window) : null;
  window.fetch = function (input, init) {
    var path = typeof input === "string" ? input : (input && input.url) || "";
    var u; try { u = new URL(path, location.href); } catch (e) { u = null; }
    var mine = u && u.origin === location.origin && (/\/api\//.test(u.pathname) || /\/files\//.test(u.pathname));
    if (!mine) return realFetch ? realFetch(input, init) : Promise.reject(new TypeError("offline"));
    var p = u.pathname.replace(/^.*?(\/(api|files)\/)/, "$1") + u.search;
    if (/^\/files\//.test(p)) return Promise.resolve(new Response("This is a preview: there is no file to download.", { status: 200 }));
    var body; try { body = init && init.body ? JSON.parse(init.body) : undefined; } catch (e) { body = undefined; }
    var out = route(((init && init.method) || "GET").toUpperCase(), p, body);
    return new Promise(function (resolve) {
      setTimeout(function () { resolve(new Response(JSON.stringify(out[1]), { status: out[0], headers: { "Content-Type": "application/json" } })); }, 60 + Math.random() * 90);
    });
  };

  // A small label so nobody takes the pretend data for a real server.
  function ribbon() {
    var css = document.createElement("style");
    css.textContent = ".sdm-ribbon{position:fixed;left:12px;bottom:12px;z-index:50;padding:6px 12px;border-radius:999px;background:rgba(20,20,28,.86);color:#fff;font:600 12px/1.2 system-ui,sans-serif;box-shadow:0 4px 16px rgba(0,0,0,.3);pointer-events:none}";
    document.head.appendChild(css);
    var d = document.createElement("div");
    d.className = "sdm-ribbon";
    d.textContent = "Preview" + (window.SDM_PREVIEW ? " of " + window.SDM_PREVIEW : "") + " · pretend data";
    document.body.appendChild(d);
  }
  if (document.body) ribbon(); else document.addEventListener("DOMContentLoaded", ribbon);

  // Downloads in a preview open nothing: say so instead of showing a broken link.
  document.addEventListener("click", function (e) {
    var a = e.target.closest && e.target.closest("a[href^='/files/'],a[href*='/files/']");
    if (a) { e.preventDefault(); var t = document.createElement("div"); t.className = "toast"; t.setAttribute("role", "status"); t.textContent = "This is a preview, so there is no file to download."; document.body.appendChild(t); setTimeout(function () { t.remove(); }, 3500); }
  }, true);
})();
