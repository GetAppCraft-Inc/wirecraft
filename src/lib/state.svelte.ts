// App-wide reactive state. Flows live in plain (non-proxied) structures for speed; `version` is bumped to
// signal changes so derived views recompute.
import { api, on, errMsg, type AppState, type FlowDetail, type FlowSummary, type Settings, type Stats, type StoreEvent, type ProxyStatus, type TrustEntry } from "./api";
import { bodyNeedles, matches, parse, type MatchCtx } from "./query";
import { url } from "./format";

export type Chip = "all" | "xhr" | "js" | "css" | "img" | "media" | "ws" | "grpc" | "gql" | "errors";
export const CHIPS: [Chip, string][] = [
  ["all", "All"], ["xhr", "XHR"], ["js", "JS"], ["css", "CSS"], ["img", "Img"], ["media", "Media"],
  ["ws", "WS"], ["grpc", "gRPC"], ["gql", "GraphQL"],
];

export function chipMatch(c: Chip, f: FlowSummary): boolean {
  switch (c) {
    case "all": return true;
    case "xhr": return f.xhr || f.kind === "JSON" || f.kind === "GQL";
    case "js": return f.kind === "JS";
    case "css": return f.kind === "CSS";
    case "img": return f.kind === "Img";
    case "media": return f.kind === "Media";
    case "ws": return f.kind === "WS";
    case "grpc": return f.kind === "gRPC";
    case "gql": return f.kind === "GQL";
    case "errors": return f.state === "error" || (f.status ?? 0) >= 400;
  }
}

export type SourceSel =
  | { kind: "client"; client: string; host?: string }
  | { kind: "domain"; domain: string; host?: string }
  | { kind: "device"; device: string; client?: string }
  | { kind: "pinned" }
  | { kind: "pin"; id: number }
  | { kind: "fav"; host: string };

export type SortKey = "idx" | "method" | "status" | "host" | "path" | "type" | "size" | "dur" | "time" | "client";
export const COLUMNS: { key: SortKey | "tls"; title: string; width: number | null; align?: "right" }[] = [
  // widths from design/generator/ui.py COLS
  { key: "idx", title: "#", width: 48, align: "right" },
  { key: "tls", title: "", width: 18 },
  { key: "method", title: "Method", width: 62 },
  { key: "status", title: "Status", width: 46 },
  { key: "host", title: "Host", width: 128 },
  { key: "path", title: "Path", width: null },
  { key: "type", title: "Type", width: 46 },
  { key: "size", title: "Size", width: 58, align: "right" },
  { key: "dur", title: "Duration", width: 62, align: "right" },
  { key: "time", title: "Time", width: 64 },
  { key: "client", title: "Client", width: 68 },
];

export interface Toast {
  id: number;
  kind: "success" | "error" | "info" | "warning";
  title: string;
  body?: string;
  action?: { label: string; run: () => void };
}

/** Registrable-ish domain: last two labels (three for common two-part TLDs). */
export function baseDomain(host: string): string {
  if (/^[\d.]+$/.test(host) || host.includes(":")) return host;
  const p = host.split(".");
  if (p.length <= 2) return host;
  const two = p.slice(-2).join(".");
  if (/^(co|com|org|net|ac|gov|edu)\.[a-z]{2}$/.test(two)) return p.slice(-3).join(".");
  return two;
}

export function deviceOf(f: FlowSummary): string {
  const ip = f.clientAddr.replace(/:\d+$/, "").replace(/^\[|\]$/g, "");
  if (ip === "wirecraft") return "WireCraft";
  if (ip === "127.0.0.1" || ip === "::1" || ip.startsWith("::ffff:127.")) return "This computer";
  return ip.replace(/^::ffff:/, "");
}

class App {
  // ---- server state
  ready = $state(false);
  info = $state<AppState | null>(null);
  settings = $state<Settings | null>(null);
  proxy = $state<ProxyStatus | null>(null);
  stats = $state<Stats>({ flows: 0, bytes: 0, requestsTotal: 0, recording: true });
  rate = $state(0);
  recording = $state(true);

  // ---- flows (non-reactive storage + version signal)
  byId = new Map<number, FlowSummary>();
  order: number[] = [];
  version = $state(0);
  /** Throttled version for heavier views (sidebar groups, chip counts). */
  slowVersion = $state(0);
  livePaused = $state(false);
  /** Arrivals since the table was paused or scrolled away from the bottom. */
  newSincePause = $state(0);

