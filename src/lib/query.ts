// Filter query language:  status:5xx host:*.api.com method:POST,PUT body:"timeout" -type:img size:>10kb dur:>500ms
// A leading `-` or `!` negates a term. Bare words match anywhere in the URL. Terms are ANDed.
import type { FlowSummary } from "./api";

export type SpanKind = "key" | "op" | "val" | "str" | "glob" | "neg" | "plain" | "bad";
export interface Span {
  text: string;
  kind: SpanKind;
}

export interface Term {
  neg: boolean;
  key: string | null; // canonical key, null for bare words
  op: string; // ":" "=" ">" "<" ">=" "<="
  value: string;
}

export const KEYS: Record<string, string> = {
  status: "status", s: "status", code: "status",
  host: "host", h: "host", domain: "host",
  method: "method", m: "method",
  path: "path", p: "path",
  url: "url", u: "url",
  type: "type", t: "type", kind: "type",
  client: "client", app: "client", c: "client",
  body: "body", b: "body",
  size: "size",
  dur: "dur", duration: "dur", time: "dur",
  scheme: "scheme",
  is: "is",
  id: "id",
};

export const KEY_HELP: { key: string; help: string; values: [string, string][] }[] = [
  { key: "status", help: "response status code or class", values: [["5xx", "Server errors"], ["4xx", "Client errors"], ["3xx", "Redirects"], ["2xx", "Success"], ["404", "Not Found"], ["500", "Internal Server Error"], ["502", "Bad Gateway"], ["pending", "No response yet"]] },
  { key: "method", help: "HTTP method, comma-separated", values: [["GET", ""], ["POST", ""], ["PUT", ""], ["PATCH", ""], ["DELETE", ""], ["OPTIONS", ""], ["CONNECT", "Tunnels"]] },
  { key: "host", help: "host name, * wildcards", values: [] },
  { key: "path", help: "path and query, * wildcards", values: [] },
  { key: "type", help: "resource type", values: [["json", ""], ["js", ""], ["css", ""], ["img", ""], ["media", ""], ["font", ""], ["html", ""], ["ws", "WebSocket"], ["grpc", ""], ["gql", "GraphQL"], ["tls", "Tunnels"]] },
  { key: "body", help: "text in the request or response body", values: [] },
  { key: "client", help: "app or process that made the request", values: [] },
  { key: "size", help: "response size, e.g. >10kb", values: [[">10kb", ""], [">1mb", ""], ["<1kb", ""]] },
  { key: "dur", help: "duration, e.g. >500ms", values: [[">500ms", "Slow"], [">1s", "Very slow"], ["<50ms", "Fast"]] },
  { key: "is", help: "flow state", values: [["error", "Failed or 5xx"], ["pending", "Waiting"], ["pinned", "Pinned rows"], ["live", "Open WebSockets"], ["tunnel", "Not decrypted"], ["plain", "Plain HTTP"]] },
];

/** Split into raw words, keeping quoted runs and whitespace (so highlighting can reproduce the input exactly). */
function words(q: string): { text: string; ws: boolean }[] {
  const out: { text: string; ws: boolean }[] = [];
  let i = 0;
  while (i < q.length) {
    if (/\s/.test(q[i])) {
      let j = i;
      while (j < q.length && /\s/.test(q[j])) j++;
      out.push({ text: q.slice(i, j), ws: true });
      i = j;
      continue;
    }
    let j = i;
    let inQ = false;
    while (j < q.length && (inQ || !/\s/.test(q[j]))) {
      if (q[j] === '"') inQ = !inQ;
      j++;
    }
    out.push({ text: q.slice(i, j), ws: false });
    i = j;
  }
  return out;
}

const TERM_RE = /^([-!]?)([a-zA-Z]+)(>=|<=|:|=|>|<)(.*)$/s;

