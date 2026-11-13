// HAR import and saved sessions (HAR files in WireCraft's data folder).
import { open } from "@tauri-apps/plugin-dialog";
import { api, errMsg } from "./api";
import { app } from "./state.svelte";
import { tools } from "./tools.svelte";

export async function importHar(path?: string) {
  const p = path ?? (await open({ multiple: false, filters: [{ name: "HAR or Charles session", extensions: ["har", "json", "chlsj"] }] }));
  if (typeof p !== "string") return;
  try {
    const n = await api.importHar(p);
    app.toast({ kind: "success", title: `Imported ${n} request${n === 1 ? "" : "s"}`, body: p.split(/[\\/]/).pop() });
  } catch (e) {
    app.toast({ kind: "error", title: "Import failed", body: errMsg(e) });
  }
}

export async function saveSession(name: string, ids?: number[]) {
  try {
    const s = await api.saveSession(name, ids);
    await tools.refreshSessions();
    app.toast({ kind: "success", title: `Saved session “${s.name}”` }, 2500);
    return true;
  } catch (e) {
    app.toast({ kind: "error", title: "Couldn't save the session", body: errMsg(e) });
    return false;
  }
}

export async function deleteSession(path: string) {
  try {
    await api.deleteSession(path);
    await tools.refreshSessions();
  } catch (e) {
    app.toast({ kind: "error", title: "Couldn't delete", body: errMsg(e) });
  }
}
