<script lang="ts">
  import { app } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";
  const TONE = { success: "success", error: "error", info: "info", warning: "warning" } as const;
  const IC = { success: "check", error: "alert", info: "info", warning: "alert" } as const;
</script>

<div class="toasts">
  {#each app.toasts as t (t.id)}
    <div class="toast overlay" role="status">
      <span class="ic" style:background="var(--{TONE[t.kind]}-soft)"><Icon name={IC[t.kind]} size={12} weight={2} color="var(--{TONE[t.kind]}-default)" /></span>
      <div class="tx">
        <div class="ti">{t.title}</div>
        {#if t.body}<div class="bo selectable">{t.body}</div>{/if}
      </div>
      {#if t.action}<button class="act" onclick={() => { t.action?.run(); app.dismiss(t.id); }}>{t.action.label}</button>{/if}
      <button class="x" onclick={() => app.dismiss(t.id)} aria-label="Dismiss"><Icon name="x" size={12} color="var(--text-tertiary)" /></button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 20px;
    bottom: 36px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 80;
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    width: 340px;
    padding: 10px 12px;
    pointer-events: auto;
    animation: in 0.14s ease-out;
  }
  @keyframes in {
    from {
      transform: translateY(6px);
      opacity: 0;
    }
  }
  .ic {
    width: 20px;
    height: 20px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    flex: none;
  }
  .tx {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .ti {
    font-size: 12.5px;
    font-weight: 500;
  }
  .bo {
    font-size: 12px;
    color: var(--text-secondary);
    word-break: break-all;
  }
  .act {
    font-size: 12px;
    font-weight: 500;
    color: var(--accent-text);
  }
  .x {
    display: grid;
    place-items: center;
  }
</style>