  // ---- view state
  query = $state("");
  chip = $state<Chip>("all");
  source = $state<SourceSel | null>(null);
  sort = $state<{ key: SortKey; desc: boolean }>({ key: "idx", desc: false });
  hiddenCols = $state<string[]>([]);
  selected = $state<number | null>(null);
  multi = $state<Set<number>>(new Set());
  pinned = $state<Set<number>>(new Set());
  bodyHits = $state.raw<Map<string, Set<number>>>(new Map());
  detail = $state.raw<FlowDetail | null>(null);
  detailError = $state<string | null>(null);
  toasts = $state<Toast[]>([]);
  screen = $state<"workspace" | "onboarding">("workspace");
  onboardingStep = $state(0);
  palette = $state(false);
  trust = $state<TrustEntry[] | null>(null);
  /** Whether the OS trust store has the CA (null while unknown). */
  caTrusted = $derived(this.trust ? this.trust.find((t) => t.id === "system")?.state === "trusted" : null);
  shortcuts = $state(false);

  /** Called with the upserted flows of each batch (set by the tools module). */
  onFlows: ((flows: FlowSummary[]) => void) | null = null;
  private toastSeq = 0;
  private lastTotal = 0;
  private detailTimer: ReturnType<typeof setTimeout> | null = null;
  private bodyTimer: ReturnType<typeof setTimeout> | null = null;

  // Incremental filtering: ids upserted / newly arrived since `filtered` last ran, and whether something
  // happened (removal, clear, resync) that needs a full rebuild.
  private dirty = new Set<number>();
  private arrivedIds = new Set<number>();
  private structural = true;
  private lastFilter: { key: unknown[]; out: FlowSummary[]; pos: Map<number, number> } | null = null;

  terms = $derived(parse(this.query));
  /** Number of captured flows (reactive). */
  total = $derived.by(() => {
    void this.version;
    return this.order.length;
  });

  filtered = $derived.by(() => {
    void this.version;
    const terms = this.terms;
    const chip = this.chip;
    const src = this.source;
    const sort = this.sort;
    const key: unknown[] = [terms, chip, src, sort.key, sort.desc, this.pinned, this.bodyHits];
    const ctx: MatchCtx = { pinned: this.pinned, bodyHits: this.bodyHits };
    const test = (f: FlowSummary) =>
      (chip === "all" || chipMatch(chip, f)) && (!src || this.sourceMatch(src, f)) && (!terms.length || matches(terms, f, ctx, url(f)));

    const dirty = this.dirty, arrived = this.arrivedIds;
    this.dirty = new Set();
    this.arrivedIds = new Set();
    const prev = this.lastFilter;
    const sameFilter = prev != null && !this.structural && prev.key.length === key.length && prev.key.every((k, i) => k === key[i]);
    // Patch the previous result when only some flows changed and rows are in arrival order.
    if (sameFilter && prev && sort.key === "idx" && !sort.desc) {
      const out = prev.out.slice();
      const pos = prev.pos;
      let ok = true;
      for (const id of dirty) {
        const f = this.byId.get(id);
        if (!f) continue;
        const at = pos.get(id);
        const hit = test(f);
        if (at != null) {
          if (!hit) { ok = false; break; } // dropped out of the view: rebuild
          out[at] = f;
        } else if (hit) {
          if (!arrived.has(id)) { ok = false; break; } // an older flow started matching: rebuild
        }
      }
      if (ok) {
        // arrivals go at the end, in arrival order
        for (const id of this.order.slice(this.order.length - arrived.size)) {
          if (!arrived.has(id) || pos.has(id)) continue;
          const f = this.byId.get(id);
          if (f && test(f)) {
            pos.set(id, out.length);
            out.push(f);
          }
        }
        this.lastFilter = { key, out, pos };
        return out;
      }
    }

    this.structural = false;
    const out: FlowSummary[] = [];
    for (const id of this.order) {
      const f = this.byId.get(id);
      if (f && test(f)) out.push(f);
    }
    const { key: sk, desc } = sort;
    if (sk !== "idx" || desc) {
      const val = (f: FlowSummary): number | string => {
        switch (sk) {
          case "idx": case "time": return f.id;
          case "method": return f.method;
          case "status": return f.status ?? -1;
          case "host": return f.host;
          case "path": return f.path;
          case "type": return f.kind;
          case "size": return f.respSize;
          case "dur": return f.durationMs ?? -1;
          case "client": return f.client ?? "";
        }
      };
      out.sort((a, b) => {
        const x = val(a), y = val(b);
        const c = x < y ? -1 : x > y ? 1 : a.id - b.id;
        return desc ? -c : c;
      });
    }
    this.lastFilter = { key, out, pos: new Map(out.map((f, i) => [f.id, i])) };
    return out;
  });

