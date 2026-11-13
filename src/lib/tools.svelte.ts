// State for the traffic tools: rules (Map Local / Map Remote / Rewrite / Breakpoints / Block / Throttle),
// paused exchanges, Compose and Diff.
import { api, errMsg, type Breakpoint, type Headers, type MapLocal, type MapRemote, type Matcher, type Paused, type Rewrite, type Rules, type SessionFile, type FlowSummary } from "./api";
import { app } from "./state.svelte";
import { url as flowUrl } from "./format";

export type Tool = "breakpoints" | "mapLocal" | "mapRemote" | "rewrite" | "throttle" | "block";

let seq = 0;
export function newId(): string {
  return `r${Date.now().toString(36)}${(seq++).toString(36)}`;
}

export function matcher(url = "", methods: string[] = []): Matcher {
  return { url, methods };
}

export function newMapLocal(m = matcher()): MapLocal {
  return { id: newId(), enabled: true, name: "", match: m, status: 200, headers: [["content-type", "application/json"]], body: "{\n  \n}", file: "" };
}
export function newMapRemote(m = matcher()): MapRemote {
  return { id: newId(), enabled: true, name: "", match: m, to: "http://localhost:3000", replacePath: false };
}
export function newRewrite(m = matcher()): Rewrite {
  return { id: newId(), enabled: true, name: "", match: m, target: "requestHeader", action: "set", key: "", value: "", find: "", regex: false };
}
export function newBreakpoint(m = matcher()): Breakpoint {
  return { id: newId(), enabled: true, name: "", match: m, request: true, response: false };
}

export const THROTTLE_PRESETS: { name: string; latencyMs: number; downloadKbps: number; uploadKbps: number }[] = [
  { name: "3G", latencyMs: 300, downloadKbps: 1600, uploadKbps: 750 },
  { name: "Slow 3G", latencyMs: 2000, downloadKbps: 400, uploadKbps: 400 },
  { name: "4G", latencyMs: 80, downloadKbps: 9000, uploadKbps: 3000 },
  { name: "DSL", latencyMs: 20, downloadKbps: 2000, uploadKbps: 1000 },
  { name: "Edge", latencyMs: 840, downloadKbps: 240, uploadKbps: 200 },
  { name: "Latency only (500 ms)", latencyMs: 500, downloadKbps: 0, uploadKbps: 0 },
];

/** Endpoint matcher for a captured flow: host + path without the query, with the method. */
export function matcherFor(f: FlowSummary, withMethod = true): Matcher {
  const path = f.path.split("?")[0];
  const def = f.scheme === "https" ? 443 : 80;
  const host = f.port === def ? f.host : `${f.host}:${f.port}`;
  return matcher(`${host}${path}*`, withMethod ? [f.method] : []);
}

export interface ComposeDraft {
  method: string;
  url: string;
  headers: string;
  body: string;
}

export function headersToText(h: Headers): string {
  return h.filter(([k]) => !k.startsWith(":")).map(([k, v]) => `${k}: ${v}`).join("\n");
}

export function textToHeaders(t: string): Headers {
  return t
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l && !l.startsWith("#"))
    .map((l) => {
      const i = l.indexOf(":", l.startsWith(":") ? 1 : 0);
      return (i < 0 ? [l, ""] : [l.slice(0, i).trim(), l.slice(i + 1).trim()]) as [string, string];
    })
    .filter(([k]) => k && !k.startsWith(":"));
}

class Tools {
  rules = $state<Rules | null>(null);
  paused = $state<Paused[]>([]);
  sheet = $state<Tool | null>(null);
  /** Rule to select when a sheet opens. */
  focus = $state<string | null>(null);
  compose = $state<ComposeDraft | null>(null);
  diff = $state<[number, number] | null>(null);
  sessions = $state<SessionFile[]>([]);
  private saveTimer: ReturnType<typeof setTimeout> | null = null;
  private pausedTimer: ReturnType<typeof setTimeout> | null = null;

  counts = $derived.by(() => {
    const r = this.rules;
    const n = (on: boolean, list: { enabled: boolean }[]) => (on ? list.filter((x) => x.enabled).length : 0);
    return {
      breakpoints: r ? n(r.breakpointsOn, r.breakpoints) : 0,
      mapLocal: r ? n(r.mapLocalOn, r.mapLocal) : 0,
      mapRemote: r ? n(r.mapRemoteOn, r.mapRemote) : 0,
      rewrite: r ? n(r.rewriteOn, r.rewrite) : 0,
      block: r ? r.block.filter((b) => b.enabled).length : 0,
    };
  });

  async init() {
    this.rules = await api.getRules();
    this.paused = await api.paused();
    this.refreshSessions();
    app.onFlows = (flows) => {
      // a flow entering or leaving the paused state means the breakpoint queue changed
      if (flows.some((f) => f.state === "paused") || this.paused.some((p) => flows.some((f) => f.id === p.flowId))) this.refreshPaused();
    };
  }