function unquote(v: string): string {
  return v.length >= 2 && v.startsWith('"') && v.endsWith('"') ? v.slice(1, -1) : v.replace(/^"/, "");
}

export function parse(q: string): Term[] {
  const terms: Term[] = [];
  for (const w of words(q)) {
    if (w.ws) continue;
    const m = TERM_RE.exec(w.text);
    if (m && KEYS[m[2].toLowerCase()]) {
      let op = m[3];
      let value = m[4];
      // status:>=400, size:>10kb
      const cmp = /^(>=|<=|>|<)(.*)$/.exec(value);
      if (op === ":" && cmp) {
        op = cmp[1];
        value = cmp[2];
      }
      terms.push({ neg: m[1] !== "", key: KEYS[m[2].toLowerCase()], op, value: unquote(value) });
    } else {
      const neg = /^[-!]./.test(w.text);
      terms.push({ neg, key: null, op: ":", value: unquote(neg ? w.text.slice(1) : w.text) });
    }
  }
  return terms.filter((t) => t.value !== "" || t.key === null);
}

/** Syntax-highlighting spans for the query field. Concatenated, they reproduce `q` exactly. */
export function highlight(q: string): Span[] {
  const out: Span[] = [];
  for (const w of words(q)) {
    if (w.ws) {
      out.push({ text: w.text, kind: "plain" });
      continue;
    }
    const m = TERM_RE.exec(w.text);
    if (m) {
      const known = !!KEYS[m[2].toLowerCase()];
      if (m[1]) out.push({ text: m[1], kind: "neg" });
      out.push({ text: m[2], kind: known ? "key" : "bad" });
      out.push({ text: m[3], kind: "op" });
      const v = m[4];
      if (v.startsWith('"')) out.push({ text: v, kind: "str" });
      else if (v.includes("*")) out.push({ text: v, kind: "glob" });
      else if (v) out.push({ text: v, kind: "val" });
    } else if (/^[-!]./.test(w.text)) {
      out.push({ text: w.text[0], kind: "neg" });
      out.push({ text: w.text.slice(1), kind: w.text[1] === '"' ? "str" : "plain" });
    } else {
      out.push({ text: w.text, kind: w.text.startsWith('"') ? "str" : "plain" });
    }
  }
  return out;
}

const globCache = new Map<string, RegExp>();
function globRe(pat: string): RegExp {
  let re = globCache.get(pat);
  if (!re) {
    const esc = pat.toLowerCase().replace(/[.+?^${}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*");
    // *.example.com also matches example.com
    const src = pat.startsWith("*.") ? `^(?:.*\\.)?${esc.slice(4)}$` : `^${esc}$`;
    re = new RegExp(src);
    globCache.set(pat, re);
  }
  return re;
}

/** Glob when the pattern has `*`, otherwise case-insensitive substring. */
function textMatch(pat: string, text: string, op: string): boolean {
  const t = text.toLowerCase();
  if (pat.includes("*")) return globRe(pat).test(t);
  if (op === "=") return t === pat.toLowerCase();
  return t.includes(pat.toLowerCase());
}

const SIZE_UNITS: Record<string, number> = { b: 1, k: 1024, kb: 1024, m: 1024 ** 2, mb: 1024 ** 2, g: 1024 ** 3, gb: 1024 ** 3 };
function parseSize(v: string): number | null {
  const m = /^([\d.]+)\s*([a-z]*)$/i.exec(v.trim());
  if (!m) return null;
  const u = SIZE_UNITS[m[2].toLowerCase() || "b"];
  return u ? parseFloat(m[1]) * u : null;
}
function parseDur(v: string): number | null {
  const m = /^([\d.]+)\s*(ms|s|m)?$/i.exec(v.trim());
  if (!m) return null;
  const k = { ms: 1, s: 1000, m: 60_000 }[(m[2] || "ms").toLowerCase() as "ms" | "s" | "m"];
  return parseFloat(m[1]) * k;
}
function cmp(op: string, a: number, b: number): boolean {
  switch (op) {
    case ">": return a > b;
    case "<": return a < b;
    case ">=": return a >= b;
    case "<=": return a <= b;
    default: return a === b;
  }
}

const TYPE_ALIAS: Record<string, string> = { image: "img", images: "img", graphql: "gql", websocket: "ws", font: "font", fonts: "font" };

export interface MatchCtx {
  pinned: Set<number>;
  /** needle → ids whose bodies contain it (filled asynchronously by the backend). */
  bodyHits: Map<string, Set<number>>;
}

function statusMatch(v: string, op: string, f: FlowSummary): boolean {
  const s = f.status;
  const lv = v.toLowerCase();
  if (lv === "pending") return f.state === "pending" && s == null;
  if (lv === "error" || lv === "failed") return f.state === "error";
  if (s == null) return false;
  if (op !== ":" && op !== "=") return cmp(op, s, parseInt(v, 10));
  return v.split(",").some((part) => {
    const p = part.trim().toLowerCase();
    const range = /^(\d{3})-(\d{3})$/.exec(p);
    if (range) return s >= +range[1] && s <= +range[2];
    const cls = /^([1-5])(xx)?$/.exec(p);
    if (cls) return Math.floor(s / 100) === +cls[1];
    return String(s).startsWith(p);
  });
}

function termMatch(t: Term, f: FlowSummary, ctx: MatchCtx, fullUrl: string): boolean {
  const v = t.value;
  switch (t.key) {
    case null: return fullUrl.toLowerCase().includes(v.toLowerCase());
    case "status": return statusMatch(v, t.op, f);
    case "host": return v.split(",").some((p) => textMatch(p.trim(), f.host, t.op));
    case "method": return v.split(",").some((p) => f.method.toLowerCase() === p.trim().toLowerCase());
    case "path": return textMatch(v, f.path, t.op);
    case "url": return textMatch(v, fullUrl, t.op);
    case "client": return textMatch(v, f.client ?? "", t.op);
    case "scheme": return f.scheme === v.toLowerCase();
    case "id": return t.op === ":" || t.op === "=" ? String(f.id) === v : cmp(t.op, f.id, +v);
    case "type":
      return v.split(",").some((p) => {
        const want = TYPE_ALIAS[p.trim().toLowerCase()] ?? p.trim().toLowerCase();
        return f.kind.toLowerCase() === want || (want === "xhr" && f.xhr);
      });
    case "size": {
      const n = parseSize(v);
      return n != null && cmp(t.op === ":" ? ">=" : t.op, f.respSize, n);
    }
    case "dur": {
      const n = parseDur(v);
      return n != null && f.durationMs != null && cmp(t.op === ":" ? ">=" : t.op, f.durationMs, n);
    }
    case "body": return ctx.bodyHits.get(v.toLowerCase())?.has(f.id) ?? false;
    case "is":
      switch (v.toLowerCase()) {
        case "error": return f.state === "error" || (f.status ?? 0) >= 500;
        case "pending": return f.state === "pending";
        case "pinned": return ctx.pinned.has(f.id);
        case "live": return f.state === "live";
        case "tunnel": return f.state === "tunnel";
        case "plain": return !f.tls && f.scheme === "http";
        case "tls": return f.tls;
        case "xhr": return f.xhr;
        default: return false;
      }
  }
  return true;
}

export function matches(terms: Term[], f: FlowSummary, ctx: MatchCtx, fullUrl: string): boolean {
  for (const t of terms) {
    if (termMatch(t, f, ctx, fullUrl) === t.neg) return false;
  }
  return true;
}

export function bodyNeedles(terms: Term[]): string[] {
  return [...new Set(terms.filter((t) => t.key === "body" && t.value).map((t) => t.value.toLowerCase()))];
}

/** Autocomplete context: what the word under the caret is. */
export function completionContext(q: string, caret: number): { start: number; end: number; key: string | null; prefix: string } {
  let start = caret;
  while (start > 0 && !/\s/.test(q[start - 1])) start--;
  let end = caret;
  while (end < q.length && !/\s/.test(q[end])) end++;
  const word = q.slice(start, caret).replace(/^[-!]/, "");
  const m = /^([a-zA-Z]+):(.*)$/.exec(word);
  if (m && KEYS[m[1].toLowerCase()]) return { start, end, key: KEYS[m[1].toLowerCase()], prefix: m[2] };
  return { start, end, key: null, prefix: word };
}
