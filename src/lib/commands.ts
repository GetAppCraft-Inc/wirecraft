// Commands for the palette (Ctrl+K) and keyboard shortcuts.
import { api, errMsg } from "./api";
import { app } from "./state.svelte";
import { copyFlowAs, exportHar } from "./actions";
import type { IconName } from "./icons";
import { tools } from "./tools.svelte";
import { importHar } from "./sessions";

export interface Command {
  id: string;
  title: string;
  icon: IconName;
  keys?: string[];
  when?: () => boolean;
  run: () => void;
}

export const SHORTCUTS: [string[], string][] = [
  [["Ctrl", "R"], "Start / stop recording"],
  [["Space"], "Pause / resume the live table"],
  [["Ctrl", "L"], "Focus the filter"],
  [["Ctrl", "K"], "Command palette"],
  [["↑", "↓"], "Previous / next request"],
  [["Home", "End"], "First request / follow new traffic"],
  [["R"], "Repeat the selected request"],
  [["P"], "Pin / unpin"],
  [["E"], "Edit and repeat"],
  [["B"], "Breakpoint on this endpoint"],
  [["D"], "Compare the two selected requests"],
  [["Ctrl", "⇧", "C"], "Copy as cURL"],
  [["Del"], "Delete selected"],
  [["Ctrl", "⇧", "X"], "Clear all"],
  [["Ctrl", "E"], "Export session as HAR"],
  [["Ctrl", "B"], "Toggle sidebar"],
  [["Esc"], "Clear the filter / close"],
  [["?"], "Show shortcuts"],
];

export function commands(hooks: { focusFilter: () => void; toggleSidebar: () => void }): Command[] {
  const sel = () => app.selected;
  return [
    { id: "rec", title: app.recording ? "Stop recording" : "Start recording", icon: "record", keys: ["Ctrl", "R"], run: () => app.toggleRecording() },
    { id: "pause", title: app.livePaused ? "Resume live table" : "Pause live table", icon: app.livePaused ? "play" : "pause", keys: ["Space"], run: () => app.toggleLive() },
    { id: "clear", title: "Clear all requests", icon: "trash", keys: ["Ctrl", "⇧", "X"], run: () => app.clear() },
    { id: "filter", title: "Focus filter", icon: "filter", keys: ["Ctrl", "L"], run: hooks.focusFilter },
    { id: "errors", title: "Show only errors", icon: "alert", run: () => (app.chip = "errors") },
    { id: "reset", title: "Reset filters", icon: "x", run: () => { app.query = ""; app.chip = "all"; app.source = null; } },
    { id: "repeat", title: "Repeat selected request", icon: "repeat", keys: ["R"], when: () => sel() != null, run: () => app.replay(sel()!) },
    { id: "curl", title: "Copy selected as cURL", icon: "terminal", keys: ["Ctrl", "⇧", "C"], when: () => sel() != null, run: () => copyFlowAs(sel()!, "curl") },
    { id: "pin", title: "Pin / unpin selected", icon: "pin", keys: ["P"], when: () => sel() != null, run: () => app.togglePin(sel()!) },
    { id: "compose", title: "Compose a new request", icon: "send", run: () => tools.composeFrom(null) },
    { id: "edit", title: "Edit and repeat selected", icon: "send", keys: ["E"], when: () => sel() != null, run: () => tools.composeFrom(sel()!) },
    { id: "diff", title: "Compare two selected requests", icon: "diff", keys: ["D"], run: () => tools.compare(app.selection()) },
    { id: "bp", title: "Breakpoints…", icon: "breakpoint", run: () => tools.open("breakpoints") },
    { id: "ml", title: "Map Local…", icon: "map", run: () => tools.open("mapLocal") },
    { id: "mr", title: "Map Remote…", icon: "remote", run: () => tools.open("mapRemote") },
    { id: "rw", title: "Rewrite rules…", icon: "rewrite", run: () => tools.open("rewrite") },
    { id: "js", title: "Scripts…", icon: "code", run: () => tools.open("scripts") },
    { id: "jslog", title: "Clear script console", icon: "trash", run: () => api.clearScriptLogs() },
    { id: "block", title: "Blocked hosts…", icon: "x", run: () => tools.open("block") },
    { id: "throttle", title: "Throttle the network…", icon: "throttle", run: () => tools.open("throttle") },
    { id: "resume", title: "Resume all paused requests", icon: "play", when: () => tools.paused.length > 0, run: () => tools.resumeAll() },
    { id: "import", title: "Import HAR…", icon: "upload", run: () => importHar() },
    { id: "har", title: "Export session as HAR…", icon: "download", keys: ["Ctrl", "E"], run: () => exportHar() },
    { id: "sidebar", title: "Toggle sidebar", icon: "sidebar", keys: ["Ctrl", "B"], run: hooks.toggleSidebar },
    { id: "theme", title: app.settings?.theme === "light" ? "Switch to dark theme" : "Switch to light theme", icon: "eye", run: () => app.setTheme(app.settings?.theme === "light" ? "dark" : "light") },
    {
      id: "sysproxy",
      title: "Toggle system proxy",
      icon: "plug",
      run: async () => {
        try {
          const cur = await api.systemProxy();
          const r = await api.setSystemProxy(!cur.enabled);
          app.toast({ kind: "success", title: r.enabled ? "System proxy on" : "System proxy off", body: r.backend });
        } catch (e) {
          app.toast({ kind: "error", title: "Couldn't change the system proxy", body: errMsg(e) });
        }
      },
    },
    { id: "cert", title: "Root certificate setup…", icon: "shield", run: () => { app.onboardingStep = 1; app.screen = "onboarding"; } },
    { id: "proxy", title: "Proxy & port settings…", icon: "settings", run: () => { app.onboardingStep = 2; app.screen = "onboarding"; } },
    { id: "keys", title: "Keyboard shortcuts", icon: "keyboard", keys: ["?"], run: () => (app.shortcuts = true) },
  ];
}
