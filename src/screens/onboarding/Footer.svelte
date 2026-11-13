<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "../../components/Icon.svelte";
  let { onback, children }: { onback?: () => void; children: Snippet } = $props();
</script>

<footer class="ft">
  {#if onback}<button class="btn ghost" onclick={onback}><Icon name="chevron-left" />Back</button>{/if}
  <span class="spacer"></span>
  {@render children()}
</footer>

<style>
  .ft {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 14px 32px;
    background: var(--bg-raised);
    border-top: 1px solid var(--border-default);
    flex: none;
  }
</style>
