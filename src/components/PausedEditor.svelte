<script lang="ts">
  import type { Paused } from "../lib/api";
  import { tools, headersToText, textToHeaders } from "../lib/tools.svelte";
  import Icon from "./Icon.svelte";

  let { p }: { p: Paused } = $props();

  // svelte-ignore state_referenced_locally
  let method = $state(p.method);
  // svelte-ignore state_referenced_locally
  let url = $state(p.url);
  // svelte-ignore state_referenced_locally
  let status = $state(String(p.status ?? 200));
  // svelte-ignore state_referenced_locally
  let headers = $state(headersToText(p.headers));
  // svelte-ignore state_referenced_locally
  let body = $state(p.body);
  let busy = $state(false);

  async function go(abort: boolean) {
    busy = true;
    const resume = abort
      ? { abort: true }
      : p.phase === "request"
        ? { method, url, headers: textToHeaders(headers), body: p.binary ? undefined : body }
        : { status: parseInt(status, 10) || undefined, headers: textToHeaders(headers), body: p.binary ? undefined : body };
    await tools.resume(p.flowId, resume);
    busy = false;
  }

  function onkeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
      e.preventDefault();
      go(false);
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="pe" role="form" {onkeydown}>
  <div class="banner">
    <Icon name="breakpoint" size={14} color="var(--warning-default)" />
    <span class="t">Paused at {p.phase} breakpoint</span>
    <span class="spacer"></span>
    <button class="btn sm danger" disabled={busy} onclick={() => go(true)}><Icon name="x" />Abort</button>
    <button class="btn sm primary" disabled={busy} onclick={() => go(false)} title="Ctrl+Enter"><Icon name="play" />Execute</button>
  </div>
  {#if p.phase === "request"}
    <div class="row" style="gap:6px">
      <label class="input" style="width:96px"><input class="mono" bind:value={method} spellcheck="false" /></label>
      <label class="input" style="flex:1"><input class="mono" bind:value={url} spellcheck="false" /></label>
    </div>
  {:else}
    <div class="row" style="gap:8px">
      <span class="lbl">Status</span>
      <label class="input" style="width:80px"><input class="mono" bind:value={status} inputmode="numeric" /></label>
      <span class="mono dim trunc" style="flex:1">{p.method} {p.url}</span>
    </div>
  {/if}
  <span class="lbl">Headers</span>
  <textarea class="mono" rows="7" bind:value={headers} spellcheck="false"></textarea>
  <span class="lbl">Body {#if p.binary}<span class="dim">— binary, can't be edited here; it will be sent unchanged</span>{/if}</span>
  <textarea class="mono grow" bind:value={body} disabled={p.binary} spellcheck="false"></textarea>
  <span class="dim" style="font-size:11px">Ctrl+Enter executes. Content-Length is updated for you.</span>
</div>

<style>
  .pe {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
  }
  .banner {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px 6px 10px;
    border-radius: 8px;
    background: var(--warning-soft);
    border: 1px solid var(--warning-default);
  }
  .t {
    font-size: 12.5px;
    font-weight: 500;
  }
  .lbl {
    font-size: 11.5px;
    font-weight: 500;
    color: var(--text-secondary);
  }
  .dim {
    color: var(--text-tertiary);
    font-weight: 400;
  }
  textarea {
    background: var(--bg-sunken);
    border: 1px solid var(--border-default);
    border-radius: 6px;
    padding: 6px 8px;
    font-size: 11.5px;
    line-height: 1.5;
    resize: vertical;
  }
  textarea.grow {
    flex: 1;
    min-height: 80px;
    resize: none;
  }
  textarea:focus {
    border-color: var(--accent-default);
  }
</style>
