<script lang="ts">
  let { size = 18, wordmark = true }: { size?: number; wordmark?: boolean } = $props();
  const dot = $derived(Math.max(4, size * 0.28));
</script>

<span class="logo">
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="var(--accent-default)"
    stroke-width={Math.max(1.6, size / 9) * (24 / size)} stroke-linecap="round" aria-hidden="true">
    <path d="M4 4C10 4 14 20 20 20" /><path d="M4 20C10 20 14 4 20 4" />
    <circle cx="12" cy="12" r={(dot / 2) * (24 / size)} fill="var(--text-primary)" stroke="none" />
  </svg>
  {#if wordmark}<span class="word" style:font-size="{Math.max(13, size * 0.72)}px">xwire</span>{/if}
</span>

<style>
  .logo {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    flex: none;
  }
  .word {
    font-weight: 700;
    letter-spacing: -0.3px;
    color: var(--text-primary);
  }
</style>
