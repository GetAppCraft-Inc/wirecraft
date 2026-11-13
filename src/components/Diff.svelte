<script lang="ts">
  import { api, errMsg, type FlowDetail, type Headers } from "../lib/api";
  import { tools } from "../lib/tools.svelte";
  import { diffLines, normalise } from "../lib/diff";
  import { fmtBytes, fmtDuration, methodColor, statusColor, url } from "../lib/format";
  import Icon from "./Icon.svelte";
  import Kbd from "./Kbd.svelte";

  let { ids }: { ids: [number, number] } = $props();
  let a = $state<FlowDetail | null>(null);
  let b = $state<FlowDetail | null>(null);
  let error = $state<string | null>(null);
  let side = $state<"request" | "response">("response");
  let onlyChanges = $state(true);

  $effect(() => {
    Promise.all([api.flowDetail(ids[0]), api.flowDetail(ids[1])])
      .then(([x, y]) => ((a = x), (b = y)))
      .catch((e) => (error = errMsg(e)));
  });

  function headerRows(x: Headers, y: Headers) {
    const map = (h: Headers) => {
      const m = new Map<string, string>();
      for (const [k, v] of h) {
        const lk = k.toLowerCase();
        m.set(lk, m.has(lk) ? `${m.get(lk)}, ${v}` : v);
      }
      return m;
    };
    const ma = map(x), mb = map(y);
    const keys = [...new Set([...ma.keys(), ...mb.keys()])].sort();
    return keys.map((k) => ({ k, a: ma.get(k), b: mb.get(k), same: ma.get(k) === mb.get(k) }));
  }

  const hdrs = $derived(a && b ? (side === "request" ? headerRows(a.reqHeaders, b.reqHeaders) : headerRows(a.respHeaders, b.respHeaders)) : []);
  const body = $derived.by(() => {
    if (!a || !b) return [];
    const [x, y] = side === "request" ? [a.reqBody, b.reqBody] : [a.respBody, b.respBody];
    const tx = x.text ?? (x.base64 ? `<binary ${fmtBytes(x.decodedSize)}>` : "");
    const ty = y.text ?? (y.base64 ? `<binary ${fmtBytes(y.decodedSize)}>` : "");
    return diffLines(normalise(tx), normalise(ty));
  });
  const changed = $derived(body.filter((l) => l.kind !== "same").length);
  // collapse long unchanged runs to 3 lines of context
  const shown = $derived.by(() => {
    if (!onlyChanges) return body.map((l) => ({ ...l, gap: 0 }));
    const keep = new Set<number>();
    body.forEach((l, i) => {
      if (l.kind !== "same") for (let k = i - 3; k <= i + 3; k++) keep.add(k);
    });
    const out: ((typeof body)[number] & { gap: number })[] = [];
    let gap = 0;
    body.forEach((l, i) => {
      if (keep.has(i)) {
        if (gap) out.push({ kind: "same", text: "", gap });
        gap = 0;
        out.push({ ...l, gap: 0 });
      } else gap++;
    });
    if (gap) out.push({ kind: "same", text: "", gap });
    return out;
  });
</script>

<svelte:window onkeydown={(e) => { if (e.key === "Escape") { e.stopPropagation(); tools.diff = null; } }} />

