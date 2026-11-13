<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "../../components/Icon.svelte";
  let { n, title, body, done = false, children }: { n: number; title: string; body?: string; done?: boolean; children?: Snippet } = $props();
</script>

<div class="step">
  <span class="c" class:done>{#if done}<Icon name="check" size={12} weight={2.2} color="var(--success-default)" />{:else}{n}{/if}</span>
  <div class="col">
    <div class="t">{title}</div>
    {#if body}<div class="b">{body}</div>{/if}
    {@render children?.()}
  </div>
</div>

<style>
  .step {
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }
  .c {
    width: 22px;
    height: 22px;
    border-radius: 11px;
    display: grid;
    place-items: center;
    flex: none;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    background: var(--bg-raised);
    border: 1px solid var(--border-default);
  }
  .c.done {
    background: var(--success-soft);
    border-color: var(--success-default);
  }
  .col {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
    align-items: flex-start;
  }
  .t {
    font-size: 13px;
    font-weight: 500;
  }
  .b {
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--text-secondary);
  }
</style>
