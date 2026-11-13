import type { FlowSummary, Headers } from "./api";

export function fmtBytes(n: number | null | undefined): string {
  if (n == null) return "—";
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(n < 10 * 1024 ? 1 : 0)} KB`;
  if (n < 1024 ** 3) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 ** 3).toFixed(2)} GB`;
}

export function fmtDuration(ms: number | null | undefined): string {
  if (ms == null) return "—";
  if (ms < 1000) return `${Math.round(ms)} ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(2)} s`;
  return `${Math.floor(ms / 60_000)}m ${Math.round((ms % 60_000) / 1000)}s`;
}

export function fmtTime(epochMs: number): string {
  const d = new Date(epochMs);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

export function fmtDate(epochSecs: number): string {
  return new Date(epochSecs * 1000).toLocaleDateString("en-GB", { day: "2-digit", month: "short", year: "numeric" });
}

export function fmtCount(n: number): string {
  if (n >= 10_000) return `${(n / 1000).toFixed(0)}k`;
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
  return String(n);
}

export function fmtInt(n: number): string {
  return n.toLocaleString("en-US");
}

export function url(f: FlowSummary): string {
  if (f.method === "CONNECT") return `${f.host}:${f.port}`;
  const def = f.scheme === "https" ? 443 : 80;
  const auth = f.port === def ? f.host : `${f.host}:${f.port}`;
  return `${f.scheme}://${auth}${f.path}`;
}

export function header(h: Headers, name: string): string | undefined {
  const n = name.toLowerCase();
  return h.find(([k]) => k.toLowerCase() === n)?.[1];
}

export const METHOD_COLOR: Record<string, string> = {
  GET: "var(--method-get)",
  POST: "var(--method-post)",
  PUT: "var(--method-put)",
  PATCH: "var(--method-patch)",
  DELETE: "var(--method-delete)",
};

export function methodColor(m: string): string {
  return METHOD_COLOR[m] ?? "var(--method-options)";
}

export function statusColor(code: number | null | undefined): string {
  if (code == null) return "var(--text-tertiary)";
  const c = Math.floor(code / 100);
  return c >= 1 && c <= 5 ? `var(--status-${c}xx)` : "var(--text-tertiary)";
}

export const STATUS_TEXT: Record<number, string> = {
  100: "Continue", 101: "Switching Protocols", 200: "OK", 201: "Created", 202: "Accepted", 204: "No Content",
  206: "Partial Content", 301: "Moved Permanently", 302: "Found", 303: "See Other", 304: "Not Modified",
  307: "Temporary Redirect", 308: "Permanent Redirect", 400: "Bad Request", 401: "Unauthorized", 403: "Forbidden",
  404: "Not Found", 405: "Method Not Allowed", 408: "Request Timeout", 409: "Conflict", 410: "Gone",
  413: "Payload Too Large", 415: "Unsupported Media Type", 418: "I'm a teapot", 422: "Unprocessable Entity",
  429: "Too Many Requests", 500: "Internal Server Error", 501: "Not Implemented", 502: "Bad Gateway",
  503: "Service Unavailable", 504: "Gateway Timeout",
};

export function isError(f: FlowSummary): boolean {
  return f.state === "error" || (f.status != null && f.status >= 500);
}

/** Path with the query string split off. */
export function splitQuery(path: string): [string, [string, string][]] {
  const i = path.indexOf("?");
  if (i < 0) return [path, []];
  const q = path.slice(i + 1);
  const params: [string, string][] = q
    .split("&")
    .filter(Boolean)
    .map((p) => {
      const j = p.indexOf("=");
      const dec = (s: string) => {
        try {
          return decodeURIComponent(s.replace(/\+/g, " "));
        } catch {
          return s;
        }
      };
      return j < 0 ? [dec(p), ""] : [dec(p.slice(0, j)), dec(p.slice(j + 1))];
    });
  return [path.slice(0, i), params];
}

export function cookies(h: Headers, response: boolean): { name: string; value: string; attrs: string }[] {
  const out: { name: string; value: string; attrs: string }[] = [];
  for (const [k, v] of h) {
    const lk = k.toLowerCase();
    if (!response && lk === "cookie") {
      for (const c of v.split(";")) {
        const j = c.indexOf("=");
        if (j > 0) out.push({ name: c.slice(0, j).trim(), value: c.slice(j + 1).trim(), attrs: "" });
      }
    } else if (response && lk === "set-cookie") {
      const [first, ...rest] = v.split(";");
      const j = first.indexOf("=");
      if (j > 0) out.push({ name: first.slice(0, j).trim(), value: first.slice(j + 1).trim(), attrs: rest.map((s) => s.trim()).join("; ") });
    }
  }
  return out;
}