  sourceMatch(s: SourceSel, f: FlowSummary): boolean {
    switch (s.kind) {
      case "client": return (f.client ?? "Unknown") === s.client && (!s.host || f.host === s.host);
      case "domain": return baseDomain(f.host) === s.domain && (!s.host || f.host === s.host);
      case "device": return deviceOf(f) === s.device && (!s.client || (f.client ?? "Unknown") === s.client);
      case "pinned": return this.pinned.has(f.id);
      case "pin": return f.id === s.id;
      case "fav": return f.host === s.host;
    }
  }

  // ---------------------------------------------------------------- lifecycle
  async init() {
    const st = await api.appState();
    this.info = st;
    this.settings = st.settings;
    this.proxy = st.proxy;
    this.recording = st.recording;
    applyTheme(st.settings.theme);
    if (!st.settings.onboarded) this.screen = "onboarding";
    try {
      this.hiddenCols = JSON.parse(localStorage.getItem("wirecraft.hiddenCols") ?? "[]");
    } catch {
      /* ignore */
    }
    await this.resync();
    this.refreshTrust();
    await on<StoreEvent[]>("flows", (evs) => this.apply(evs));
    await on("resync", () => this.resync());
    await on<Stats>("stats", (s) => {
      this.rate = Math.max(0, s.requestsTotal - this.lastTotal);
      this.lastTotal = s.requestsTotal;
      this.stats = s;
    });
    await on<ProxyStatus>("proxy", (p) => (this.proxy = p));
    setInterval(() => {
      if (this.slowVersion !== this.version) this.slowVersion = this.version;
    }, 400);
    this.ready = true;
  }

  async resync() {
    const all = await api.listFlows();
    this.byId.clear();
    this.order = [];
    this.structural = true;
    for (const f of all) {
      this.byId.set(f.id, f);
      this.order.push(f.id);
    }
    this.bump();
  }

  private bump() {
    if (!this.livePaused) this.version++;
  }

  private apply(evs: StoreEvent[]) {
    let touchedSelected = false;
    let arrived = 0;
    for (const ev of evs) {
      if (ev.type === "upsert") {
        const f = ev.flow;
        if (!this.byId.has(f.id)) {
          this.order.push(f.id);
          this.arrivedIds.add(f.id);
          arrived++;
        }
        this.byId.set(f.id, f);
        this.dirty.add(f.id);
        if (f.id === this.selected) touchedSelected = true;
      } else if (ev.type === "removed") {
        const set = new Set(ev.ids);
        this.structural = true;
        for (const id of ev.ids) this.byId.delete(id);
        this.order = this.order.filter((id) => !set.has(id));
        if (this.selected != null && set.has(this.selected)) this.select(null);
      } else {
        this.byId.clear();
        this.order = [];
        this.structural = true;
        this.select(null);
        this.newSincePause = 0;
      }
    }
    if (arrived) {
      this.newSincePause += arrived;
      this.scheduleBodySearch();
    }
    if (this.livePaused && evs.some((e) => e.type !== "upsert")) this.version++;
    else this.bump();
    if (touchedSelected) this.loadDetail(true);
    this.onFlows?.(evs.flatMap((e) => (e.type === "upsert" ? [e.flow] : [])));
  }

  // ---------------------------------------------------------------- selection & detail
  select(id: number | null, opts: { toggle?: boolean; range?: boolean } = {}) {
    if (id != null && opts.toggle) {
      const m = new Set(this.multi);
      if (m.has(id)) m.delete(id);
      else m.add(id);
      if (this.selected != null) m.add(this.selected);
      this.multi = m;
    } else if (id != null && opts.range && this.selected != null) {
      const ids = this.filtered.map((f) => f.id);
      const a = ids.indexOf(this.selected), b = ids.indexOf(id);
      if (a >= 0 && b >= 0) this.multi = new Set(ids.slice(Math.min(a, b), Math.max(a, b) + 1));
      return;
    } else {
      this.multi = new Set();
    }
    if (this.selected === id) return;
    this.selected = id;
    this.detail = null;
    this.detailError = null;
    this.loadDetail(false);
  }

