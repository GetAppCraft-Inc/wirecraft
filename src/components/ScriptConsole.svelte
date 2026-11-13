<script lang="ts">
  // console.log output from Scripts, polled while visible.
  import { api, type ScriptLog } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";

  let { script = "" }: { script?: string } = $props();

  let lines = $state<ScriptLog[]>([]);
  let onlyThis = $state(false);
  let box = $state<HTMLDivElement>();
  let last = 0;

  async function poll() {
    try {
      const fresh = await api.scriptLogs(last);
      if (!fresh.length) return;
      last = fresh[fresh.length - 1].seq;
      const stick = box ? box.scrollTop + box.clientHeight >= box.scrollHeight - 8 : true;
      lines = [...lines, ...fresh].slice(-1000);
      if (stick) requestAnimationFrame(() => box && (box.scrollTop = box.scrollHeight));
    } catch {
      /* app shutting down */
    }
  }

  $effect(() => {
    poll();
    const t = setInterval(poll, 700);
    return () => clearInterval(t);
  });

  const label = $derived(script.trim() ? `Script · ${script.trim()}` : "Script");
  const shown = $derived(onlyThis ? lines.filter((l) => l.script === label) : lines);

  function time(ms: number) {
    const d = new Date(ms);
    return d.toLocaleTimeString(undefined, { hour12: false }) + "." + String(d.getMilliseconds()).padStart(3, "0");
  }

  async function clear() {
    await api.clearScriptLogs();
    lines = [];
  }
</script>

<div class="console">
  <div class="bar">
    <Icon name="terminal" size={14} />
    <b>Console</b>
    <span class="n">{shown.length}</span>
    <label class="row" style="gap:4px;margin-left:auto"><input type="checkbox" bind:checked={onlyThis} />This script only</label>
    <button class="btn sm ghost" onclick={clear}><Icon name="trash" />Clear</button>
  </div>
  <div class="lines mono" bind:this={box}>
    {#each shown as l (l.seq)}
      <div class="ln {l.level}">
        <span class="t">{time(l.at)}</span>
        {#if l.flowId != null}
          <button class="fid" title="Show this request" onclick={() => app.select(l.flowId!)}>#{l.flowId}</button>
        {:else}
          <span class="fid none">—</span>
        {/if}
        {#if !onlyThis}<span class="sn trunc">{l.script}</span>{/if}
        <span class="msg">{l.message}</span>
      </div>
    {:else}
      <div class="empty">console.log output from scripts shows up here.</div>
    {/each}
  </div>
</div>

<style>
  .console {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    overflow: hidden;
    flex: none;
    height: 170px;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 6px 4px 10px;
    background: var(--bg-raised);
    border-bottom: 1px solid var(--border-subtle);
    font-size: 12px;
    color: var(--text-secondary);
  }
  .bar b {
    font-weight: 500;
    color: var(--text-primary);
  }
  .n {
    color: var(--text-tertiary);
  }
  .lines {
    flex: 1;
    overflow: auto;
    background: var(--bg-sunken);
    font-size: 11.5px;
    padding: 4px 0;
  }
  .ln {
    display: flex;
    gap: 8px;
    padding: 1px 10px;
    align-items: baseline;
  }
  .ln:hover {
    background: var(--bg-hover);
  }
  .t,
  .none {
    color: var(--text-tertiary);
    flex: none;
  }
  .fid {
    flex: none;
    color: var(--accent-text);
    font: inherit;
    cursor: pointer;
  }
  .sn {
    flex: none;
    max-width: 140px;
    color: var(--text-secondary);
  }
  .msg {
    white-space: pre-wrap;
    word-break: break-word;
    min-width: 0;
  }
  .warn .msg {
    color: var(--warning-default);
  }
  .error .msg {
    color: var(--error-default);
  }
  .debug .msg {
    color: var(--text-tertiary);
  }
  .empty {
    padding: 12px;
    color: var(--text-tertiary);
    font-family: var(--font-ui);
  }
  label.row {
    font-size: 12px;
  }
  input[type="checkbox"] {
    accent-color: var(--accent-default);
  }
</style>
