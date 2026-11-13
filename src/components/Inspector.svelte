<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { cookies, fmtBytes, fmtDuration, header, methodColor, splitQuery, STATUS_TEXT, statusColor, url } from "../lib/format";
  import { copyFlowAs, copyText, rowMenu } from "../lib/actions";
  import type { FlowDetail, Headers } from "../lib/api";
  import Icon from "./Icon.svelte";
  import BodyViewer from "./BodyViewer.svelte";
  import Menu, { type MenuItem } from "./Menu.svelte";
  import PausedEditor from "./PausedEditor.svelte";
  import WsMessages from "./WsMessages.svelte";
  import { tools } from "../lib/tools.svelte";

  let { layout = "right" }: { layout?: "right" | "bottom" } = $props();

  type ReqTab = "headers" | "query" | "cookies" | "body" | "auth" | "timing";
  type RespTab = "headers" | "cookies" | "body" | "messages" | "timing" | "tls";
  let reqTab = $state<ReqTab>("headers");
  let respTab = $state<RespTab>("body");
  let reqH = $state(248);
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  const d = $derived(app.detail);
  const s = $derived(d?.summary ?? (app.selected != null ? app.byId.get(app.selected) : undefined));
  const query = $derived(s ? splitQuery(s.path)[1] : []);
  const reqCookies = $derived(d ? cookies(d.reqHeaders, false) : []);
  const respCookies = $derived(d ? cookies(d.respHeaders, true) : []);
  const paused = $derived(s ? tools.paused.find((p) => p.flowId === s.id) : undefined);
  const isWs = $derived(!!s && (s.kind === "WS" || s.wsMessages > 0));
  $effect(() => {
    // WebSocket flows open on their messages; others never show the tab
    if (isWs && respTab === "body") respTab = "messages";
    else if (!isWs && respTab === "messages") respTab = "body";
  });
  const statusText = $derived(d?.statusText || (s?.status ? STATUS_TEXT[s.status] : "") || "");

  function auth(dd: FlowDetail): { scheme: string; rows: [string, string][]; jwt?: { header: string; payload: string } } | null {
    const v = header(dd.reqHeaders, "authorization") ?? header(dd.reqHeaders, "proxy-authorization");
    if (!v) return null;
    const [scheme, ...rest] = v.split(" ");
    const cred = rest.join(" ");
    if (scheme.toLowerCase() === "basic") {
      try {
        const dec = atob(cred);
        const i = dec.indexOf(":");
        return { scheme: "Basic", rows: [["Username", dec.slice(0, i)], ["Password", dec.slice(i + 1)]] };
      } catch {
        return { scheme: "Basic", rows: [["Credentials", cred]] };
      }
    }
    const parts = cred.split(".");
    if (parts.length === 3) {
      const dec = (p: string) => {
        try {
          return JSON.stringify(JSON.parse(atob(p.replace(/-/g, "+").replace(/_/g, "/"))), null, 2);
        } catch {
          return p;
        }
      };
      return { scheme, rows: [["Token", cred]], jwt: { header: dec(parts[0]), payload: dec(parts[1]) } };
    }
    return { scheme, rows: [["Credentials", cred]] };
  }

  function startResize(e: PointerEvent) {
    const start = e.clientY, h0 = reqH;
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => (reqH = Math.max(90, Math.min(window.innerHeight - 300, h0 + ev.clientY - start)));
    const up = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
  }

  function headerText(h: Headers) {
    return h.map(([k, v]) => `${k}: ${v}`).join("\n");
  }
</script>

