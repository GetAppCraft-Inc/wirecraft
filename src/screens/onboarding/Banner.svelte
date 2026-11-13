<script lang="ts">
  import Icon from "../../components/Icon.svelte";
  let { tone, title, body }: { tone: "success" | "warning" | "error" | "info"; title: string; body?: string } = $props();
  const IC = { success: "shield-check", warning: "alert", error: "alert", info: "info" } as const;
</script>

<div class="banner" style:background="var(--{tone}-soft)" style:border-color="var(--{tone}-default)">
  <Icon name={IC[tone]} size={16} color="var(--{tone}-default)" />
  <div class="tx">
    <div class="t">{title}</div>
    {#if body}<div class="b">{body}</div>{/if}
  </div>
</div>

<style>
  .banner {
    display: flex;
    gap: 10px;
    padding: 10px 12px;
    border: 1px solid;
    border-radius: 8px;
    align-items: flex-start;
  }
  .tx {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .t {
    font-size: 12.5px;
    font-weight: 500;
  }
  .b {
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-secondary);
  }
</style>
