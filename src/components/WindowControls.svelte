<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Icon from "./Icon.svelte";
  const w = getCurrentWindow();
</script>

<div class="wc">
  <button class="icon-btn" style="width:24px;height:24px" title="Minimize" onclick={() => w.minimize()}><Icon name="minus" /></button>
  <button class="icon-btn" style="width:24px;height:24px" title="Maximize" onclick={() => w.toggleMaximize()}><Icon name="square" /></button>
  <button class="icon-btn close" style="width:24px;height:24px" title="Close" onclick={() => w.close()}><Icon name="x" /></button>
</div>

<style>
  .wc {
    display: flex;
    gap: 2px;
  }
  .close:hover {
    background: var(--error-default);
    color: #fff;
  }
</style>
