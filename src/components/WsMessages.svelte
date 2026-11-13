<script lang="ts">
  import { api, type WsMessage } from "../lib/api";
  import { fmtBytes } from "../lib/format";
  import { copyText } from "../lib/actions";
  import Icon from "./Icon.svelte";

  let { id, count }: { id: number; count: number } = $props();
  let msgs = $state<WsMessage[]>([]);
  let sel = $state<number | null>(null);
  let dir = $state<"all" | "out" | "in">("all");
  let find = $state("");

  // reload whenever the flow or its message count changes (count arrives with each batched flow update)
  $effect(() => {
    void count;
    api.wsMessages(id).then((m) => (msgs = m)).catch(() => {});
  });

  const shown = $derived(
    msgs
      .map((m, i) => ({ m, i }))
      .filter(({ m }) => (dir === "all" || (dir === "out") === m.outgoing) && (!find || m.data.toLowerCase().includes(find.toLowerCase()))),
  );
  const cur = $derived(sel != null ? msgs[sel] : null);
  const pretty = $derived.by(() => {
    if (!cur || cur.kind !== "text") return cur?.data ?? "";
    try {
      return JSON.stringify(JSON.parse(cur.data), null, 2);
    } catch {
      return cur.data;
    }
  });
  function time(at: number) {
    const d = new Date(at);
    return d.toLocaleTimeString("en-GB", { hour12: false }) + "." + String(d.getMilliseconds()).padStart(3, "0");
  }
</script>

<div class="ws">
  <div class="bar">
    <div class="seg sm">
      {#each [["all", "All"], ["out", "Sent"], ["in", "Received"]] as [k, l]}<button class:on={dir === k} onclick={() => (dir = k as typeof dir)}>{l}</button>{/each}
    </div>
    <label class="input" style="height:22px;flex:1"><Icon name="search" size={12} color="var(--text-tertiary)" /><input placeholder="Filter messages" bind:value={find} /></label>
    <span class="n mono">{shown.length}{count > msgs.length ? ` of ${count}` : ""}</span>
  </div>
  <div class="list">
    {#each shown as { m, i } (i)}
      <button class="msg" class:on={sel === i} onclick={() => (sel = i)}>
        <Icon name={m.outgoing ? "upload" : "download"} size={12} color={m.outgoing ? "var(--success-default)" : "var(--info-default)"} />
        <span class="mono data trunc" class:ctl={m.kind !== "text"}>{m.kind === "text" ? m.data : m.kind === "binary" ? `binary · ${fmtBytes(m.len)}` : `${m.kind}${m.data ? " · " + m.data : ""}`}</span>
        <span class="mono meta">{fmtBytes(m.len)}</span>
        <span class="mono meta">{time(m.at)}</span>
      </button>
    {:else}
      <div class="none">{msgs.length ? "No matching messages" : "No messages yet. They appear here live."}</div>
    {/each}
  </div>
  {#if cur}
    <div class="detail">
      <div class="dh">
        <span>{cur.outgoing ? "Sent" : "Received"} · {cur.kind} · {fmtBytes(cur.len)}{cur.truncated ? " (first 64 KB shown)" : ""}</span>
        <span class="spacer"></span>
        <button class="icon-btn" style="width:22px;height:22px" title="Copy" onclick={() => copyText(cur.data, "Copied message")}><Icon name="copy" /></button>
      </div>
      <pre class="mono selectable">{pretty}</pre>
    </div>
  {/if}
</div>

<style>
  .ws {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
  }
  .n {
    font-size: 10.5px;
    color: var(--text-tertiary);
  }
  .list {
    flex: 1;
    min-height: 80px;
    overflow: auto;
    border-top: 1px solid var(--border-subtle);
  }
  .msg {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 22px;
    padding: 0 10px;
    border-bottom: 1px solid var(--border-subtle);
    text-align: left;
  }
  .msg:hover {
    background: var(--bg-hover);
  }
  .msg.on {
    background: var(--bg-selected);
  }
  .data {
    flex: 1;
    font-size: 11.5px;
  }
  .ctl {
    color: var(--text-tertiary);
  }
  .meta {
    font-size: 10.5px;
    color: var(--text-tertiary);
    flex: none;
  }
  .none {
    padding: 16px 12px;
    color: var(--text-tertiary);
    font-size: 12px;
  }
  .detail {
    height: 40%;
    min-height: 100px;
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--border-default);
  }
  .dh {
    display: flex;
    align-items: center;
    padding: 4px 8px 4px 12px;
    font-size: 11px;
    color: var(--text-secondary);
  }
  pre {
    flex: 1;
    margin: 0 8px 8px;
    padding: 8px 10px;
    overflow: auto;
    background: var(--bg-sunken);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    font-size: 11.5px;
    white-space: pre-wrap;
    word-break: break-all;
  }
</style>
