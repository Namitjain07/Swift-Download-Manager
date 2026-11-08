/* Swift Download Manager: the website's script.
 * No libraries, nothing loaded from other sites except the public GitHub API (releases and the star count),
 * and the page works without it: a copy of the release list is published with the page (data/releases.json).
 * Everything that comes from outside is put on the page as text, never as HTML.
 */
(function () {
  "use strict";

  var REPO = "Namitjain07/Swift-Download-Manager";
  var SVGNS = "http://www.w3.org/2000/svg";
  var reduce = window.matchMedia && matchMedia("(prefers-reduced-motion: reduce)").matches;

  function $(s, r) { return (r || document).querySelector(s); }
  function $$(s, r) { return Array.prototype.slice.call((r || document).querySelectorAll(s)); }
  function safe(fn) { try { fn(); } catch (e) { if (window.console) console.warn(e); } }
  function el(tag, attrs) {
    var n = document.createElement(tag), k;
    for (k in (attrs || {})) {
      if (k === "class") n.className = attrs[k];
      else if (k === "text") n.textContent = attrs[k];
      else if (k.slice(0, 2) === "on") n.addEventListener(k.slice(2), attrs[k]);
      else if (attrs[k] !== false && attrs[k] != null) n.setAttribute(k, attrs[k]);
    }
    (function add(list) {
      for (var i = 0; i < list.length; i++) {
        var c = list[i];
        if (c == null || c === false) continue;
        if (Array.isArray(c)) add(c);
        else n.appendChild(typeof c === "string" ? document.createTextNode(c) : c);
      }
    })(Array.prototype.slice.call(arguments, 2));
    return n;
  }
  function icon(name) {
    var s = document.createElementNS(SVGNS, "svg"), u = document.createElementNS(SVGNS, "use");
    s.setAttribute("class", "ico"); s.setAttribute("aria-hidden", "true");
    u.setAttribute("href", "#i-" + name); s.appendChild(u);
    return s;
  }
  function visibleWatcher(node, on, off, margin) {
    if (!("IntersectionObserver" in window)) { on(); return; }
    new IntersectionObserver(function (es) { es.forEach(function (e) { if (e.isIntersecting) on(); else if (off) off(); }); }, { rootMargin: margin || "0px" }).observe(node);
  }

  // ------------------------------------------------------------------ the site's own light / dark
  safe(function () {
    var root = document.documentElement;
    $("#theme-toggle").addEventListener("click", function () {
      var next = root.getAttribute("data-theme") === "dark" ? "light" : "dark";
      root.setAttribute("data-theme", next);
      try { localStorage.setItem("sdm-site-theme", next); } catch (e) {}
    });
    if (window.matchMedia) matchMedia("(prefers-color-scheme: dark)").addEventListener("change", function (e) {
      var stored = null; try { stored = localStorage.getItem("sdm-site-theme"); } catch (x) {}
      if (!stored) root.setAttribute("data-theme", e.matches ? "dark" : "light");
    });
  });

  // ------------------------------------------------------------------ navigation
  safe(function () {
    var nav = $("#nav"), burger = $("#burger");
    var onScroll = function () { nav.classList.toggle("scrolled", window.scrollY > 8); };
    onScroll(); window.addEventListener("scroll", onScroll, { passive: true });
    burger.addEventListener("click", function () { var o = nav.classList.toggle("open"); burger.setAttribute("aria-expanded", String(o)); });
    $$("nav a").forEach(function (a) { a.addEventListener("click", function () { nav.classList.remove("open"); burger.setAttribute("aria-expanded", "false"); }); });
    if (!("IntersectionObserver" in window)) return;
    var links = {};
    $$("nav a[href^='#']").forEach(function (a) { links[a.getAttribute("href").slice(1)] = a; });
    var io = new IntersectionObserver(function (es) {
      es.forEach(function (e) { if (e.isIntersecting && links[e.target.id]) { $$("nav a.on").forEach(function (x) { x.classList.remove("on"); }); links[e.target.id].classList.add("on"); } });
    }, { rootMargin: "-45% 0px -50% 0px" });
    Object.keys(links).forEach(function (id) { var s = document.getElementById(id); if (s) io.observe(s); });
  });

  // ------------------------------------------------------------------ reveal on scroll, counters, glow
  safe(function () {
    var items = $$(".reveal");
    if (!("IntersectionObserver" in window) || reduce) { items.forEach(function (n) { n.classList.add("in"); }); }
    else {
      var io = new IntersectionObserver(function (es) { es.forEach(function (e) { if (e.isIntersecting) { e.target.classList.add("in"); io.unobserve(e.target); } }); }, { threshold: .12, rootMargin: "0px 0px -6% 0px" });
      items.forEach(function (n) { io.observe(n); });
    }
    $$("[data-count]").forEach(function (n) {
      var to = Number(n.getAttribute("data-count")) || 0;
      if (reduce) return;
      n.textContent = "0";
      visibleWatcher(n, function () {
        if (n.dataset.done) return; n.dataset.done = "1";
        var t0 = performance.now(), dur = 1300; setTimeout(function () { n.textContent = String(to); }, dur + 400);
        (function step(t) { var p = Math.min(1, (t - t0) / dur), e = 1 - Math.pow(1 - p, 3); n.textContent = String(Math.round(to * e)); if (p < 1) requestAnimationFrame(step); })(t0);
      });
    });
    $$(".cell").forEach(function (c) {
      c.addEventListener("pointermove", function (e) { var r = c.getBoundingClientRect(); c.style.setProperty("--mx", (e.clientX - r.left) + "px"); c.style.setProperty("--my", (e.clientY - r.top) + "px"); });
    });
  });

  // ------------------------------------------------------------------ the strip of features
  safe(function () {
    var words = ["32 connections per file", "Resume after a restart", "Queues and schedules", "Browser add-on", "Wi-Fi + Ethernet + USB", "Web interface", "Docker server", "MCP for AI assistants", "15 themes", "WinUI 3 with Mica",
      "SHA-256 checks", "Link grabber", "Clipboard watching", "Proxy and speed limits", "System tray", "Windows, Linux, macOS", "x86 and ARM", "Open source"];
    var track = $("#marquee");
    words.concat(words).forEach(function (w) { track.appendChild(el("span", { text: w })); });
  });

  // ------------------------------------------------------------------ the "32 connections" picture
  safe(function () {
    var box = $("#lanes"), n = 8, i;
    for (i = 0; i < n; i++) {
      var t = (4.2 + ((i * 37) % 23) / 10).toFixed(1), w = ((i * 13) % 7 / 10).toFixed(1);
      box.appendChild(el("div", { class: "lane" }, el("span", { text: "part " + (i + 1) }), el("div", { class: "bar" }, el("i", { style: "--t:" + t + "s;--w:" + w + "s" })), el("span", { class: "speed", text: "" })));
    }
    var total = el("div", { class: "total" }, el("em", { text: "8 of up to 32 connections" }), el("b", { class: "tot", text: "0.0 MB/s" }));
    box.appendChild(total);
    var speeds = $$(".speed", box), tot = $(".tot", box), base = [1.3, .9, 1.1, .8, 1.4, .7, 1.0, 1.2];
    if (reduce) { speeds.forEach(function (s, k) { s.textContent = base[k].toFixed(1) + " MB/s"; }); tot.textContent = base.reduce(function (a, b) { return a + b; }, 0).toFixed(1) + " MB/s"; return; }
    var run = false;
    visibleWatcher(box, function () { run = true; }, function () { run = false; });
    setInterval(function () {
      if (!run) return;
      var sum = 0, now = Date.now() / 1000;
      speeds.forEach(function (s, k) { var v = base[k] * (1 + .22 * Math.sin(now * 1.7 + k * 1.9)); sum += v; s.textContent = v.toFixed(1) + " MB/s"; });
      tot.textContent = sum.toFixed(1) + " MB/s";
    }, 700);
  });

  // ------------------------------------------------------------------ the themes
  var THEMES = [
    { id: "coral", name: "Coral", l: "#ff385c", d: "#ff385c", blurb: "The default: white cards, one coral accent, capsule buttons." },
    { id: "winui", name: "WinUI 3", l: "#0067c0", d: "#4cc2ff", blurb: "Windows 11's own design tokens: Mica, the system accent, Segoe UI." },
    { id: "files", name: "Files", l: "#0067c0", d: "#4cc2ff", blurb: "Small corners and a blue accent, after the Files app." },
    { id: "carbon", name: "Carbon", l: "#0f62fe", d: "#4589ff", blurb: "Square corners and a dark, technical look." },
    { id: "blue", name: "Blue", l: "#2563eb", d: "#3b82f6", blurb: "Clear and calm." },
    { id: "green", name: "Green", l: "#16a34a", d: "#22c55e", blurb: "Fresh, with a green accent." },
    { id: "black", name: "Black", l: "#111111", d: "#ffffff", blurb: "Monochrome: OLED black with a white accent." },
    { id: "purple", name: "Purple", l: "#7c3aed", d: "#8b5cf6", blurb: "Deep violet." },
    { id: "pink", name: "Pink", l: "#ec4899", d: "#f472b6", blurb: "Warm and playful." },
    { id: "orange", name: "Orange", l: "#f97316", d: "#fb923c", blurb: "Energetic." },
    { id: "teal", name: "Teal", l: "#0d9488", d: "#2dd4bf", blurb: "Cool and quiet." },
    { id: "gold", name: "Gold", l: "#d97706", d: "#fbbf24", blurb: "Amber on cream, or on near-black." },
    { id: "nord", name: "Nord", l: "#5e81ac", d: "#88c0d0", blurb: "Polar Night and Frost blue." },
    { id: "dracula", name: "Dracula", l: "#6b46c1", d: "#bd93f9", blurb: "The dark classic, with a light sibling." },
    { id: "mocha", name: "Mocha", l: "#8839ef", d: "#cba6f7", blurb: "Catppuccin Latte and Mocha." }
  ];
  function themeById(id) { for (var i = 0; i < THEMES.length; i++) if (THEMES[i].id === id) return THEMES[i]; return THEMES[0]; }
  var IMG = "assets/img/themes/";

  // the picture in the hero changes theme by itself
  safe(function () {
    var order = [["coral", "light"], ["winui", "dark"], ["files", "light"], ["carbon", "dark"], ["pink", "light"], ["nord", "dark"], ["green", "light"], ["mocha", "dark"], ["winui", "light"], ["coral", "dark"]];
    var box = $("#hero-shots"), tag = $("#hero-tag"), nm = $("#hero-tag-name"), md = $("#hero-tag-mode"), made = { "coral-light": $("img", box) }, at = 0;
    function show(i) {
      var t = order[i], key = t[0] + "-" + t[1], th = themeById(t[0]);
      if (!made[key]) { made[key] = el("img", { src: IMG + key + ".webp", width: "1770", height: "1080", alt: "", "aria-hidden": "true", decoding: "async" }); box.appendChild(made[key]); }
      var go = function () { $$("img", box).forEach(function (x) { x.classList.toggle("on", x === made[key]); }); nm.textContent = th.name; md.textContent = t[1]; tag.style.setProperty("--dot", t[1] === "dark" ? th.d : th.l); };
      if (made[key].complete) go(); else made[key].addEventListener("load", go, { once: true });
    }
    tag.style.setProperty("--dot", "#ff385c");
    if (reduce) return;
    var running = true;
    visibleWatcher($("#stage"), function () { running = true; }, function () { running = false; });
    setInterval(function () { if (!running || document.hidden) return; at = (at + 1) % order.length; show(at); }, 3800);
    // warm up the next picture a little before it is needed
    setTimeout(function () { order.slice(1, 4).forEach(function (t) { var i = new Image(); i.src = IMG + t[0] + "-" + t[1] + ".webp"; }); }, 2500);

    // a gentle tilt that follows the pointer
    var stage = $("#stage"), win = $("#window");
    stage.addEventListener("pointermove", function (e) {
      if (e.pointerType === "touch") return;
      var r = stage.getBoundingClientRect(), x = (e.clientX - r.left) / r.width - .5, y = (e.clientY - r.top) / r.height - .5;
      win.style.setProperty("--ry", (x * 5).toFixed(2) + "deg"); win.style.setProperty("--rx", (3 - y * 5).toFixed(2) + "deg");
    });
    stage.addEventListener("pointerleave", function () { win.style.removeProperty("--ry"); win.style.removeProperty("--rx"); });

    // the speed chips breathe
    var sp = $("#live-speed"), tot = $("#net-total");
    setInterval(function () {
      if (!running || document.hidden) return;
      var v = (6.46 + Math.sin(Date.now() / 900) * .35 + Math.sin(Date.now() / 370) * .12).toFixed(2);
      sp.textContent = v + " MB/s"; if (tot) tot.textContent = v + " MB/s";
    }, 900);
  });

  // the gallery
  safe(function () {
    var swatches = $("#swatches"), shot = $("#gal-shot"), stage = $("#gal-stage"), nm = $("#gal-name"), bl = $("#gal-blurb");
    var cur = 0, mode = "light", made = {}, auto = true, timer = null;
    function show() {
      var t = THEMES[cur], key = t.id + "-" + mode, dot = mode === "dark" ? t.d : t.l;
      if (!made[key]) { made[key] = el("img", { src: IMG + key + ".webp", width: "1770", height: "1080", alt: t.name + " theme, " + mode + " mode: the main window", loading: "lazy", decoding: "async" }); shot.appendChild(made[key]); }
      var go = function () { $$("img", shot).forEach(function (x) { x.classList.toggle("on", x === made[key]); }); };
      if (made[key].complete && made[key].naturalWidth) go(); else made[key].addEventListener("load", go, { once: true });
      stage.style.setProperty("--dot", dot); nm.textContent = t.name; bl.textContent = t.blurb;
      $$("button", swatches).forEach(function (b, i) { b.setAttribute("aria-pressed", String(i === cur)); b.style.setProperty("--dot", mode === "dark" ? THEMES[i].d : THEMES[i].l); });
    }
    THEMES.forEach(function (t, i) {
      var b = el("button", { class: "sw", type: "button", "aria-pressed": "false", "aria-label": t.name, onclick: function () { auto = false; cur = i; show(); } },
        el("span", { class: "dots" }, el("i", { style: "background:" + t.l }), el("i", { style: "background:" + t.d })), el("span", { text: t.name }));
      swatches.appendChild(b);
    });
    $$(".seg button", stage).forEach(function (b) { b.addEventListener("click", function () { auto = false; mode = b.getAttribute("data-mode"); $$(".seg button", stage).forEach(function (x) { x.setAttribute("aria-pressed", String(x === b)); }); show(); }); });
    show();
    if (reduce) return;
    var vis = false;
    visibleWatcher($("#themes"), function () { vis = true; }, function () { vis = false; });
    timer = setInterval(function () { if (!auto || !vis || document.hidden) return; cur = (cur + 1) % THEMES.length; show(); }, 3200);
  });

  // ------------------------------------------------------------------ the pretend conversation
  safe(function () {
    var body = $("#term-body");
    var script = [
      ["user", "Grab the two release files and the checksum list from example.org, and tell me when they are verified."],
      ["ai", "I'll check the links before downloading anything."],
      ["call", "probe_link", "look", '{ "url": "https://example.org/releases/app-2.4.zip" }', "200 OK · 148 MB · resumable · app-2.4.zip"],
      ["call", "add_downloads", "standard", '{ "urls": [ …3 links ] }', "3 added to the Main queue"],
      ["ai", "Let me speed things up while they run."],
      ["call", "set_speed_limit", "full", '{ "kbps": 0 }', "refused (forbidden): this connection has the Standard level.\nAsk the user to allow Full access.", true],
      ["call", "get_status", "look", "{ }", "3 active · 7.8 MB/s over 3 networks"],
      ["call", "verify_download", "standard", '{ "id": 41, "sha256": "9f2c…b71e" }', "SHA-256 matches"],
      ["ai", "All three files are downloaded and the archive is verified. I could not lift the speed limit: that needs Full access, which you can allow under Help → AI assistants."]
    ];
    var timers = [], running = false;
    function node(s) {
      var kind = s[0];
      if (kind === "call") {
        var c = el("div", { class: "msg call" + (s[5] ? " denied" : "") }, el("span", { class: "lvl", text: s[2] }), el("div", { class: "fn" }, s[1] + "(", el("span", { text: s[3] }), ")"), el("div", { class: "out", text: (s[5] ? "✕ " : "✓ ") + s[4] }));
        return c;
      }
      return el("div", { class: "msg " + kind }, el("span", { class: "who", text: kind === "user" ? "You" : "Claude" }), el("div", { class: "t", text: s[1] }));
    }
    function play() {
      timers.forEach(clearTimeout); timers = []; body.textContent = "";
      var at = 400;
      script.forEach(function (s) {
        var n = node(s);
        timers.push(setTimeout(function () { body.appendChild(n); requestAnimationFrame(function () { n.classList.add("show"); }); body.scrollTop = body.scrollHeight; }, at));
        at += s[0] === "call" ? 1500 : 1900;
      });
      timers.push(setTimeout(function () { if (running) play(); }, at + 7000));
    }
    if (reduce) { script.forEach(function (s) { var n = node(s); n.classList.add("show"); body.appendChild(n); }); return; }
    visibleWatcher($(".term"), function () { if (!running) { running = true; play(); } }, function () { running = false; timers.forEach(clearTimeout); });
  });

  // ------------------------------------------------------------------ copy buttons
  safe(function () {
    $$("[data-copy]").forEach(function (box) {
      var text = box.innerText.split("\n").filter(function (l) { return !/^\s*(#|\/\/)/.test(l); }).join("\n").trim();
      var b = el("button", { class: "copy", type: "button", "aria-label": "Copy the command" }, icon("copy"), el("span", { text: "Copy" }));
      b.addEventListener("click", function () {
        var done = function () { b.lastChild.textContent = "Copied"; setTimeout(function () { b.lastChild.textContent = "Copy"; }, 1600); };
        if (navigator.clipboard && window.isSecureContext) navigator.clipboard.writeText(text).then(done, function () {});
        else { var ta = el("textarea", { style: "position:fixed;left:-9999px", "aria-hidden": "true" }); ta.value = text; document.body.appendChild(ta); ta.select(); try { document.execCommand("copy"); done(); } catch (e) {} ta.remove(); }
      });
      box.appendChild(b);
    });
  });

  // ------------------------------------------------------------------ releases
  function fmtSize(n) { return n >= 1073741824 ? (n / 1073741824).toFixed(1) + " GB" : n >= 1048576 ? (n / 1048576).toFixed(1) + " MB" : Math.max(1, Math.round(n / 1024)) + " KB"; }
  function fmtDate(s) { try { return new Date(s).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" }); } catch (e) { return s || ""; } }
  function normalize(r) {
    return { tag: r.tag_name, name: r.name || r.tag_name, published: r.published_at, prerelease: !!r.prerelease, notes: r.body || "", url: r.html_url,
      assets: (r.assets || []).map(function (a) { return { name: a.name, size: a.size, downloads: a.download_count, url: a.browser_download_url }; }) };
  }
  function version(tag) { return String(tag || "").replace(/^v/, ""); }

  // What each file is. Order matters: the first match wins.
  var KINDS = [
    { re: /-setup-x64\.exe$/i, os: "windows", arch: "x64", t: "Installer, 64-bit", s: "Windows 10 and 11 on Intel or AMD. Installs for you, adds a Start menu entry and an uninstaller.", ic: "windows", c: "#0067c0", rec: 1 },
    { re: /-setup-arm64\.exe$/i, os: "windows", arch: "arm64", t: "Installer, ARM", s: "Windows 11 on ARM (Snapdragon and others), running natively.", ic: "windows", c: "#0067c0", rec: 1 },
    { re: /-windows-x64\.zip$/i, os: "windows", arch: "x64", t: "Portable zip, 64-bit", s: "The program with the browser add-on, the certificate and a short read-me. No installing.", ic: "package", c: "#6366f1", rec: 3 },
    { re: /-windows-arm64\.zip$/i, os: "windows", arch: "arm64", t: "Portable zip, ARM", s: "The same, built for Windows on ARM.", ic: "package", c: "#6366f1", rec: 3 },
    { re: /^SwiftDownloadManager-arm64\.exe$/i, os: "windows", arch: "arm64", t: "Just the program, ARM", s: "A single file. Double-click it.", ic: "file", c: "#0ea5e9", rec: 4 },
    { re: /^SwiftDownloadManager\.exe$/i, os: "windows", arch: "x64", t: "Just the program, 64-bit", s: "A single file that needs no installer. Double-click it.", ic: "file", c: "#0ea5e9", rec: 2 },
    { re: /_amd64\.deb$/i, os: "linux", arch: "x64", t: "Debian, Ubuntu, Mint (.deb), 64-bit", s: "Ubuntu 22.04+, Debian 12+, Mint 21+, on Intel or AMD.", ic: "package", c: "#e95420", rec: 1, cmd: function (f) { return "sudo apt install ./" + f; } },
    { re: /_arm64\.deb$/i, os: "linux", arch: "arm64", t: "Debian, Ubuntu, Mint (.deb), ARM", s: "Raspberry Pi 4/5 with a 64-bit OS, ARM servers and laptops.", ic: "package", c: "#e95420", rec: 1, cmd: function (f) { return "sudo apt install ./" + f; } },
    { re: /-x86_64\.pkg\.tar\.zst$/i, os: "linux", arch: "x64", t: "Arch, Manjaro, EndeavourOS", s: "Signed pacman package for x86_64.", ic: "package", c: "#1793d1", rec: 2, cmd: function (f) { return "sudo pacman -U " + f; } },
    { re: /-aarch64\.pkg\.tar\.zst$/i, os: "linux", arch: "arm64", t: "Arch Linux ARM", s: "Signed pacman package for aarch64.", ic: "package", c: "#1793d1", rec: 2, cmd: function (f) { return "sudo pacman -U " + f; } },
    { re: /\.pkg\.tar\.zst\.sig$/i, os: "linux", t: "Package signature", s: "The GPG signature of the Arch package.", ic: "key", c: "#64748b", rec: 9 },
    { re: /-Helper-.*-chrome\.zip$/i, os: "browser", t: "Chrome, Edge, Brave, Opera", s: "Unzip into a folder you keep, then Load unpacked.", ic: "plug", c: "#16a34a", rec: 1 },
    { re: /-Helper-.*\.xpi$/i, os: "browser", t: "Firefox", s: "Load it as a temporary add-on, or have it signed to keep it.", ic: "plug", c: "#f97316", rec: 2 },
    { re: /-source\.zip$/i, os: "all", t: "Source code", s: "Everything needed to build it yourself.", ic: "terminal", c: "#64748b", rec: 5 },
    { re: /^SHA256SUMS\.asc$/i, os: "all", t: "Checksum signature", s: "The GPG signature of SHA256SUMS.", ic: "key", c: "#64748b", rec: 8 },
    { re: /^SHA256SUMS$/i, os: "all", t: "Checksums", s: "SHA-256 of every file in this release.", ic: "hash", c: "#64748b", rec: 7 }
  ];
  function kindOf(a) { for (var i = 0; i < KINDS.length; i++) if (KINDS[i].re.test(a.name)) return KINDS[i]; return { os: "all", t: a.name, s: "", ic: "file", c: "#64748b", rec: 20 }; }

  var state = { releases: [], latest: null, env: { os: "other", arch: "x64" }, previews: { main: null, tags: [] } };

  function detect() {
    var ua = navigator.userAgent || "", plat = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || "", os = "other", arch = "x64";
    if (/Android|iPhone|iPad|iPod/i.test(ua)) os = "mobile";
    else if (/Win/i.test(plat) || /Windows/i.test(ua)) os = "windows";
    else if (/Mac/i.test(plat) || /Mac OS X/i.test(ua)) os = "macos";
    else if (/Linux|X11|CrOS/i.test(ua + plat)) os = "linux";
    if (/aarch64|arm64|ARM64/i.test(ua)) arch = "arm64";
    state.env = { os: os, arch: arch };
    if (navigator.userAgentData && navigator.userAgentData.getHighEntropyValues) {
      navigator.userAgentData.getHighEntropyValues(["architecture"]).then(function (h) {
        if (h && /arm/i.test(h.architecture || "")) state.env.arch = "arm64";
        render();
      }, function () {});
    }
  }

  function pickFor(rel, os, arch) {
    var list = rel.assets.map(function (a) { return { a: a, k: kindOf(a) }; }).filter(function (x) { return x.k.os === os && x.k.rec < 9; });
    list.sort(function (x, y) { return (x.k.arch === arch ? 0 : 1) - (y.k.arch === arch ? 0 : 1) || x.k.rec - y.k.rec; });
    return list[0] || null;
  }

  function fileRow(x) {
    var a = x.a, k = x.k, acts = el("div", { class: "acts" });
    if (k.cmd) {
      var cmd = k.cmd(a.name);
      acts.appendChild(el("button", { class: "btn sm ghost", type: "button", title: cmd, onclick: function (e) {
        var b = e.currentTarget, done = function () { b.lastChild.textContent = "Copied"; setTimeout(function () { b.lastChild.textContent = "Install command"; }, 1500); };
        if (navigator.clipboard && window.isSecureContext) navigator.clipboard.writeText(cmd).then(done, function () {}); } }, icon("copy"), el("span", { text: "Install command" })));
    }
    acts.appendChild(el("a", { class: "btn sm primary", href: a.url, rel: "noopener" }, icon("download"), "Download"));
    var title = el("b", { text: k.t });
    if (k.rec === 1 && (k.os === "windows" || k.os === "linux")) title.appendChild(el("span", { class: "tagx", text: "Recommended" }));
    return el("div", { class: "file" }, el("div", { class: "fi", style: "--c:" + k.c }, icon(k.ic)),
      el("div", {}, title, el("div", { class: "sub" }, k.s ? k.s + " " : "", el("code", { text: a.name }), " · " + fmtSize(a.size))), acts);
  }

  function renderDownloads() {
    var rel = state.latest, env = state.env;
    var label = $("#dl-version");
    if (!rel) {
      $$("[data-files]").forEach(function (box) { box.replaceChildren(el("div", { class: "note" }, "The list of files could not be loaded. ", el("a", { href: "https://github.com/" + REPO + "/releases/latest", rel: "noopener", text: "Open the latest release on GitHub" }), ".")); });
      return;
    }
    label.textContent = "v" + version(rel.tag);
    var groups = { windows: [], linux: [], browser: [], all: [] };
    rel.assets.map(function (a) { return { a: a, k: kindOf(a) }; }).sort(function (x, y) { return x.k.rec - y.k.rec; }).forEach(function (x) {
      groups.all.push(x);
      if (groups[x.k.os] && x.k.os !== "all") groups[x.k.os].push(x);
    });
    // Put the file for this computer first.
    ["windows", "linux"].forEach(function (os) { groups[os].sort(function (x, y) { return (x.k.arch === env.arch ? 0 : 1) - (y.k.arch === env.arch ? 0 : 1) || x.k.rec - y.k.rec; }); });
    Object.keys(groups).forEach(function (g) {
      var box = $("[data-files='" + g + "']"); if (!box) return;
      if (!groups[g].length) { box.replaceChildren(el("div", { class: "note" }, "This version has no files for that. ", el("a", { href: "#versions", text: "Other versions" }), " may.")); return; }
      box.replaceChildren.apply(box, groups[g].map(fileRow));
    });

    // The big card at the top, and the button in the hero.
    var primary = $("#dl-primary"), title = $("#dl-title"), desc = $("#dl-desc"), det = $("#dl-detected"), hb = $("#hero-dl"), hl = $("#hero-dl-label"), hs = $("#hero-dl-sub");
    var names = { windows: "Windows", linux: "Linux", macos: "macOS" };
    var pick = (env.os === "windows" || env.os === "linux") ? pickFor(rel, env.os, env.arch) : null;
    if (pick) {
      var v = "v" + version(rel.tag);
      det.textContent = "Recommended for your computer";
      title.textContent = names[env.os] + (env.arch === "arm64" ? " on ARM" : "") + ", " + v;
      desc.textContent = pick.k.s;
      primary.replaceChildren(el("a", { class: "btn primary stack", href: pick.a.url, rel: "noopener" }, icon("download"), el("span", {}, "Download for " + names[env.os], el("small", { text: pick.a.name + " · " + fmtSize(pick.a.size) }))));
      hb.setAttribute("href", pick.a.url); hl.textContent = "Download for " + names[env.os]; hs.textContent = v + " · " + fmtSize(pick.a.size) + " · " + pick.k.t.split(",")[0].toLowerCase();
      activateTab(env.os, true);
    } else if (env.os === "macos") {
      det.textContent = "Recommended for your computer";
      title.textContent = "macOS: build it in a few minutes";
      desc.textContent = "A Mac app can only be built on a Mac, so there is no ready-made download. The steps are short.";
      primary.replaceChildren(el("a", { class: "btn primary stack", href: "#download", onclick: function () { activateTab("macos"); } }, icon("terminal"), el("span", {}, "Build it on your Mac", el("small", { text: "Rust and a few commands" }))));
      hb.setAttribute("href", "#download"); hl.textContent = "Get it for macOS"; hs.textContent = "Build from source";
      activateTab("macos", true);
    } else {
      det.textContent = env.os === "mobile" ? "A desktop app" : "Choose your platform";
      title.textContent = env.os === "mobile" ? "Open this page on your computer" : "Windows, Linux or macOS";
      desc.textContent = env.os === "mobile" ? "Swift Download Manager runs on Windows, Linux and macOS. The web interface works from a phone once a copy runs on a computer or a server." : "Pick a tab below.";
      primary.replaceChildren(el("a", { class: "btn primary", href: "#web" }, icon("play"), "Try the web interface"));
      hl.textContent = "Download"; hs.textContent = "v" + version(rel.tag) + " · free & open source";
    }
  }

  // ------------------------------------------------------------------ tabs of the download card
  function activateTab(name, quiet) {
    var tabs = $$("#dl-tabs [role=tab]"), found = false;
    tabs.forEach(function (t) { var on = t.getAttribute("data-tab") === name; if (on) found = true; });
    if (!found) return;
    tabs.forEach(function (t) {
      var on = t.getAttribute("data-tab") === name;
      t.setAttribute("aria-selected", String(on)); t.tabIndex = on ? 0 : -1;
      var p = document.getElementById(t.getAttribute("aria-controls")); if (p) p.hidden = !on;
    });
  }
  safe(function () {
    var tabs = $$("#dl-tabs [role=tab]");
    tabs.forEach(function (t, i) {
      t.addEventListener("click", function () { activateTab(t.getAttribute("data-tab")); });
      t.addEventListener("keydown", function (e) {
        var n = e.key === "ArrowRight" ? i + 1 : e.key === "ArrowLeft" ? i - 1 : e.key === "Home" ? 0 : e.key === "End" ? tabs.length - 1 : null;
        if (n == null) return; e.preventDefault(); n = (n + tabs.length) % tabs.length; tabs[n].focus(); activateTab(tabs[n].getAttribute("data-tab"));
      });
    });
  });

  // ------------------------------------------------------------------ the notes of a release, as text
  function inline(parent, text) {
    var re = /(`[^`]+`|\*\*[^*]+\*\*|\[[^\]]+\]\(https?:\/\/[^)\s]+\))/g, last = 0, m;
    while ((m = re.exec(text))) {
      if (m.index > last) parent.appendChild(document.createTextNode(text.slice(last, m.index)));
      var s = m[0];
      if (s[0] === "`") parent.appendChild(el("code", { text: s.slice(1, -1) }));
      else if (s[0] === "*") parent.appendChild(el("b", { text: s.slice(2, -2) }));
      else { var i = s.indexOf("]("); parent.appendChild(el("a", { href: s.slice(i + 2, -1), rel: "noopener noreferrer", target: "_blank", text: s.slice(1, i) })); }
      last = m.index + s.length;
    }
    if (last < text.length) parent.appendChild(document.createTextNode(text.slice(last)));
  }
  function notes(md) {
    var box = el("div", { class: "notes" }), ul = null, lines = String(md || "").replace(/\r/g, "").split("\n");
    lines.forEach(function (raw) {
      var line = raw.trim(), m;
      if (!line) { ul = null; return; }
      if ((m = /^#{1,6}\s+(.*)$/.exec(line))) { ul = null; var h = el("h4"); inline(h, m[1]); box.appendChild(h); return; }
      if ((m = /^[-*]\s+(.*)$/.exec(line))) { if (!ul) { ul = el("ul"); box.appendChild(ul); } var li = el("li"); inline(li, m[1]); ul.appendChild(li); return; }
      ul = null; var p = el("p"); inline(p, line); box.appendChild(p);
    });
    if (!box.firstChild) box.appendChild(el("p", { text: "No notes were written for this version." }));
    return box;
  }
  function firstLine(md) {
    var lines = String(md || "").split("\n"), i, m;
    for (i = 0; i < lines.length; i++) { if ((m = /^\s*[-*]\s+(.*)$/.exec(lines[i]))) return m[1].replace(/[`*]|\[([^\]]+)\]\([^)]*\)/g, "$1").slice(0, 160); }
    return "";
  }

  // ------------------------------------------------------------------ the list of versions
  var filter = "all";
  function renderVersions() {
    var box = $("#timeline"); if (!box) return;
    var list = state.releases.filter(function (r) { return filter === "all" || (filter === "pre" ? r.prerelease : !r.prerelease); });
    if (!list.length) { box.replaceChildren(el("div", { class: "note" }, state.releases.length ? "No versions of that kind." : "The list of versions could not be loaded. ", state.releases.length ? null : el("a", { href: "https://github.com/" + REPO + "/releases", rel: "noopener", text: "See them on GitHub" }))); return; }
    box.replaceChildren.apply(box, list.map(function (r) {
      var isLatest = state.latest && r.tag === state.latest.tag, hasPreview = state.previews.tags.indexOf(r.tag) >= 0;
      var more = el("div", { class: "more", hidden: "" }, notes(r.notes), el("div", { class: "assets" }, el("h4", { text: "Files" }),
        r.assets.length ? r.assets.map(function (a) { return el("a", { href: a.url, rel: "noopener" }, el("span", { text: a.name }), el("span", { text: fmtSize(a.size) })); }) : el("span", { class: "sub", text: "No files were attached." })));
      var toggle = el("button", { class: "btn sm", type: "button", "aria-expanded": "false", onclick: function (e) {
        var b = e.currentTarget, open = more.hasAttribute("hidden"); if (open) more.removeAttribute("hidden"); else more.setAttribute("hidden", "");
        b.setAttribute("aria-expanded", String(open)); b.lastChild.textContent = open ? "Hide notes and files" : "Notes and files"; } }, icon("list"), el("span", { text: "Notes and files" }));
      var acts = el("div", { class: "acts" }, toggle);
      if (hasPreview) acts.appendChild(el("button", { class: "btn sm", type: "button", onclick: function () { openPreview(r.tag); } }, icon("eye"), "Preview"));
      acts.appendChild(el("a", { class: "btn sm ghost", href: r.url, rel: "noopener" }, "GitHub", icon("external")));
      var sum = firstLine(r.notes);
      return el("article", { class: "rel reveal in" + (isLatest ? " latest" : "") + (r.prerelease ? " pre" : ""), id: "v-" + r.tag },
        el("div", { class: "head" }, el("h3", { text: "v" + version(r.tag) }),
          el("span", { class: "badge-s " + (isLatest ? "latest" : r.prerelease ? "pre" : "stable"), text: isLatest ? "Latest" : r.prerelease ? "Pre-release" : "Stable" }),
          el("span", { class: "sp" }), el("time", { datetime: r.published || "", text: fmtDate(r.published) })),
        sum ? el("p", { class: "sum", text: sum }) : null, acts, more);
    }));
  }
  safe(function () {
    $$("#ver-filter button").forEach(function (b) {
      b.addEventListener("click", function () { filter = b.getAttribute("data-f"); $$("#ver-filter button").forEach(function (x) { x.setAttribute("aria-pressed", String(x === b)); }); renderVersions(); });
    });
  });

  var lastKey = "";
  function render() {
    var key = JSON.stringify([state.releases.map(function (r) { return [r.tag, r.assets.length, r.notes.length]; }, state.env), state.env, state.previews.tags]);
    if (key === lastKey) return; lastKey = key;
    renderDownloads(); renderVersions(); fillDemoVersions();
  }
  function setReleases(list) {
    list = list.filter(function (r) { return r && r.tag; }).sort(function (a, b) { return new Date(b.published || 0) - new Date(a.published || 0); });
    if (!list.length) return;
    state.releases = list;
    state.latest = list.filter(function (r) { return !r.prerelease; })[0] || list[0];
    render();
  }

  // ------------------------------------------------------------------ the live preview of the web interface
  function demoPath(tag) { return tag ? "demo/" + tag + "/" : "demo/"; }
  function fillDemoVersions() {
    var sel = $("#demo-version"); if (!sel) return;
    var keep = sel.value;
    sel.replaceChildren(el("option", { value: "", text: "Latest build (main)" }));
    state.previews.tags.slice().sort(function (a, b) { return compare(b, a); }).forEach(function (t) { sel.appendChild(el("option", { value: t, text: "v" + version(t) })); });
    sel.value = keep || "";
  }
  function compare(a, b) {
    var pa = version(a).split(/[.-]/), pb = version(b).split(/[.-]/), i;
    for (i = 0; i < 3; i++) { var d = (parseInt(pa[i], 10) || 0) - (parseInt(pb[i], 10) || 0); if (d) return d; }
    var ra = pa[3] ? 0 : 1, rb = pb[3] ? 0 : 1; return ra - rb;
  }
  safe(function () {
    var sel = $("#demo-version"), frame = $("#demo-frame"), launch = $("#demo-launch"), open = $("#demo-open"), url = $("#demo-url"), browser = $("#browser");
    function start() {
      var p = demoPath(sel.value);
      launch.hidden = true; frame.hidden = false; frame.src = p; open.setAttribute("href", p);
      url.textContent = "http://localhost:9615/" + (sel.value ? "  ·  v" + version(sel.value) : "");
    }
    $("#demo-start").addEventListener("click", start);
    $("#demo-reset").addEventListener("click", function () { if (!frame.hidden) { frame.src = "about:blank"; setTimeout(start, 30); } });
    sel.addEventListener("change", function () { open.setAttribute("href", demoPath(sel.value)); if (!frame.hidden) start(); else url.textContent = "http://localhost:9615/" + (sel.value ? "  ·  v" + version(sel.value) : ""); });
    $$(".tools-row .seg button").forEach(function (b) {
      b.addEventListener("click", function () { browser.style.maxWidth = b.getAttribute("data-w"); $$(".tools-row .seg button").forEach(function (x) { x.setAttribute("aria-pressed", String(x === b)); }); });
    });
  });
  var dialog;
  function openPreview(tag) {
    dialog = dialog || $("#preview-dialog");
    var f = $("#pv-frame"), p = demoPath(tag);
    $("#pv-title").textContent = "Preview of v" + version(tag); $("#pv-open").setAttribute("href", p);
    f.src = p;
    if (dialog.showModal) dialog.showModal(); else dialog.setAttribute("open", "");
  }
  safe(function () {
    dialog = $("#preview-dialog");
    var close = function () { $("#pv-frame").src = "about:blank"; if (dialog.close) dialog.close(); else dialog.removeAttribute("open"); };
    $("#pv-close").addEventListener("click", close);
    dialog.addEventListener("click", function (e) { if (e.target === dialog) close(); });
    dialog.addEventListener("close", function () { $("#pv-frame").src = "about:blank"; });
  });

  // ------------------------------------------------------------------ loading the data
  safe(function () {
    detect();
    fetch("demo/versions.json", { cache: "no-cache" }).then(function (r) { return r.ok ? r.json() : null; }).then(function (j) { if (j && j.tags) { state.previews.tags = j.tags; lastKey = ""; render(); } }, function () {});
    fetch("data/releases.json", { cache: "no-cache" }).then(function (r) { return r.ok ? r.json() : null; }).then(function (j) { if (j && j.length) setReleases(j.map(normalize)); }, function () {})
      .then(function () {
        return fetch("https://api.github.com/repos/" + REPO + "/releases?per_page=100", { headers: { Accept: "application/vnd.github+json" } }).then(function (r) { return r.ok ? r.json() : null; });
      }).then(function (j) { if (j && j.length) setReleases(j.filter(function (r) { return !r.draft; }).map(normalize)); else if (!state.releases.length) render(); }, function () { if (!state.releases.length) render(); });
    fetch("https://api.github.com/repos/" + REPO, { headers: { Accept: "application/vnd.github+json" } }).then(function (r) { return r.ok ? r.json() : null; }).then(function (j) {
      if (j && typeof j.stargazers_count === "number") { var n = $("#star-count"); n.textContent = j.stargazers_count.toLocaleString(); n.hidden = false; }
    }, function () {});
  });
})();
