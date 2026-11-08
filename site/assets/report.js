/* The "report a bug" page: checks the answers and opens a ready-made issue on GitHub.
 * The answers go nowhere until the button is pressed, and then only into the address of the GitHub page. */
(function () {
  "use strict";
  var REPO = "Namitjain07/Swift-Download-Manager";
  var MAX_URL = 7500; // GitHub refuses very long addresses

  function $(s) { return document.querySelector(s); }
  function val(id) { return ($("#f-" + id).value || "").trim(); }

  // light / dark, as on the main page
  $("#theme-toggle").addEventListener("click", function () {
    var root = document.documentElement, next = root.getAttribute("data-theme") === "dark" ? "light" : "dark";
    root.setAttribute("data-theme", next);
    try { localStorage.setItem("sdm-site-theme", next); } catch (e) {}
  });

  // What the app (Help > Report a bug) or the address already knows: ?version=1.9.0&system=Windows+11&install=...
  var q = new URLSearchParams(location.search);
  function pick(sel, wanted) {
    if (!wanted) return false;
    var o = Array.prototype.slice.call(sel.options).filter(function (x) { return x.text.toLowerCase() === wanted.toLowerCase(); })[0];
    if (o) { sel.value = o.value || o.text; return true; }
    return false;
  }
  if (q.get("version")) $("#f-version").value = q.get("version").slice(0, 40);
  var guessed = pick($("#f-system"), q.get("system"));
  pick($("#f-install"), q.get("install"));
  pick($("#f-addon"), q.get("addon"));
  if (q.get("title")) $("#f-title").value = q.get("title").slice(0, 120);
  if (!guessed) {
    var ua = navigator.userAgent || "";
    if (/Windows NT 1[01]/.test(ua)) pick($("#f-system"), /Windows NT 10\.0.*Win64.*(Edg|Chrome|Firefox)/.test(ua) ? "Windows 11" : "Windows 10");
    else if (/Mac OS X/.test(ua)) pick($("#f-system"), "macOS");
    else if (/Linux|X11/.test(ua)) pick($("#f-system"), /Ubuntu|Debian|Mint/.test(ua) ? "Ubuntu / Debian / Mint" : "Other Linux");
  }

  // The versions people can pick from, and the newest one to compare with.
  var latest = "";
  fetch("../data/releases.json").then(function (r) { return r.ok ? r.json() : []; }).then(function (list) {
    var dl = $("#versions");
    (list || []).forEach(function (r) {
      var v = String(r.tag_name || "").replace(/^v/, ""); if (!v) return;
      var o = document.createElement("option"); o.value = v; dl.appendChild(o);
      if (!latest && !r.prerelease) latest = v;
    });
    if (!$("#f-version").value && latest) $("#f-version").placeholder = latest;
  }).catch(function () {});

  function body() {
    var parts = [];
    var add = function (head, text) { parts.push("### " + head + "\n\n" + (text || "_No response_")); };
    add("Version", val("version"));
    add("Your system", $("#f-system").value);
    add("How you got the app", $("#f-install").value);
    add("What happened?", val("what"));
    add("How can we make it happen again?", val("steps"));
    add("What did you expect instead?", val("expected"));
    add("Is the browser add-on involved?", $("#f-addon").value);
    add("Anything else?", val("extra"));
    return parts.join("\n\n");
  }

  // The address of the issue form: each answer goes into the field of the same id (see .github/ISSUE_TEMPLATE/bug_report.yml).
  function issueUrl() {
    var title = val("title"), p = new URLSearchParams();
    p.set("template", "bug_report.yml");
    p.set("title", "[Bug]: " + title);
    p.set("labels", "bug");
    p.set("version", val("version"));
    p.set("system", $("#f-system").value);
    p.set("install", $("#f-install").value);
    p.set("what-happened", val("what"));
    p.set("steps", val("steps"));
    p.set("expected", val("expected"));
    p.set("addon", $("#f-addon").value);
    if (val("extra")) p.set("extra", val("extra"));
    return "https://github.com/" + REPO + "/issues/new?" + p.toString();
  }

  function refresh() {
    $("#preview").textContent = "Title: [Bug]: " + val("title") + "\n\n" + body();
    var n = issueUrl().length;
    $("#count").textContent = n > MAX_URL ? "That is a long report: it will be cut to fit, so paste the rest in the issue." : "";
  }
  Array.prototype.forEach.call(document.querySelectorAll("input, textarea, select"), function (n) { n.addEventListener("input", refresh); n.addEventListener("change", refresh); });
  refresh();

  function check() {
    var ok = true;
    ["version", "title", "what", "steps", "expected"].forEach(function (id) {
      var f = $("#f-" + id), bad = !val(id);
      f.closest(".field").classList.toggle("bad", bad);
      if (bad && ok) { f.focus(); ok = false; }
    });
    return ok;
  }
  Array.prototype.forEach.call(document.querySelectorAll(".field input, .field textarea"), function (n) {
    n.addEventListener("input", function () { n.closest(".field").classList.remove("bad"); });
  });

  $("#form").addEventListener("submit", function (e) {
    e.preventDefault();
    if (!check()) return;
    var url = issueUrl();
    if (url.length > MAX_URL) {
      // Keep the short fields, and put the long ones in as far as they fit.
      var p = new URLSearchParams(url.split("?")[1]), room = MAX_URL - 700;
      ["what-happened", "steps", "expected", "extra"].forEach(function (k) {
        var v = p.get(k); if (v == null) return;
        var cap = Math.max(200, Math.floor(room / 4)); if (v.length > cap) p.set(k, v.slice(0, cap) + " [cut: paste the rest below]");
      });
      url = "https://github.com/" + REPO + "/issues/new?" + p.toString();
    }
    window.open(url, "_blank", "noopener");
    var s = $("#sent"); s.textContent = "GitHub opened in a new tab with your report filled in. Press “Submit new issue” there. You can add screenshots on that page."; s.classList.add("show");
  });

  $("#copy").addEventListener("click", function () {
    if (!check()) return;
    var text = "[Bug]: " + val("title") + "\n\n" + body(), s = $("#sent");
    var done = function () { s.textContent = "Copied. Paste it into an issue or an e-mail."; s.classList.add("show"); };
    if (navigator.clipboard && window.isSecureContext) navigator.clipboard.writeText(text).then(done, function () {});
    else { var ta = document.createElement("textarea"); ta.value = text; document.body.appendChild(ta); ta.select(); try { document.execCommand("copy"); done(); } catch (x) {} ta.remove(); }
  });
})();
