// window.__WIRECRAFT__ is injected by the app before this page runs (src-tauri/src/lib.rs, open_splash).
const boot = window.__WIRECRAFT__ || {};
document.documentElement.dataset.theme = boot.theme === "light" ? "light" : "dark";
if (boot.version) document.getElementById("version").textContent = "Version " + boot.version;
