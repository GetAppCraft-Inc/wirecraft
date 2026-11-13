// "Copy as …" code generators for a captured request.
import type { FlowDetail } from "./api";
import { url } from "./format";

export type CopyFormat = "curl" | "fetch" | "python" | "go" | "reqwest" | "httpie";
export const COPY_FORMATS: [CopyFormat, string][] = [
  ["curl", "cURL"],
  ["fetch", "fetch (JavaScript)"],
  ["python", "Python requests"],
  ["go", "Go net/http"],
  ["reqwest", "Rust reqwest"],
  ["httpie", "HTTPie"],
];

// Headers that the client library or transport sets on its own.
const SKIP = new Set(["host", "content-length", "connection", "accept-encoding", "transfer-encoding", "proxy-connection", "keep-alive"]);

function headers(d: FlowDetail): [string, string][] {
  return d.reqHeaders.filter(([k]) => !k.startsWith(":") && !SKIP.has(k.toLowerCase()));
}

function body(d: FlowDetail): string | null {
  return d.reqBody.text && d.reqBody.text.length ? d.reqBody.text : null;
}

const sq = (s: string) => `'${s.replace(/'/g, `'\\''`)}'`;
const js = (s: string) => JSON.stringify(s);
const py = (s: string) => JSON.stringify(s);
const goq = (s: string) => (s.includes("`") ? JSON.stringify(s) : "`" + s + "`");

export function copyAs(fmt: CopyFormat, d: FlowDetail): string {
  const u = url(d.summary);
  const m = d.summary.method;
  const h = headers(d);
  const b = body(d);
  switch (fmt) {
    case "curl": {
      const parts = [`curl ${sq(u)}`];
      if (m !== "GET" || b) parts.push(`-X ${m}`);
      for (const [k, v] of h) parts.push(`-H ${sq(`${k}: ${v}`)}`);
      if (b) parts.push(`--data-raw ${sq(b)}`);
      parts.push("--compressed");
      return parts.join(" \\\n  ");
    }
    case "fetch": {
      const opts: string[] = [`  method: ${js(m)}`];
      if (h.length) opts.push(`  headers: {\n${h.map(([k, v]) => `    ${js(k)}: ${js(v)}`).join(",\n")}\n  }`);
      if (b) opts.push(`  body: ${js(b)}`);
      return `await fetch(${js(u)}, {\n${opts.join(",\n")}\n});`;
    }
    case "python": {
      const lines = ["import requests", ""];
      lines.push(`headers = {\n${h.map(([k, v]) => `    ${py(k)}: ${py(v)},`).join("\n")}\n}`);
      if (b) lines.push(`data = ${py(b)}`);
      lines.push("", `r = requests.request(${py(m)}, ${py(u)}, headers=headers${b ? ", data=data.encode()" : ""})`, "print(r.status_code, r.text)");
      return lines.join("\n");
    }
    case "go": {
      const lines = [
        "package main",
        "",
        'import (\n\t"fmt"\n\t"io"\n\t"net/http"\n\t"strings"\n)',
        "",
        "func main() {",
        `\tbody := strings.NewReader(${goq(b ?? "")})`,
        `\treq, err := http.NewRequest(${JSON.stringify(m)}, ${JSON.stringify(u)}, body)`,
        "\tif err != nil {\n\t\tpanic(err)\n\t}",
        ...h.map(([k, v]) => `\treq.Header.Set(${JSON.stringify(k)}, ${JSON.stringify(v)})`),
        "\tresp, err := http.DefaultClient.Do(req)",
        "\tif err != nil {\n\t\tpanic(err)\n\t}",
        "\tdefer resp.Body.Close()",
        "\tout, _ := io.ReadAll(resp.Body)",
        "\tfmt.Println(resp.Status, string(out))",
        "}",
      ];
      return lines.join("\n");
    }
    case "reqwest": {
      const lines = [
        "let client = reqwest::Client::new();",
        `let resp = client`,
        `    .request(reqwest::Method::from_bytes(b${JSON.stringify(m)})?, ${JSON.stringify(u)})`,
        ...h.map(([k, v]) => `    .header(${JSON.stringify(k)}, ${JSON.stringify(v)})`),
      ];
      if (b) lines.push(`    .body(${JSON.stringify(b)})`);
      lines.push("    .send()", "    .await?;", 'println!("{} {}", resp.status(), resp.text().await?);');
      return lines.join("\n");
    }
    case "httpie": {
      const parts = [`http ${m} ${sq(u)}`];
      for (const [k, v] of h) parts.push(sq(`${k}:${v}`));
      const cmd = parts.join(" \\\n  ");
      return b ? `printf %s ${sq(b)} | ${cmd}` : cmd;
    }
  }
}
