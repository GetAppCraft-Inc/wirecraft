<script lang="ts">
  import { ICONS } from "../lib/icons";
  import { EXTRA_ICONS, type IconName } from "../lib/icons-extra";
  const ALL: Record<string, string> = { ...ICONS, ...EXTRA_ICONS };
  let { name, size = 14, color = "currentColor", weight }: { name: IconName; size?: number; color?: string; weight?: number } = $props();
  const fills = new Set(["play", "record"]);
</script>

<svg
  class="icon"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill={fills.has(name) ? color : "none"}
  stroke={color}
  stroke-width={(weight ?? (size >= 16 ? 1.6 : 1.4)) * (24 / size)}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true">{@html ALL[name]}</svg>

<style>
  .icon {
    flex: none;
    display: block;
  }
</style>
