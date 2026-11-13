<script lang="ts">
  let { keys, size = 10.5 }: { keys: string[]; size?: number } = $props();
</script>

<span class="kbd">
  {#each keys as k}<span class="key" style:font-size="{size}px">{k}</span>{/each}
</span>

<style>
  .kbd {
    display: inline-flex;
    gap: 2px;
    flex: none;
  }
  .key {
    min-width: 16px;
    padding: 1px 4px;
    border-radius: 3px;
    background: var(--bg-sunken);
    border: 1px solid var(--border-default);
    color: var(--text-secondary);
    font-weight: 500;
    line-height: 1.25;
    text-align: center;
  }
</style>