  refreshPaused() {
    if (this.pausedTimer) clearTimeout(this.pausedTimer);
    this.pausedTimer = setTimeout(async () => {
      const before = this.paused.length;
      this.paused = await api.paused();
      if (this.paused.length > before) {
        const p = this.paused[this.paused.length - 1];
        app.toast({ kind: "warning", title: `Paused at ${p.phase} breakpoint`, body: `${p.method} ${p.url}`, action: { label: "Show", run: () => app.select(p.flowId) } }, 6000);
      }
    }, 60);
  }

  /** Persist after edits (debounced so typing doesn't write on every key). */
  save(now = false) {
    if (!this.rules) return;
    if (this.saveTimer) clearTimeout(this.saveTimer);
    const snap = $state.snapshot(this.rules) as Rules;
    const run = () => api.setRules(snap).catch((e) => app.toast({ kind: "error", title: "Couldn't save rules", body: errMsg(e) }));
    if (now) run();
    else this.saveTimer = setTimeout(run, 300);
  }

  open(t: Tool, focus: string | null = null) {
    this.focus = focus;
    this.sheet = t;
  }

  // ---- quick actions from the table / inspector
  addBreakpoint(f: FlowSummary) {
    if (!this.rules) return;
    const b = newBreakpoint(matcherFor(f));
    this.rules.breakpoints.push(b);
    this.rules.breakpointsOn = true;
    this.save(true);
    app.toast({ kind: "success", title: "Breakpoint added", body: `${b.match.methods.join(",")} ${b.match.url}`, action: { label: "Edit", run: () => this.open("breakpoints", b.id) } });
  }

  addMapLocal(f: FlowSummary, body?: string, headers?: Headers, status?: number) {
    if (!this.rules) return;
    const r = newMapLocal(matcherFor(f));
    if (body != null) r.body = body;
    if (headers) r.headers = headers.filter(([k]) => !k.startsWith(":") && !["content-length", "content-encoding", "transfer-encoding"].includes(k.toLowerCase()));
    if (status) r.status = status;
    this.rules.mapLocal.push(r);
    this.rules.mapLocalOn = true;
    this.save(true);
    this.open("mapLocal", r.id);
  }

  addMapRemote(f: FlowSummary) {
    if (!this.rules) return;
    const r = newMapRemote(matcher(`${f.host}*`));
    this.rules.mapRemote.push(r);
    this.rules.mapRemoteOn = true;
    this.save(true);
    this.open("mapRemote", r.id);
  }

  addRewrite(f: FlowSummary) {
    if (!this.rules) return;
    const r = newRewrite(matcherFor(f, false));
    this.rules.rewrite.push(r);
    this.rules.rewriteOn = true;
    this.save(true);
    this.open("rewrite", r.id);
  }

  blockHost(host: string) {
    if (!this.rules) return;
    this.rules.block.push({ id: newId(), enabled: true, match: matcher(host) });
    this.save(true);
    app.toast({ kind: "info", title: `Blocking ${host}`, body: "Requests get 403 Forbidden.", action: { label: "Undo", run: () => this.unblock(host) } });
  }

  unblock(host: string) {
    if (!this.rules) return;
    this.rules.block = this.rules.block.filter((b) => b.match.url !== host);
    this.save(true);
  }

  async resume(id: number, r: Parameters<typeof api.resume>[1]) {
    const ok = await api.resume(id, r);
    if (!ok) app.toast({ kind: "warning", title: "That exchange is gone", body: "The client disconnected while it was paused." });
    this.paused = await api.paused();
  }

  async resumeAll() {
    await api.resumeAll();
    this.paused = [];
  }

  // ---- compose
  async composeFrom(id: number | null) {
    if (id == null) {
      this.compose = { method: "GET", url: "https://", headers: "accept: */*", body: "" };
      return;
    }
    try {
      const d = await api.flowDetail(id);
      this.compose = { method: d.summary.method, url: flowUrl(d.summary), headers: headersToText(d.reqHeaders), body: d.reqBody.text ?? "" };
    } catch (e) {
      app.toast({ kind: "error", title: "Couldn't load request", body: errMsg(e) });
    }
  }

  async send(d: ComposeDraft) {
    try {
      const id = await api.sendRequest(d.method, d.url, textToHeaders(d.headers), d.body);
      app.select(id);
      return true;
    } catch (e) {
      app.toast({ kind: "error", title: "Couldn't send", body: errMsg(e) });
      return false;
    }
  }

  // ---- diff
  compare(ids: number[]) {
    if (ids.length !== 2) {
      app.toast({ kind: "info", title: "Select two requests to compare", body: "Ctrl+click a second row, then choose Compare." });
      return;
    }
    const [a, b] = [...ids].sort((x, y) => x - y);
    this.diff = [a, b];
  }

  // ---- sessions
  async refreshSessions() {
    this.sessions = await api.listSessions().catch(() => []);
  }
}

export const tools = new Tools();
