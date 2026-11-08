Swift Download Manager Helper (browser add-on)

What it does
  - Sends downloads you start in the browser to Swift Download Manager.
  - Adds "Download with Swift Download Manager" when you right-click a link, picture or video, and
    "Download the selected links" and "Download all the files on this page" for several at once.
  - Sends the site's cookies along, so downloads that need you to be logged in work.
  - Spots videos and sounds a page plays, and lists the files a page links to (archives, programs,
    documents, pictures ...): open the pop-up, tick what you want, click "Download".
    (Streams split into many small pieces, like most live TV and some big video sites, are not supported.)
  - The pop-up shows what the app is downloading right now, with pause and resume, and opens the app.
  - Settings in the pop-up: only catch big files, only some file types, leave some sites to the browser.
    Hold Alt while you click a link to let the browser download that one itself.
  - If the app is not open, the browser downloads as usual.

Install in Chrome, Edge or Brave
  1. Open the extensions page (chrome://extensions or edge://extensions).
  2. Turn on "Developer mode".
  3. Click "Load unpacked" and choose this folder.

Install in Firefox
  1. Open about:debugging#/runtime/this-firefox
  2. Click "Load Temporary Add-on…" and choose manifest.json in this folder.

The add-on only talks to the app on this computer (127.0.0.1, port 9614), and the app only answers
this add-on: a web page cannot reach it. It also reads the pages you view only when you open the
pop-up (to list their links) or use the right-click menu.
