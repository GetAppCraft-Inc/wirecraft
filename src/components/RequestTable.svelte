<script lang="ts">
  import { app, COLUMNS, type SortKey } from "../lib/state.svelte";
  import { fmtBytes, fmtDuration, fmtTime, isError, methodColor, statusColor } from "../lib/format";
  import type { FlowSummary } from "../lib/api";
  import Icon from "./Icon.svelte";
  import Kbd from "./Kbd.svelte";
  import Menu, { type MenuItem } from "./Menu.svelte";
  import { rowMenu } from "../lib/actions";

  let { compact = false }: { compact?: boolean } = $props();

  const ROW = 24;
  const COMPACT_COLS = new Set(["idx", "tls", "method", "status", "host", "path", "size", "dur"]);

  let scroller: HTMLDivElement;
  let scrollTop = $state(0);
  let height = $state(400);
  let follow = $state(true);
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  let colMenu = $state<{ x: number; y: number } | null>(null);

  const cols = $derived(COLUMNS.filter((c) => !app.hiddenCols.includes(c.key) && (!compact || COMPACT_COLS.has(c.key))));
  const grid = $derived(cols.map((c) => (c.width == null ? "minmax(60px, 1fr)" : `${c.width}px`)).join(" ") + " 22px");
  const rows = $derived(app.filtered);
  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 8));
  const last = $derived(Math.min(rows.length, Math.ceil((scrollTop + height) / ROW) + 8));
  const visible = $derived(rows.slice(first, last));
  const followable = $derived(app.sort.key === "idx" && !app.sort.desc);

  // keep pinned to the bottom while following new traffic
  $effect(() => {
    void rows.length;
    if (follow && followable && scroller) {
      queueMicrotask(() => {
        scroller.scrollTop = scroller.scrollHeight;
      });
      app.newSincePause = 0;
    }
  });

  function onscroll() {
    scrollTop = scroller.scrollTop;
    const atBottom = scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - ROW;
    if (atBottom !== follow) {
      follow = atBottom;
      if (atBottom) app.newSincePause = 0;
    }
  }

  function jumpToEnd() {
    follow = true;
    app.newSincePause = 0;
    if (app.livePaused) app.toggleLive();
    scroller.scrollTop = scroller.scrollHeight;
  }

  export function scrollToSelected() {
    if (app.selected == null || !scroller) return;
    const i = rows.findIndex((f) => f.id === app.selected);
    if (i < 0) return;
    const top = i * ROW;
    if (top < scroller.scrollTop) scroller.scrollTop = top;
    else if (top + ROW > scroller.scrollTop + scroller.clientHeight) scroller.scrollTop = top + ROW - scroller.clientHeight;
  }

  export function move(delta: number) {
    if (!rows.length) return;
    let i = app.selected == null ? -1 : rows.findIndex((f) => f.id === app.selected);
    i = i < 0 ? (delta > 0 ? 0 : rows.length - 1) : Math.max(0, Math.min(rows.length - 1, i + delta));
    app.select(rows[i].id);
    follow = false;
    queueMicrotask(scrollToSelected);
  }

  export function end() {
    jumpToEnd();
  }

  function sortBy(key: string) {
    if (key === "tls") return;
    const k = key as SortKey;
    app.sort = app.sort.key === k ? { key: k, desc: !app.sort.desc } : { key: k, desc: false };
  }

  function rowClick(e: MouseEvent, f: FlowSummary) {
    app.select(f.id, { toggle: e.ctrlKey || e.metaKey, range: e.shiftKey });
    follow = false;
  }

  function rowContext(e: MouseEvent, f: FlowSummary) {
    e.preventDefault();
    if (!app.multi.has(f.id)) app.select(f.id);
    menu = { x: e.clientX, y: e.clientY, items: rowMenu(f) };
  }

  function statusLabel(f: FlowSummary): string {
    if (f.status != null) return String(f.status);
    if (f.state === "error") return "ERR";
    if (f.state === "tunnel") return "—";
    return "";
  }

  function pathLabel(f: FlowSummary): string {
    if (f.method === "CONNECT") return `:${f.port}  ${f.note ? `(${f.note})` : ""}`;
    return f.path;
  }
</script>

