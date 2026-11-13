<script lang="ts">
  import type { BodyView } from "../lib/api";
  import { fmtBytes } from "../lib/format";
  import { copyText } from "../lib/actions";
  import { api, errMsg } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import Icon from "./Icon.svelte";
  import JsonTree from "./JsonTree.svelte";
  import ProtobufView from "./ProtobufView.svelte";
  import { untrack } from "svelte";

  let { body, contentType, flowId, response }: { body: BodyView; contentType: string | null; flowId: number; response: boolean } = $props();

  type Mode = "pretty" | "raw" | "tree" | "hex" | "preview" | "proto";
  let mode = $state<Mode>("pretty");
  let wrap = $state(false);
  let searching = $state(false);
  let needle = $state("");
  let showAll = $state(false);
  const LINE_CAP = 3000;
  const HEX_CAP = 64 * 1024;

  const ct = $derived((contentType ?? "").toLowerCase());
  const json = $derived.by((): { ok: true; value: unknown } | { ok: false } => {
    if (body.text == null || body.clipped) return { ok: false };
    const t = body.text.trim();
    if (!(t.startsWith("{") || t.startsWith("[")) && !ct.includes("json")) return { ok: false };
    try {
      return { ok: true, value: JSON.parse(t) };
    } catch {
      return { ok: false };
    }
  });
  const isImage = $derived(ct.startsWith("image/"));
  const isHtml = $derived(ct.includes("html"));
  const label = $derived(
    body.grpc ? (body.grpc.kind === "protobuf" ? "Protobuf" : "gRPC") : json.ok ? "JSON" : isImage ? ct.replace("image/", "").toUpperCase().split(";")[0] : isHtml ? "HTML" : ct.includes("xml") ? "XML" : ct.includes("javascript") ? "JS" : ct.includes("css") ? "CSS" : ct.includes("x-www-form-urlencoded") ? "Form" : body.text != null ? "Text" : "Binary",
  );

  // pick a sensible default whenever a different body is shown
  $effect(() => {
    void flowId;
    untrack(() => {
      const img = isImage, html = isHtml, text = body.text != null;
      mode = body.grpc ? "proto" : img ? "preview" : !text ? "hex" : mode === "hex" || (mode === "preview" && !html) ? "pretty" : mode;
      showAll = false;
    });
  });

  const bytes = $derived.by((): Uint8Array => {
    if (body.base64 != null) {
      const bin = atob(body.base64);
      const out = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
      return out;
    }
    return new TextEncoder().encode(body.text ?? "");
  });

  const prettyText = $derived.by(() => {
    if (body.text == null) return "";
    if (mode === "pretty") {
      if (json.ok) return JSON.stringify(json.value, null, 2);
      if (ct.includes("x-www-form-urlencoded"))
        return body.text.split("&").map((p) => {
          try {
            return decodeURIComponent(p.replace(/\+/g, " ")).replace("=", " = ");
          } catch {
            return p;
          }
        }).join("\n");
    }
    return body.text;
  });
  const lines = $derived(prettyText.split("\n"));
  const shownLines = $derived(showAll ? lines : lines.slice(0, LINE_CAP));
  const hits = $derived.by(() => {
    if (!needle) return new Set<number>();
    const n = needle.toLowerCase();
    const s = new Set<number>();
    lines.forEach((l, i) => l.toLowerCase().includes(n) && s.add(i));
    return s;
  });

  const TOK = /("(?:[^"\\]|\\.)*")(\s*:)?|(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)|\b(true|false|null)\b|([{}[\],:])|([^"\d{}[\],:]+|.)/g;
  function jsonSpans(line: string): [string, string][] {
    const out: [string, string][] = [];
    for (const m of line.matchAll(TOK)) {
      if (m[1] != null) {
        out.push([m[1], m[2] ? "var(--syntax-key)" : "var(--syntax-string)"]);
        if (m[2]) out.push([m[2], "var(--syntax-punct)"]);
      } else if (m[3]) out.push([m[3], "var(--syntax-number)"]);
      else if (m[4]) out.push([m[4], "var(--syntax-keyword)"]);
      else if (m[5]) out.push([m[5], "var(--syntax-punct)"]);
      else out.push([m[6] ?? m[0], "var(--text-primary)"]);
    }
    return out;
  }

  function hexRows(): { off: string; hex: string; ascii: string }[] {
    const b = bytes.subarray(0, showAll ? bytes.length : HEX_CAP);
    const rows = [];
    for (let o = 0; o < b.length; o += 16) {
      const chunk = b.subarray(o, o + 16);
      const h = Array.from(chunk, (c) => c.toString(16).padStart(2, "0"));
      rows.push({
        off: o.toString(16).padStart(8, "0"),
        hex: (h.slice(0, 8).join(" ") + "  " + h.slice(8).join(" ")).padEnd(49),
        ascii: Array.from(chunk, (c) => (c >= 32 && c < 127 ? String.fromCharCode(c) : ".")).join(""),
      });
    }
    return rows;
  }

  const imgSrc = $derived(isImage ? `data:${ct.split(";")[0]};base64,${body.base64 ?? btoa(unescape(encodeURIComponent(body.text ?? "")))}` : "");

  async function saveBody() {
    const path = await save({ defaultPath: response ? "response-body" : "request-body" });
    if (!path) return;
    try {
      await api.saveBody(flowId, response, path);
      app.toast({ kind: "success", title: "Saved body", body: path }, 2500);
    } catch (e) {
      app.toast({ kind: "error", title: "Save failed", body: errMsg(e) });
    }
  }