{#snippet kv(rows: [string, string][], keyW = 128)}
  {#if !rows.length}
    <div class="none">None</div>
  {:else}
    <div class="kv selectable">
      {#each rows as [k, v]}
        <div class="kvr">
          <span class="k mono trunc" style:width="{keyW}px" title={k}>{k}</span>
          <span class="v mono">{v}</span>
        </div>
      {/each}
    </div>
  {/if}
{/snippet}

{#snippet timing(dd: FlowDetail)}
  {@const t = dd.timing}
  {@const segs = [["Request", t.requestMs, "var(--info-default)"], ["Waiting (TTFB)", t.waitingMs, "var(--warning-default)"], ["Download", t.downloadMs, "var(--success-default)"]] as [string, number | null, string][]}
  {@const total = t.totalMs ?? dd.summary.durationMs ?? segs.reduce((a, [, ms]) => a + (ms ?? 0), 0)}
  <div class="timing">
    {#each segs as [name, ms, c], i}
      {@const off = segs.slice(0, i).reduce((a, [, m]) => a + (m ?? 0), 0)}
      <div class="trow">
        <span class="tn">{name}</span>
        <span class="tbar">
          {#if ms != null && total > 0}
            <span style:left="{(off / total) * 100}%" style:width="max(3px, {(ms / total) * 100}%)" style:background={c}></span>
          {/if}
        </span>
        <span class="tv mono">{ms == null ? "—" : fmtDuration(ms)}</span>
      </div>
    {/each}
    <div class="trow tot"><span class="tn">Total</span><span class="tbar" style="background:none"></span><span class="tv mono">{fmtDuration(total)}</span></div>
    <div class="kvr" style="margin-top:8px"><span class="k" style="width:110px">Started</span><span class="v mono">{new Date(t.started).toISOString().replace("T", " ").replace("Z", " UTC")}</span></div>
    <div class="kvr"><span class="k" style="width:110px">Client</span><span class="v mono">{dd.summary.client ?? "—"} · {dd.summary.clientAddr}</span></div>
  </div>
{/snippet}

<section class="inspector" class:bottom={layout === "bottom"}>
  {#if !s}
    <div class="empty">
      <div class="ei"><Icon name="eye" size={20} /></div>
      <div class="et">No request selected</div>
      <div class="eb">Select a row to inspect its headers, body and timing. Use <span class="mono">↑</span> <span class="mono">↓</span> to move.</div>
    </div>
  {:else}
    <div class="ih">
      <div class="r">
        <span class="pill mono" style:color={methodColor(s.method)} style:border="1px solid {methodColor(s.method)}" style="background:var(--bg-sunken);font-size:10.5px">{s.method}</span>
        <span class="url mono trunc selectable" title={url(s)}>{url(s)}</span>
        <button class="icon-btn" style="width:24px;height:24px" class:active={app.pinned.has(s.id)} title="Pin" onclick={() => app.togglePin(s.id)}><Icon name="pin" /></button>
        <button class="icon-btn" style="width:24px;height:24px" title="Repeat (R)" disabled={s.method === "CONNECT"} onclick={() => app.replay(s.id)}><Icon name="repeat" /></button>
        <button class="icon-btn" style="width:24px;height:24px" title="Edit and repeat (E)" disabled={s.method === "CONNECT"} onclick={() => tools.composeFrom(s.id)}><Icon name="send" /></button>
        <button class="icon-btn" style="width:24px;height:24px" title="Copy as cURL" disabled={s.method === "CONNECT"} onclick={() => copyFlowAs(s.id, "curl")}><Icon name="terminal" /></button>
        <button class="icon-btn" style="width:24px;height:24px" title="Copy URL" onclick={() => copyText(url(s), "Copied URL")}><Icon name="copy" /></button>
        <button class="icon-btn" style="width:24px;height:24px" title="More" onclick={(e) => (menu = { x: e.clientX - 220, y: e.clientY + 12, items: rowMenu(s) })}><Icon name="more" /></button>
      </div>
      <div class="r meta">
        {#if s.status != null}
          <span class="pill mono status" style:color={statusColor(s.status)}><span class="dot" style:background={statusColor(s.status)}></span>{s.status} {statusText}</span>
        {:else if s.state === "error"}
          <span class="pill status" style="color:var(--error-default)"><span class="dot" style="background:var(--error-default)"></span>Failed</span>
        {:else if s.state === "paused"}
          <span class="pill status" style="color:var(--warning-default)"><span class="dot" style="background:var(--warning-default)"></span>Paused</span>
        {:else if s.state === "tunnel"}
          <span class="pill status" style="color:var(--warning-default)"><span class="dot" style="background:var(--warning-default)"></span>Tunnelled · not decrypted</span>
        {:else}
          <span class="pill status" style="color:var(--info-default)"><span class="dot" style="background:var(--info-default)"></span>Pending</span>
        {/if}
        <span class="m"><Icon name="clock" size={12} color="var(--text-tertiary)" /><span class="mono">{s.state === "live" ? "live" : fmtDuration(s.durationMs)}</span></span>
        <span class="m" title="Request body"><Icon name="upload" size={12} color="var(--text-tertiary)" /><span class="mono">{fmtBytes(s.reqSize)}</span></span>
        <span class="m" title="Response body"><Icon name="download" size={12} color="var(--text-tertiary)" /><span class="mono">{fmtBytes(s.respSize)}</span></span>
        {#if d?.tls?.version}
          <span class="m"><Icon name="lock" size={12} color="var(--text-tertiary)" /><span class="mono">{d.tls.version.replace("TLSv", "TLS ").replace("_", ".")}{d.tls.alpn ? ` · ${d.tls.alpn}` : ""}</span></span>
        {:else}
          <span class="m"><span class="mono">{s.httpVersion}</span></span>
        {/if}
      </div>
      {#if s.applied?.length}
        <div class="r" style="gap:4px;flex-wrap:wrap">
          {#each s.applied as a}
            <span class="pill" style="background:var(--accent-soft);color:var(--accent-text);font-size:10.5px">{a}</span>
          {/each}
          {#if s.note && (s.applied.includes("Map Remote") || s.applied.includes("Map Local"))}<span class="mono trunc" style="font-size:11px;color:var(--text-secondary)">{s.note}</span>{/if}
        </div>
      {/if}
      {#if s.error}
        <div class="errbox"><Icon name="alert" size={12} /> <span class="selectable">{s.error}</span></div>
      {/if}
    </div>

    {#if paused}
      {#key paused.flowId + paused.phase}<PausedEditor p={paused} />{/key}
    {:else}
    <div class="panes">
      <div class="pane req" style:height={layout === "right" ? `${reqH}px` : undefined}>
        <div class="tabs">
          {#each [["headers", "Headers", d ? d.reqHeaders.length : s.reqHeaderCount], ["query", "Query", query.length], ["cookies", "Cookies", reqCookies.length], ["body", "Body", null], ["auth", "Auth", null], ["timing", "Timing", null]] as [k, l, n]}
            <button class:on={reqTab === k} onclick={() => (reqTab = k as ReqTab)}>{l}{#if n != null}<span class="n">{n}</span>{/if}</button>
          {/each}
          <span class="spacer"></span>
          <span class="side">REQUEST</span>
          <button class="icon-btn" style="width:22px;height:22px;align-self:center" title="Copy request headers" disabled={!d} onclick={() => d && copyText(headerText(d.reqHeaders), "Copied request headers")}><Icon name="copy" /></button>
        </div>
        <div class="content">
          {#if !d}
            <div class="none">{app.detailError ?? "Loading…"}</div>
          {:else if reqTab === "headers"}
            {@render kv(d.reqHeaders)}
          {:else if reqTab === "query"}
            {@render kv(query)}
          {:else if reqTab === "cookies"}
            {@render kv(reqCookies.map((c) => [c.name, c.value]))}
          {:else if reqTab === "body"}
            <BodyViewer body={d.reqBody} contentType={header(d.reqHeaders, "content-type") ?? null} flowId={s.id} response={false} />
          {:else if reqTab === "auth"}
            {@const a = auth(d)}
            {#if !a}
              <div class="none">No Authorization header</div>
            {:else}
              <div class="kvr"><span class="k" style="width:128px">Scheme</span><span class="v mono">{a.scheme}</span></div>
              {@render kv(a.rows)}
              {#if a.jwt}
                <div class="jwt">
                  <div class="jl">JWT header</div><pre class="mono selectable">{a.jwt.header}</pre>
                  <div class="jl">JWT payload</div><pre class="mono selectable">{a.jwt.payload}</pre>
                </div>
              {/if}
            {/if}
          {:else if reqTab === "timing"}
            {@render timing(d)}
          {/if}
        </div>
      </div>

      {#if layout === "right"}
        <div class="split-h" role="separator" aria-orientation="horizontal" onpointerdown={startResize}></div>
      {:else}
        <div class="split-v"></div>
      {/if}

      <div class="pane resp">
        <div class="tabs">
          {#each [["headers", "Headers", d ? d.respHeaders.length : s.respHeaderCount], ["cookies", "Cookies", respCookies.length], ...(isWs ? [["messages", "Messages", s.wsMessages]] : [["body", "Body", null]]), ["timing", "Timing", null], ["tls", "TLS", null]] as [k, l, n]}
            <button class:on={respTab === k} onclick={() => (respTab = k as RespTab)}>{l}{#if n != null}<span class="n">{n}</span>{/if}</button>
          {/each}
          <span class="spacer"></span>
          <span class="side">RESPONSE</span>
        </div>
        <div class="content">
          {#if !d}
            <div class="none">{app.detailError ?? "Loading…"}</div>
          {:else if respTab === "headers"}
            {@render kv(d.respHeaders)}
          {:else if respTab === "cookies"}
            {#if !respCookies.length}
              <div class="none">None</div>
            {:else}
              <div class="kv selectable">
                {#each respCookies as c}
                  <div class="kvr"><span class="k mono trunc" style="width:128px">{c.name}</span><span class="v mono">{c.value}{#if c.attrs}<span class="attrs"> · {c.attrs}</span>{/if}</span></div>
                {/each}
              </div>
            {/if}
          {:else if respTab === "body"}
            {#if s.state === "tunnel"}
              <div class="none">Tunnelled connections aren't decrypted, so there's no body to show.</div>
            {:else}
              <BodyViewer body={d.respBody} contentType={header(d.respHeaders, "content-type") ?? null} flowId={s.id} response />
            {/if}
          {:else if respTab === "messages"}
            <WsMessages id={s.id} count={s.wsMessages} />
          {:else if respTab === "timing"}
            {@render timing(d)}
          {:else if respTab === "tls"}
            {#if d.tls}
              {@render kv([["Protocol", d.tls.version ?? "—"], ["ALPN", d.tls.alpn ?? "—"], ["SNI", d.tls.sni ?? "—"], ["Cipher", d.tls.cipher ?? "—"], ["Decrypted", s.state === "tunnel" ? "No (tunnelled)" : "Yes — signed by the xwire root CA"]], 110)}
            {:else}
              <div class="none">Plain HTTP — no TLS on this connection.</div>
            {/if}
          {/if}
        </div>
      </div>
    </div>
    {/if}
  {/if}
</section>

{#if menu}
  <Menu items={menu.items} x={menu.x} y={menu.y} onclose={() => (menu = null)} />
{/if}

<style>
  .inspector {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-width: 0;
    background: var(--bg-panel);
    overflow: hidden;
  }
  .ih {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px;
    background: var(--bg-raised);
    border-bottom: 1px solid var(--border-subtle);
    flex: none;
  }
  .r {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .url {
    flex: 1;
    font-size: 12px;
  }
  .meta {
    gap: 10px;
  }
  .status {
    background: var(--bg-sunken);
    border: 1px solid var(--border-subtle);
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 700;
  }
  .m {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    color: var(--text-secondary);
    white-space: nowrap;
  }
  .errbox {
    display: flex;
    gap: 6px;
    align-items: flex-start;
    padding: 6px 8px;
    border-radius: 6px;
    background: var(--error-soft);
    color: var(--error-default);
    font-size: 11.5px;
    line-height: 1.4;
  }
  .panes {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .bottom .panes {
    flex-direction: row;
  }
  .pane {
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    flex: none;
  }
  .pane.resp,
  .bottom .pane {
    flex: 1;
  }
  .tabs .side {
    align-self: center;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.06em;
    color: var(--text-tertiary);
    margin-right: 4px;
  }
  .content {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
  }
  .split-h {
    height: 1px;
    background: var(--border-default);
    flex: none;
    position: relative;
    cursor: row-resize;
  }
  .split-h::after {
    content: "";
    position: absolute;
    inset: -3px 0;
  }
  .split-h:hover {
    background: var(--accent-default);
  }
  .split-v {
    width: 1px;
    background: var(--border-default);
    flex: none;
  }
  .kv {
    display: flex;
    flex-direction: column;
  }
  .kvr {
    display: flex;
    gap: 8px;
    min-height: 22px;
    padding: 3px 12px;
    border-bottom: 1px solid var(--border-subtle);
    align-items: baseline;
  }
  .kvr .k {
    flex: none;
    font-size: 11.5px;
    color: var(--text-secondary);
  }
  .kvr .v {
    flex: 1;
    min-width: 0;
    font-size: 11.5px;
    word-break: break-all;
    line-height: 16px;
  }
  .attrs {
    color: var(--text-tertiary);
  }
  .none {
    padding: 16px 12px;
    color: var(--text-tertiary);
    font-size: 12px;
  }
  .timing {
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .timing .kvr {
    padding: 3px 0;
  }
  .trow {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .tn {
    width: 100px;
    font-size: 11.5px;
    color: var(--text-secondary);
    flex: none;
  }
  .tbar {
    position: relative;
    flex: 1;
    height: 8px;
    border-radius: 2px;
    background: var(--bg-sunken);
  }
  .tbar span {
    position: absolute;
    top: 0;
    bottom: 0;
    border-radius: 2px;
  }
  .tv {
    width: 64px;
    text-align: right;
    font-size: 11px;
    flex: none;
  }
  .tot .tn,
  .tot .tv {
    font-weight: 700;
    color: var(--text-primary);
  }
  .jwt {
    padding: 8px 12px;
  }
  .jl {
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-tertiary);
    margin: 6px 0 4px;
  }
  .jwt pre {
    margin: 0;
    padding: 8px 10px;
    background: var(--bg-sunken);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    font-size: 11.5px;
    white-space: pre-wrap;
    word-break: break-all;
  }
  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 24px;
    text-align: center;
  }
  .ei {
    width: 44px;
    height: 44px;
    border-radius: 12px;
    background: var(--bg-raised);
    border: 1px solid var(--border-default);
    display: grid;
    place-items: center;
    color: var(--text-secondary);
  }
  .et {
    font-size: 14px;
    font-weight: 600;
  }
  .eb {
    max-width: 300px;
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--text-secondary);
  }
</style>