{#snippet head(d: FlowDetail, label: string)}
  <div class="fh">
    <span class="tag">{label} · #{d.summary.id}</span>
    <span class="mono b" style:color={methodColor(d.summary.method)}>{d.summary.method}</span>
    <span class="mono trunc" style="flex:1" title={url(d.summary)}>{url(d.summary)}</span>
    <span class="mono b" style:color={statusColor(d.summary.status)}>{d.summary.status ?? "—"}</span>
    <span class="mono dim">{fmtDuration(d.summary.durationMs)} · {fmtBytes(d.summary.respSize)}</span>
  </div>
{/snippet}

<div class="scrim" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (tools.diff = null)}>
  <div class="sheet" role="dialog" aria-label="Compare requests">
    <div class="hd">
      <Icon name="diff" size={16} color="var(--accent-text)" /><b>Compare</b>
      <div class="seg" style="margin-left:12px">
        <button class:on={side === "request"} onclick={() => (side = "request")}>Request</button>
        <button class:on={side === "response"} onclick={() => (side = "response")}>Response</button>
      </div>
      <span class="spacer"></span>
      <button class="btn sm ghost" onclick={() => (tools.diff = [ids[1], ids[0]])}><Icon name="remote" />Swap</button>
      <Kbd keys={["Esc"]} />
      <button class="icon-btn" title="Close" onclick={() => (tools.diff = null)}><Icon name="x" size={16} /></button>
    </div>
    {#if error}
      <div class="empty">{error}</div>
    {:else if !a || !b}
      <div class="empty">Loading…</div>
    {:else}
      <div class="heads">{@render head(a, "A")}{@render head(b, "B")}</div>
      <div class="content">
        <div class="sec">Headers <span class="dim">{hdrs.filter((h) => !h.same).length} different</span></div>
        <div class="ht">
          {#each hdrs.filter((h) => !onlyChanges || !h.same) as h}
            <div class="hr" class:diff={!h.same}>
              <span class="mono k trunc" title={h.k}>{h.k}</span>
              <span class="mono v" class:del={!h.same && h.a != null} class:none={h.a == null}>{h.a ?? "—"}</span>
              <span class="mono v" class:add={!h.same && h.b != null} class:none={h.b == null}>{h.b ?? "—"}</span>
            </div>
          {:else}
            <div class="dim" style="padding:6px 12px">Identical headers</div>
          {/each}
        </div>
        <div class="sec">
          Body <span class="dim">{changed ? `${changed} changed line${changed === 1 ? "" : "s"}` : "identical"}</span>
          <span class="spacer"></span>
          <label class="row" style="gap:6px;font-weight:400;text-transform:none;letter-spacing:0"><input type="checkbox" bind:checked={onlyChanges} />Only changes</label>
        </div>
        <div class="code mono selectable">
          {#each shown as l}
            {#if l.gap}
              <div class="gap">⋯ {l.gap} unchanged line{l.gap === 1 ? "" : "s"}</div>
            {:else}
              <div class="ln {l.kind}">
                <span class="g">{l.a ?? ""}</span><span class="g">{l.b ?? ""}</span>
                <span class="sg">{l.kind === "add" ? "+" : l.kind === "del" ? "−" : " "}</span>
                <span class="t">{l.text}</span>
              </div>
            {/if}
          {:else}
            <div class="dim" style="padding:6px 12px">Both bodies are empty</div>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: var(--bg-scrim);
    display: grid;
    place-items: center;
  }
  .sheet {
    width: min(1100px, calc(100vw - 48px));
    height: calc(100vh - 80px);
    display: flex;
    flex-direction: column;
    background: var(--bg-overlay);
    border: 1px solid var(--border-strong);
    border-radius: 10px;
    box-shadow: var(--shadow-3);
    overflow: hidden;
  }
  .hd {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px 10px 16px;
    border-bottom: 1px solid var(--border-subtle);
    font-size: 14px;
  }
  .heads {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1px;
    background: var(--border-subtle);
    border-bottom: 1px solid var(--border-subtle);
  }
  .fh {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    background: var(--bg-raised);
    font-size: 11.5px;
    min-width: 0;
  }
  .tag {
    font-size: 10.5px;
    font-weight: 600;
    color: var(--text-tertiary);
    white-space: nowrap;
  }
  .b {
    font-weight: 700;
  }
  .dim {
    color: var(--text-tertiary);
    font-weight: 400;
  }
  .content {
    flex: 1;
    overflow: auto;
  }
  .sec {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px 6px;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--text-secondary);
  }
  .hr {
    display: grid;
    grid-template-columns: 200px 1fr 1fr;
    gap: 8px;
    padding: 3px 12px;
    font-size: 11.5px;
    border-bottom: 1px solid var(--border-subtle);
  }
  .hr .k {
    color: var(--text-secondary);
  }
  .hr .v {
    word-break: break-all;
  }
  .del {
    background: var(--error-soft);
  }
  .add {
    background: var(--success-soft);
  }
  .none {
    color: var(--text-tertiary);
  }
  .code {
    margin: 0 12px 12px;
    padding: 6px 0;
    background: var(--bg-sunken);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
  }
  .ln {
    display: flex;
    font-size: 11.5px;
    line-height: 17px;
  }
  .ln.add {
    background: var(--success-soft);
  }
  .ln.del {
    background: var(--error-soft);
  }
  .g {
    width: 40px;
    flex: none;
    text-align: right;
    padding-right: 8px;
    color: var(--text-tertiary);
    font-size: 10.5px;
    user-select: none;
  }
  .sg {
    width: 16px;
    flex: none;
    color: var(--text-tertiary);
  }
  .add .sg {
    color: var(--success-default);
  }
  .del .sg {
    color: var(--error-default);
  }
  .t {
    white-space: pre-wrap;
    word-break: break-all;
    padding-right: 10px;
  }
  .gap {
    padding: 2px 0 2px 104px;
    font-size: 11px;
    color: var(--text-tertiary);
    background: var(--bg-raised);
  }
  .empty {
    margin: auto;
    color: var(--text-tertiary);
  }
</style>