  /** Ids the user is acting on: the multi-selection, or the selected row. */
  selection(): number[] {
    if (this.multi.size) return [...this.multi];
    return this.selected != null ? [this.selected] : [];
  }

  private loadDetail(throttled: boolean) {
    const id = this.selected;
    if (id == null) return;
    if (this.detailTimer) clearTimeout(this.detailTimer);
    this.detailTimer = setTimeout(async () => {
      try {
        const d = await api.flowDetail(id);
        if (this.selected === id) {
          this.detail = d;
          this.detailError = null;
        }
      } catch (e) {
        if (this.selected === id) this.detailError = errMsg(e);
      }
    }, throttled ? 150 : 0);
  }

  // ---------------------------------------------------------------- body search
  scheduleBodySearch(immediate = false) {
    const needles = bodyNeedles(this.terms);
    if (!needles.length) {
      if (this.bodyHits.size) this.bodyHits = new Map();
      return;
    }
    if (this.bodyTimer) clearTimeout(this.bodyTimer);
    this.bodyTimer = setTimeout(async () => {
      const m = new Map<string, Set<number>>();
      for (const n of needles) m.set(n, new Set(await api.searchBodies(n)));
      this.bodyHits = m;
    }, immediate ? 120 : 1000);
  }

  // ---------------------------------------------------------------- actions
  toast(t: Omit<Toast, "id">, ms = 4000) {
    const id = ++this.toastSeq;
    this.toasts = [...this.toasts.slice(-3), { ...t, id }];
    setTimeout(() => this.dismiss(id), ms);
  }

  dismiss(id: number) {
    this.toasts = this.toasts.filter((t) => t.id !== id);
  }

  async toggleRecording() {
    this.recording = await api.setRecording(!this.recording);
    this.toast({ kind: "info", title: this.recording ? "Recording" : "Recording stopped", body: this.recording ? undefined : "New traffic still flows through the proxy but isn't captured." }, 2500);
  }

  toggleLive() {
    this.livePaused = !this.livePaused;
    if (!this.livePaused) this.version++;
  }

  async clear() {
    await api.clearFlows();
    this.pinned = new Set();
  }

  async remove(ids: number[]) {
    if (!ids.length) return;
    await api.removeFlows(ids);
    this.multi = new Set();
  }

  togglePin(id: number) {
    const p = new Set(this.pinned);
    if (p.has(id)) p.delete(id);
    else p.add(id);
    this.pinned = p;
    this.version++;
  }

  async toggleFavourite(host: string) {
    if (!this.settings) return;
    const fav = this.settings.favourites.includes(host)
      ? this.settings.favourites.filter((h) => h !== host)
      : [...this.settings.favourites, host];
    this.settings = await api.setPrefs({ favourites: fav });
  }

  async setTheme(theme: "dark" | "light") {
    applyTheme(theme);
    this.settings = await api.setPrefs({ theme });
  }

  async replay(id: number) {
    try {
      const nid = await api.replay(id);
      this.select(nid);
      this.toast({ kind: "success", title: `Repeated as #${nid}` }, 2000);
    } catch (e) {
      this.toast({ kind: "error", title: "Repeat failed", body: errMsg(e) });
    }
  }

  async refreshTrust() {
    try {
      this.trust = await api.trustStatus();
    } catch {
      /* leave unknown */
    }
  }

  async refreshState() {
    const st = await api.appState();
    this.info = st;
    this.settings = st.settings;
    this.proxy = st.proxy;
  }

  hideCol(key: string, hidden: boolean) {
    this.hiddenCols = hidden ? [...new Set([...this.hiddenCols, key])] : this.hiddenCols.filter((k) => k !== key);
    try {
      localStorage.setItem("wirecraft.hiddenCols", JSON.stringify(this.hiddenCols));
    } catch {
      /* ignore */
    }
  }
}

export function applyTheme(t: string) {
  document.documentElement.dataset.theme = t === "light" ? "light" : "dark";
}

export const app = new App();