</script>

<div class="bv">
  <div class="bt">
    <div class="seg sm">
      {#each [...(body.grpc ? [["proto", "Protobuf"]] : []), ["pretty", "Pretty"], ["raw", "Raw"], ["tree", "Tree"], ["hex", "Hex"], ["preview", "Preview"]] as [k, l]}
        <button class:on={mode === k} disabled={(k === "tree" && !json.ok) || (k === "preview" && !isImage && !isHtml) || ((k === "pretty" || k === "raw") && body.text == null)}
          onclick={() => (mode = k as Mode)}>{l}</button>
      {/each}
    </div>
    <span class="pill" style="background:var(--bg-hover);color:var(--text-secondary);font-size:10.5px">{label}</span>
    <span class="sz mono">{fmtBytes(body.decodedSize)}{body.decodedSize !== body.size ? ` (${fmtBytes(body.size)} on wire)` : ""}</span>
    <span class="spacer"></span>
    {#if searching}
      <label class="input" style="height:22px;width:150px">
        <!-- svelte-ignore a11y_autofocus -->
        <input autofocus placeholder="Find in body" bind:value={needle} onkeydown={(e) => e.key === "Escape" && ((searching = false), (needle = ""))} />
        <span class="sz mono">{needle ? hits.size : ""}</span>
      </label>
    {/if}
    <button class="icon-btn" style="width:22px;height:22px" class:active={searching} title="Find" onclick={() => { searching = !searching; if (!searching) needle = ""; }}><Icon name="search" /></button>
    <button class="icon-btn" style="width:22px;height:22px" class:active={wrap} title="Wrap lines" onclick={() => (wrap = !wrap)}><Icon name="wrap" /></button>
    <button class="icon-btn" style="width:22px;height:22px" title="Copy body" disabled={body.text == null} onclick={() => copyText(prettyText, "Copied body")}><Icon name="copy" /></button>
    <button class="icon-btn" style="width:22px;height:22px" title="Save body as…" onclick={saveBody}><Icon name="download" /></button>
  </div>

  {#if body.truncated || body.clipped || body.decodeError}
    <div class="note">
      <Icon name="info" size={12} />
      {#if body.decodeError}Couldn't decode ({body.decodeError}); showing raw bytes.
      {:else if body.truncated}Body larger than the capture limit — only the first {fmtBytes(body.size)} were kept.
      {:else}Showing the first 4 MB. Use Save to get the whole body.{/if}
    </div>
  {/if}

  <div class="wrapbox">
    {#if body.decodedSize === 0}
      <div class="none">No body</div>
    {:else if mode === "proto" && body.grpc}
      <div class="code tree"><ProtobufView data={body.grpc} /></div>
    {:else if mode === "tree" && json.ok}
      <div class="code tree" role="tree"><JsonTree value={json.value} open /></div>
    {:else if mode === "hex"}
      <div class="code hex mono selectable">
        <div class="hl t3">Offset    00 01 02 03 04 05 06 07  08 09 0a 0b 0c 0d 0e 0f   ASCII</div>
        {#each hexRows() as r}
          <div class="hl"><span class="t3">{r.off}  </span>{r.hex}   <span class="t2">{r.ascii}</span></div>
        {/each}
        {#if !showAll && bytes.length > HEX_CAP}
          <button class="btn sm secondary more" onclick={() => (showAll = true)}>Show all {fmtBytes(bytes.length)}</button>
        {/if}
      </div>
    {:else if mode === "preview" && isImage}
      <div class="code preview"><img src={imgSrc} alt="response" /></div>
    {:else if mode === "preview" && isHtml}
      <iframe class="code frame" sandbox="" srcdoc={body.text ?? ""} title="HTML preview"></iframe>
    {:else}
      <div class="code lines mono selectable" class:wrap>
        {#each shownLines as ln, i}
          <div class="ln" class:hit={hits.has(i)}>
            <span class="g">{i + 1}</span>
            <span class="t">{#if json.ok && mode === "pretty"}{#each jsonSpans(ln) as [t, c]}<span style:color={c}>{t}</span>{/each}{:else}{ln}{/if}</span>
          </div>
        {/each}
        {#if !showAll && lines.length > LINE_CAP}
          <button class="btn sm secondary more" onclick={() => (showAll = true)}>Show all {lines.length.toLocaleString()} lines</button>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .bv {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }
  .bt {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    flex: none;
  }
  .seg button:disabled {
    opacity: 0.4;
  }
  .sz {
    font-size: 10.5px;
    color: var(--text-tertiary);
    white-space: nowrap;
  }
  .note {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 8px 6px;
    padding: 4px 8px;
    border-radius: 5px;
    background: var(--info-soft);
    color: var(--text-secondary);
    font-size: 11.5px;
  }
  .wrapbox {
    flex: 1;
    min-height: 0;
    padding: 0 8px 8px;
    display: flex;
  }
  .code {
    flex: 1;
    min-width: 0;
    overflow: auto;
    background: var(--bg-sunken);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    padding: 8px 0;
  }
  .none {
    flex: 1;
    display: grid;
    place-items: center;
    color: var(--text-tertiary);
    font-size: 12px;
  }
  .ln {
    display: flex;
    min-height: 17px;
    font-size: 11.5px;
    line-height: 17px;
    padding-right: 10px;
  }
  .ln.hit {
    background: var(--accent-soft);
  }
  .g {
    width: 38px;
    flex: none;
    text-align: right;
    padding-right: 10px;
    color: var(--text-tertiary);
    font-size: 10.5px;
    user-select: none;
  }
  .t {
    white-space: pre;
  }
  .wrap .t {
    white-space: pre-wrap;
    word-break: break-all;
  }
  .hex {
    padding: 8px 10px;
  }
  .hl {
    font-size: 11px;
    line-height: 18px;
    white-space: pre;
  }
  .t2 {
    color: var(--text-secondary);
  }
  .t3 {
    color: var(--text-tertiary);
  }
  .more {
    margin: 8px 10px;
  }
  .preview {
    display: grid;
    place-items: center;
    padding: 12px;
    background-color: var(--bg-sunken);
    background-image: linear-gradient(45deg, var(--bg-hover) 25%, transparent 25%, transparent 75%, var(--bg-hover) 75%),
      linear-gradient(45deg, var(--bg-hover) 25%, transparent 25%, transparent 75%, var(--bg-hover) 75%);
    background-size: 16px 16px;
    background-position: 0 0, 8px 8px;
  }
  .preview img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }
  .frame {
    background: #fff;
    padding: 0;
  }
</style>
