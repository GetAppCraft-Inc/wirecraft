<script lang="ts">
  // The app icon (an engraved owl on a wire, assets/app-icon) at UI sizes, with the wordmark.
  import icon from "../../assets/app-icon/wirecraft-small.svg?url";

  let { size = 18, wordmark = true }: { size?: number; wordmark?: boolean } = $props();
</script>

<span class="logo">
  <img src={icon} width={size} height={size} alt="" aria-hidden="true" draggable="false" />
  {#if wordmark}<span class="word" style:font-size="{Math.max(13, size * 0.72)}px">Wire<span class="craft">Craft</span></span>{/if}
</span>

<style>
  .logo {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    flex: none;
  }
  img {
    border-radius: 22%;
    flex: none;
  }
  .word {
    font-weight: 700;
    letter-spacing: -0.3px;
    color: var(--text-primary);
  }
  .craft {
    font-weight: 500;
    color: var(--text-secondary);
  }
</style>
