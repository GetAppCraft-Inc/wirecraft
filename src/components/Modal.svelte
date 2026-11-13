<script lang="ts">
  import type { Snippet } from "svelte";
  import Kbd from "./Kbd.svelte";
  let { title, onclose, width = 520, children, footer }: { title: string; onclose: () => void; width?: number; children: Snippet; footer?: Snippet } = $props();
</script>

<svelte:window onkeydown={(e) => { if (e.key === "Escape") { e.stopPropagation(); onclose(); } }} />
<div class="scrim" role="presentation" onmousedown={(e) => e.target === e.currentTarget && onclose()}>
  <div class="modal" style:width="{width}px" role="dialog" aria-label={title}>
    <div class="hd"><span class="t">{title}</span><Kbd keys={["Esc"]} /></div>
    <div class="bd">{@render children()}</div>
    {#if footer}<div class="ft">{@render footer()}</div>{/if}
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: var(--bg-scrim);
    z-index: 70;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 12vh;
  }
  .modal {
    max-width: calc(100vw - 32px);
    max-height: 76vh;
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
  }
  .t {
    flex: 1;
    font-size: 14px;
    font-weight: 600;
  }
  .bd {
    padding: 16px;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .ft {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 16px;
    background: var(--bg-raised);
    border-top: 1px solid var(--border-subtle);
  }
</style>
