<script lang="ts">
  import { tools, type ComposeDraft } from "../lib/tools.svelte";
  import Icon from "./Icon.svelte";
  import Kbd from "./Kbd.svelte";

  let { draft }: { draft: ComposeDraft } = $props();
  // svelte-ignore state_referenced_locally
  let d = $state({ ...draft });
  let busy = $state(false);
  const METHODS = ["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD"];
  const valid = $derived(/^https?:\/\/[^/\s]+/i.test(d.url.trim()));

  async function send() {
    if (!valid || busy) return;
    busy = true;
    if (await tools.send(d)) tools.compose = null;
    busy = false;
  }
  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      tools.compose = null;
    } else if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
      e.preventDefault();
      send();
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="scrim" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (tools.compose = null)}>
  <div class="modal" role="dialog" aria-label="Compose request">
    <div class="hd"><Icon name="send" size={16} color="var(--accent-text)" /><b>Compose request</b><span class="spacer"></span><Kbd keys={["Esc"]} /></div>
    <div class="bd">
      <div class="row" style="gap:6px">
        <select bind:value={d.method} class="mono">
          {#each METHODS.includes(d.method) ? METHODS : [d.method, ...METHODS] as m}<option>{m}</option>{/each}
        </select>
        <label class="input" style="flex:1"><input class="mono" bind:value={d.url} placeholder="https://api.example.com/v1/users" spellcheck="false" /></label>
      </div>
      <span class="lbl">Headers <span class="dim">one per line, <span class="mono">name: value</span></span></span>
      <textarea class="mono" rows="8" bind:value={d.headers} spellcheck="false"></textarea>
      <span class="lbl">Body</span>
      <textarea class="mono" rows="10" bind:value={d.body} spellcheck="false" placeholder={d.method === "GET" ? "GET requests usually have no body" : '{ "key": "value" }'}></textarea>
      <span class="dim" style="font-size:11.5px">Sent through WireCraft, so your rules apply and the exchange appears in the table. Content-Length is set for you.</span>
    </div>
    <div class="ft">
      <button class="btn secondary" onclick={() => (tools.compose = null)}>Cancel</button>
      <button class="btn primary" disabled={!valid || busy} onclick={send}><Icon name="send" />Send <Kbd keys={["Ctrl", "↵"]} /></button>
    </div>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 65;
    background: var(--bg-scrim);
    display: grid;
    place-items: center;
  }
  .modal {
    width: min(720px, calc(100vw - 48px));
    max-height: calc(100vh - 80px);
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
    padding: 14px 16px;
    border-bottom: 1px solid var(--border-subtle);
    font-size: 14px;
  }
  .bd {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    overflow: auto;
  }
  .ft {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 16px;
    background: var(--bg-raised);
    border-top: 1px solid var(--border-subtle);
  }
  .lbl {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-secondary);
    margin-top: 4px;
  }
  .dim {
    color: var(--text-tertiary);
    font-weight: 400;
  }
  textarea,
  select {
    background: var(--bg-sunken);
    border: 1px solid var(--border-default);
    border-radius: 6px;
    padding: 6px 8px;
    font-size: 12px;
    resize: vertical;
  }
  select {
    height: 28px;
    width: 104px;
  }
  textarea:focus,
  select:focus {
    border-color: var(--accent-default);
  }
</style>