<div class="table" style:--grid={grid}>
  <div class="head">
    {#each cols as c}
      <button class="hc" class:right={c.align === "right"} class:on={app.sort.key === c.key} onclick={() => sortBy(c.key)}>
        {#if c.key === "tls"}
          <Icon name="lock" size={11} color="var(--text-tertiary)" />
        {:else}
          <span>{c.title}</span>
          {#if app.sort.key === c.key}<Icon name="chevron-down" size={10} color="var(--accent-text)" weight={1.6} />{/if}
        {/if}
      </button>
    {/each}
    <button class="icon-btn" style="width:22px;height:22px" title="Columns" onclick={(e) => (colMenu = { x: e.clientX - 200, y: e.clientY + 10 })}><Icon name="columns" /></button>
  </div>

  <div class="scroll" bind:this={scroller} bind:clientHeight={height} {onscroll}>
    {#if !rows.length}
      <div class="empty">
        <div class="ei"><Icon name={app.total ? "filter" : "bolt"} size={20} /></div>
        {#if app.total}
          <div class="et">No requests match</div>
          <div class="eb">{app.total.toLocaleString()} captured requests are hidden by the current filter.</div>
          <button class="btn secondary" onclick={() => { app.query = ""; app.chip = "all"; app.source = null; }}>Clear filters</button>
        {:else}
          <div class="et">Waiting for traffic</div>
          <div class="eb">
            Point an app at <span class="mono">{app.proxy?.addr ?? "127.0.0.1:" + (app.settings?.port ?? 9090)}</span>, or try
            <span class="mono selectable">curl -x http://127.0.0.1:{app.settings?.port ?? 9090} https://example.com</span>
          </div>
          {#if app.proxy?.error}<div class="eb" style="color:var(--error-default)">{app.proxy.error}</div>{/if}
        {/if}
      </div>
    {/if}
    <div class="spacer-box" style:height="{rows.length * ROW}px">
      {#each visible as f, i (f.id)}
        {@const err = isError(f)}
        {@const sel = app.selected === f.id || app.multi.has(f.id)}
        {@const pinned = app.pinned.has(f.id)}
        <div
          class="tr"
          class:sel
          class:err={err && !sel}
          class:pinned={pinned && !sel && !err}
          style:top="{(first + i) * ROW}px"
          role="row"
          tabindex="-1"
          onmousedown={(e) => e.button === 0 && rowClick(e, f)}
          oncontextmenu={(e) => rowContext(e, f)}
          ondblclick={() => app.replay(f.id)}>
          {#each cols as c}
            {#if c.key === "idx"}
              <span class="td right mono t3">
                {#if f.state === "paused" || f.applied?.includes("Breakpoint")}<Icon name="breakpoint" size={11} color="var(--warning-default)" />{/if}
                {#if f.applied?.some((a) => a !== "Breakpoint")}<span title={f.applied.join(", ") + (f.applied.includes("Script error") && f.note ? ` — ${f.note}` : "")}><Icon name={f.applied.includes("Map Local") ? "map" : f.applied.includes("Map Remote") ? "remote" : f.applied.includes("Throttle") && f.applied.length === 1 ? "throttle" : f.applied.includes("Blocked") ? "x" : f.applied.some((a) => a.startsWith("Script")) ? "code" : "rewrite"} size={11} color={f.applied.includes("Script error") ? "var(--warning-default)" : "var(--accent-text)"} /></span>{/if}
                {#if pinned}<Icon name="pin" size={11} color="var(--accent-text)" />{/if}{f.id}
              </span>
            {:else if c.key === "tls"}
              <span class="td">
                {#if f.state === "tunnel"}
                  <Icon name="lock" size={11} color="var(--warning-default)" />
                {:else if f.tls}
                  <Icon name="lock" size={11} color="var(--text-tertiary)" />
                {:else}
                  <Icon name="unlock" size={11} color="var(--warning-default)" />
                {/if}
              </span>
            {:else if c.key === "method"}
              <span class="td mono b" style:color={methodColor(f.method)}>{f.method}</span>
            {:else if c.key === "status"}
              <span class="td mono">
                {#if f.state === "paused"}
                  <span class="b" style="color:var(--warning-default);font-size:9.5px">PAUSED</span>
                {:else if f.status == null && (f.state === "pending" || f.state === "live")}
                  <span class="dot" style="background:var(--info-default)"></span><span class="t3">···</span>
                {:else}
                  <span style:color={f.state === "error" && f.status == null ? "var(--error-default)" : statusColor(f.status)}
                    class:b={err || (f.status ?? 0) >= 400}>{statusLabel(f)}</span>
                {/if}
              </span>
            {:else if c.key === "host"}
              <span class="td mono trunc" class:red={err}>{f.host}</span>
            {:else if c.key === "path"}
              <span class="td mono path" class:red={err} class:t2={!err && !sel}>
                <span class="trunc">{pathLabel(f)}</span>
                {#if f.state === "live"}<span class="pill" style="background:var(--success-soft);color:var(--success-default);font-size:10px"><span class="dot" style="background:var(--success-default)"></span>{f.note ?? "live"}</span>{/if}
                {#if f.error}<span class="errtxt trunc" title={f.error}>{f.error}</span>{/if}
              </span>
            {:else if c.key === "type"}
              <span class="td t3 ui">{f.kind}</span>
            {:else if c.key === "size"}
              <span class="td right mono t2">{f.state === "pending" || f.state === "tunnel" ? "—" : fmtBytes(f.respSize)}</span>
            {:else if c.key === "dur"}
              <span class="td right mono" style:color={(f.durationMs ?? 0) >= 1000 ? "var(--warning-default)" : "var(--text-secondary)"}>
                {f.state === "live" ? "live" : fmtDuration(f.durationMs)}
              </span>
            {:else if c.key === "time"}
              <span class="td mono t3">{fmtTime(f.started)}</span>
            {:else if c.key === "client"}
              <span class="td trunc t2 ui">{f.client ?? "—"}</span>
            {/if}
          {/each}
        </div>
      {/each}
    </div>
  </div>

  {#if (!follow || app.livePaused) && app.newSincePause > 0 && followable}
    <button class="newpill" onclick={jumpToEnd}>
      <Icon name="download" size={12} weight={2} />{app.newSincePause} new request{app.newSincePause === 1 ? "" : "s"}
      <Kbd keys={["End"]} />
    </button>
  {/if}
</div>

{#if menu}
  <Menu items={menu.items} x={menu.x} y={menu.y} onclose={() => (menu = null)} />
{/if}
{#if colMenu}
  <Menu
    x={colMenu.x}
    y={colMenu.y}
    width={200}
    onclose={() => (colMenu = null)}
    items={[{ header: "Columns" }, ...COLUMNS.filter((c) => c.key !== "idx" && c.key !== "path").map((c) => ({
      label: c.title || "TLS",
      icon: (app.hiddenCols.includes(c.key) ? "square" : "check") as "square" | "check",
      run: () => app.hideCol(c.key, !app.hiddenCols.includes(c.key)),
    }))]} />
{/if}

<style>
  .table {
    position: relative;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    background: var(--bg-panel);
  }
  .head,
  .tr {
    display: grid;
    grid-template-columns: var(--grid);
    padding: 0 8px;
    align-items: center;
  }
  .head {
    height: 26px;
    background: var(--bg-raised);
    border-bottom: 1px solid var(--border-default);
    flex: none;
  }
  .hc {
    display: flex;
    align-items: center;
    gap: 3px;
    padding: 0 6px;
    height: 100%;
    font-size: 11px;
    font-weight: 500;
    color: var(--text-secondary);
    min-width: 0;
    overflow: hidden;
  }
  .hc.right {
    justify-content: flex-end;
  }
  .hc.on {
    color: var(--text-primary);
  }
  .hc:hover {
    color: var(--text-primary);
  }
  .scroll {
    position: relative;
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
  }
  .spacer-box {
    position: relative;
  }
  .tr {
    position: absolute;
    left: 0;
    right: 0;
    height: 24px;
    border-bottom: 1px solid var(--border-subtle);
  }
  .tr:hover {
    background: var(--bg-hover);
  }
  .tr.err {
    background: var(--row-error);
  }
  .tr.pinned {
    background: var(--row-pinned);
  }
  .tr.sel {
    background: var(--bg-selected);
    box-shadow: inset 2px 0 0 var(--accent-default);
  }
  .td {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 6px;
    font-size: 11.5px;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    height: 100%;
  }
  .td.mono {
    font-size: 11.5px;
  }
  .td.right {
    justify-content: flex-end;
  }
  .td.ui {
    font-size: 11px;
  }
  .td.path {
    gap: 8px;
  }
  .t2 {
    color: var(--text-secondary);
  }
  .t3 {
    color: var(--text-tertiary);
  }
  .b {
    font-weight: 700;
  }
  .red {
    color: var(--error-default);
  }
  .errtxt {
    color: var(--error-default);
    font-family: var(--font-ui);
    font-size: 11px;
    opacity: 0.85;
    flex: 0 1 auto;
  }
  .newpill {
    position: absolute;
    bottom: 16px;
    left: 50%;
    transform: translateX(-50%);
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 6px 0 12px;
    border-radius: 13px;
    background: var(--accent-default);
    color: var(--text-on-accent);
    font-size: 12px;
    font-weight: 500;
    box-shadow: var(--shadow-2);
    z-index: 5;
  }
  .empty {
    position: absolute;
    inset: 0;
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
    max-width: 380px;
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--text-secondary);
  }
</style>
