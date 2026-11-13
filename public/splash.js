// window.__WIRECRAFT__ is injected by the app before this page runs (src-tauri/src/lib.rs, open_splash).
const boot = window.__WIRECRAFT__ || {};
document.documentElement.dataset.theme = boot.theme === "light" ? "light" : "dark";
if (boot.version) document.getElementById("version").textContent = "Version " + boot.version;

// Tell the app this page is on screen, so the splash stays up long enough to be seen. The splash has no
// bundled API, so it uses the IPC bridge Tauri injects into every page.
requestAnimationFrame(() =>
  requestAnimationFrame(() => {
    const ipc = window.__TAURI_INTERNALS__;
    if (ipc && typeof ipc.invoke === "function") ipc.invoke("splash_painted").catch(() => {});
  }),
);
