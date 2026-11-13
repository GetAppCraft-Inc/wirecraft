<script lang="ts" module>
  import type { IconName } from "../lib/icons";
  export type MenuItem =
    | "-"
    | { header: string }
    | { label: string; icon?: IconName; keys?: string; danger?: boolean; disabled?: boolean; run?: () => void; sub?: MenuItem[] };
</script>

<script lang="ts">
  import Icon from "./Icon.svelte";
  import Self from "./Menu.svelte";

  let { items, x, y, onclose, width = 232, nested = false }: { items: MenuItem[]; x: number; y: number; onclose: () => void; width?: number; nested?: boolean } = $props();

  let el: HTMLDivElement;
  let pos = $state({ x: 0, y: 0 });
  let open = $state<number | null>(null);
  let subPos = $state({ x: 0, y: 0 });

  $effect(() => {
    const r = el.getBoundingClientRect();
    const vw = window.innerWidth, vh = window.innerHeight;
    pos = {
      x: x + r.width > vw - 4 ? (nested ? x - r.width - width - 4 : vw - r.width - 4) : x,
      y: Math.max(4, y + r.height > vh - 4 ? vh - r.height - 4 : y),
    };
  });

  function enter(i: number, e: MouseEvent) {
    const it = items[i];
    if (typeof it === "object" && "sub" in it && it.sub) {
      const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
      subPos = { x: r.right + 2, y: r.top - 4 };
      open = i;
    } else open = null;
  }
  function click(it: MenuItem) {
    if (typeof it !== "object" || "header" in it || it.disabled || it.sub) return;
    it.run?.();
    onclose();
  }
</script>

{#if !nested}
  <div class="scrim" role="presentation" onmousedown={onclose} oncontextmenu={(e) => { e.preventDefault(); onclose(); }}></div>
{/if}
<div class="menu overlay" bind:this={el} style:left="{pos.x}px" style:top="{pos.y}px" style:width="{width}px" role="menu">
  {#each items as it, i}
    {#if it === "-"}
      <div class="sep"></div>
    {:else if "header" in it}
      <div class="hdr trunc">{it.header}</div>
    {:else}
      <button class="mi" class:danger={it.danger} class:open={open === i} disabled={it.disabled} onmouseenter={(e) => enter(i, e)} onclick={() => click(it)}>
        {#if it.icon}<Icon name={it.icon} />{:else}<span style="width:14px"></span>{/if}
        <span class="l trunc">{it.label}</span>
        {#if it.keys}<span class="k">{it.keys}</span>{/if}
        {#if it.sub}<Icon name="chevron-right" size={12} color="var(--text-tertiary)" />{/if}
      </button>
    {/if}
  {/each}
</div>
{#if open != null}
  {@const it = items[open]}
  {#if typeof it === "object" && "sub" in it && it.sub}
    <Self items={it.sub} x={subPos.x} y={subPos.y} {onclose} nested width={190} />
  {/if}
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 50;
  }
  .menu {
    position: fixed;
    z-index: 51;
    padding: 4px;
  }
  .sep {
    height: 1px;
    margin: 4px 0;
    background: var(--border-default);
  }
  .hdr {
    padding: 6px 8px 2px;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--text-tertiary);
  }
  .mi {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 26px;
    padding: 0 8px;
    border-radius: 5px;
    color: var(--text-secondary);
    text-align: left;
  }
  .mi .l {
    flex: 1;
    font-size: 12.5px;
    color: var(--text-primary);
  }
  .mi .k {
    font-size: 11px;
    color: var(--text-tertiary);
  }
  .mi:hover:not(:disabled),
  .mi.open {
    background: var(--accent-default);
    color: var(--text-on-accent);
  }
  .mi:hover:not(:disabled) .l,
  .mi:hover:not(:disabled) .k,
  .mi.open .l {
    color: var(--text-on-accent);
  }
  .mi.danger,
  .mi.danger .l {
    color: var(--error-default);
  }
  .mi:disabled {
    opacity: 0.45;
  }
</style>
