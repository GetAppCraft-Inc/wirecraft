// Actions on flows shared by the table context menu, inspector and command palette.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { save } from "@tauri-apps/plugin-dialog";
import { api, errMsg, type FlowSummary } from "./api";
import { app } from "./state.svelte";
import { copyAs, COPY_FORMATS, type CopyFormat } from "./copyas";
import { url } from "./format";
import type { MenuItem } from "../components/Menu.svelte";
import { tools } from "./tools.svelte";

export async function copyText(text: string, title: string, body?: string) {
  try {
    await writeText(text);
    app.toast({ kind: "success", title, body }, 2500);
  } catch (e) {
    app.toast({ kind: "error", title: "Couldn't copy", body: errMsg(e) });
  }
}

export async function copyFlowAs(id: number, fmt: CopyFormat) {
  try {
    const d = await api.flowDetail(id);
    const label = COPY_FORMATS.find(([f]) => f === fmt)?.[1] ?? fmt;
    const headers = d.reqHeaders.filter(([k]) => !k.startsWith(":")).length;
    await copyText(copyAs(fmt, d), `Copied as ${label}`, `${d.summary.method} ${url(d.summary)} · ${headers} headers`);
  } catch (e) {
    app.toast({ kind: "error", title: "Couldn't copy", body: errMsg(e) });
  }
}

export async function exportHar(ids?: number[]) {
  const path = await save({ defaultPath: ids ? "xwire-selection.har" : "xwire-session.har", filters: [{ name: "HAR", extensions: ["har"] }] });
  if (!path) return;
  try {
    const n = await api.exportHar(path, ids);
    app.toast({ kind: "success", title: `Exported ${n} request${n === 1 ? "" : "s"}`, body: path });
  } catch (e) {
    app.toast({ kind: "error", title: "Export failed", body: errMsg(e) });
  }
}

export function neverDecrypt(host: string) {
  const s = app.settings;
  if (!s) return;
  const cfg = { ...s.intercept, never: [...new Set([...s.intercept.never, host])] };
  api.setIntercept(cfg).then(() => {
    app.settings = { ...s, intercept: cfg };
    app.toast({ kind: "info", title: `${host} won't be decrypted`, body: "New connections to it are tunnelled untouched." });
  });
}

/** Map Local prefilled with the captured response. */
export async function mapLocalFrom(f: FlowSummary) {
  try {
    const d = await api.flowDetail(f.id);
    tools.addMapLocal(f, d.respBody.text ?? "", d.respHeaders, d.summary.status ?? 200);
  } catch {
    tools.addMapLocal(f);
  }
}

export function rowMenu(f: FlowSummary): MenuItem[] {
  const sel = app.selection();
  const many = sel.length > 1;
  const fav = app.settings?.favourites.includes(f.host) ?? false;
  return [
    { header: many ? `${sel.length} requests` : `${f.method} ${f.path.split("?")[0]}`.slice(0, 48) },
    { label: app.pinned.has(f.id) ? "Unpin" : "Pin", icon: "pin", keys: "P", run: () => app.togglePin(f.id) },
    { label: fav ? "Remove host from favourites" : "Add host to favourites", icon: "star", run: () => app.toggleFavourite(f.host) },
    "-",
    { label: "Copy URL", icon: "link", keys: "Ctrl C", run: () => copyText(url(f), "Copied URL", url(f)) },
    { label: "Copy as cURL", icon: "terminal", keys: "Ctrl ⇧ C", disabled: f.method === "CONNECT", run: () => copyFlowAs(f.id, "curl") },
    {
      label: "Copy as…",
      icon: "code",
      disabled: f.method === "CONNECT",
      sub: COPY_FORMATS.map(([fmt, label]) => ({ label, icon: fmt === "curl" ? "terminal" : "code", run: () => copyFlowAs(f.id, fmt) }) as MenuItem),
    },
    "-",
    { label: "Repeat", icon: "repeat", keys: "R", disabled: f.method === "CONNECT", run: () => app.replay(f.id) },
    { label: "Edit and repeat", icon: "send", keys: "E", disabled: f.method === "CONNECT", run: () => tools.composeFrom(f.id) },
    { label: sel.length === 2 ? "Compare selected" : "Compare with… (select two)", icon: "diff", keys: "D", disabled: sel.length !== 2, run: () => tools.compare(sel) },
    "-",
    { label: "Breakpoint on this endpoint", icon: "breakpoint", keys: "B", disabled: f.method === "CONNECT", run: () => tools.addBreakpoint(f) },
    { label: "Map local…", icon: "map", disabled: f.method === "CONNECT", run: () => mapLocalFrom(f) },
    { label: "Map remote…", icon: "remote", run: () => tools.addMapRemote(f) },
    { label: "New rewrite rule…", icon: "rewrite", run: () => tools.addRewrite(f) },
    { label: "Block host", icon: "x", run: () => tools.blockHost(f.host) },
    "-",
    { label: "Filter to this host", icon: "filter", run: () => (app.query = `host:${f.host}`) },
    { label: "Never decrypt this host", icon: "unlock", disabled: !f.tls, run: () => neverDecrypt(f.host) },
    "-",
    { label: many ? `Export ${sel.length} as HAR…` : "Export as HAR…", icon: "download", run: () => exportHar(many ? sel : [f.id]) },
    { label: many ? `Delete ${sel.length}` : "Delete", icon: "trash", keys: "Del", danger: true, run: () => app.remove(many ? sel : [f.id]) },
  ];
}
