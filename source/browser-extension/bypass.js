// Alt+click on a link leaves that one download to the browser instead of sending it to Swift Download Manager.
// Runs in every page; it only tells the add-on's background script which link was Alt+clicked.
(() => {
  const api = globalThis.browser ?? globalThis.chrome;
  document.addEventListener(
    "click",
    (e) => {
      if (!e.altKey) return;
      const a = e.target && e.target.closest ? e.target.closest("a[href]") : null;
      if (a && a.href) {
        // The answer is not needed: if the add-on is asleep or gone the click just behaves as usual.
        Promise.resolve(api.runtime.sendMessage({ type: "bypass", url: a.href })).catch(() => {});
      }
    },
    true
  );
})();
