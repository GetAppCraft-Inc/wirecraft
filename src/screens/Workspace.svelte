<script lang="ts">
  import { app, type SourceSel } from "../lib/state.svelte";
  import Toolbar from "../components/Toolbar.svelte";
  import Sidebar from "../components/Sidebar.svelte";
  import FilterBar from "../components/FilterBar.svelte";
  import RequestTable from "../components/RequestTable.svelte";
  import Inspector from "../components/Inspector.svelte";
  import StatusBar from "../components/StatusBar.svelte";
  import Icon from "../components/Icon.svelte";
  import type { IconName } from "../lib/icons";

  let { sidebarOpen = $bindable(true) }: { sidebarOpen?: boolean } = $props();

  let width = $state(window.innerWidth);
  let sideW = $state(232);
  let inspW = $state(470);
  let bottomH = $state(300);
  let filterBar: FilterBar;
  let table: RequestTable;

  const compact = $derived(width < 1200);

  export function focusFilter() {
    filterBar?.focus();
  }
  export function move(d: number) {
    table?.move(d);
  }
  export function end() {
    table?.end();
  }

  function drag(e: PointerEvent, apply: (dx: number, dy: number) => void) {
    const x0 = e.clientX, y0 = e.clientY;
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const mv = (ev: PointerEvent) => apply(ev.clientX - x0, ev.clientY - y0);
    const up = () => {
      el.removeEventListener("pointermove", mv);
      el.removeEventListener("pointerup", up);
    };
    el.addEventListener("pointermove", mv);
    el.addEventListener("pointerup", up);
  }

  const RAIL: [IconName, string, SourceSel | null][] = [
    ["layers", "All sources", null],
    ["pin", "Pinned", { kind: "pinned" }],
  ];
</script>

<svelte:window bind:innerWidth={width} />

<div class="ws">
  <Toolbar {compact} />
  <div class="body">
    {#if compact}
      <nav class="rail">
        {#each RAIL as [ic, title, sel]}
          <button class="icon-btn" class:active={JSON.stringify(app.source) === JSON.stringify(sel)} {title} onclick={() => (app.source = sel)}><Icon name={ic} size={16} /></button>
        {/each}
        {#each app.settings?.favourites ?? [] as h}
          <button class="icon-btn" class:active={app.source?.kind === "fav" && app.source.host === h} title={h} onclick={() => (app.source = { kind: "fav", host: h })}><Icon name="star" size={16} /></button>
        {/each}
      </nav>
      <div class="split"></div>
    {:else if sidebarOpen}
      <div class="side" style:width="{sideW}px"><Sidebar onCollapse={() => (sidebarOpen = false)} /></div>
      <div class="split v" role="separator" onpointerdown={(e) => { const w0 = sideW; drag(e, (dx) => (sideW = Math.max(180, Math.min(420, w0 + dx)))); }}></div>
    {:else}
      <nav class="rail">
        <button class="icon-btn" title="Show sidebar (Ctrl+B)" onclick={() => (sidebarOpen = true)}><Icon name="sidebar" size={16} /></button>
      </nav>
      <div class="split"></div>
    {/if}

    <div class="centre">
      <FilterBar bind:this={filterBar} {compact} />
      <RequestTable bind:this={table} {compact} />
      {#if compact}
        <div class="split h" role="separator" onpointerdown={(e) => { const h0 = bottomH; drag(e, (_, dy) => (bottomH = Math.max(160, Math.min(window.innerHeight - 260, h0 - dy)))); }}></div>
        <div class="bottom" style:height="{bottomH}px"><Inspector layout="bottom" /></div>
      {/if}
    </div>

    {#if !compact}
      <div class="split v" role="separator" onpointerdown={(e) => { const w0 = inspW; drag(e, (dx) => (inspW = Math.max(340, Math.min(width - 560, w0 - dx)))); }}></div>
      <div class="insp" style:width="{inspW}px"><Inspector layout="right" /></div>
    {/if}
  </div>
  <StatusBar {compact} />
</div>

<style>
  .ws {
    display: flex;
    flex-direction: column;
    height: 100%;
  }
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .side,
  .insp {
    flex: none;
    min-height: 0;
  }
  .centre {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .bottom {
    flex: none;
    min-height: 0;
  }
  .rail {
    width: 44px;
    flex: none;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 8px 0;
    background: var(--bg-panel);
  }
  .split {
    flex: none;
    background: var(--border-default);
    width: 1px;
    position: relative;
  }
  .split.v {
    cursor: col-resize;
  }
  .split.h {
    width: auto;
    height: 1px;
    cursor: row-resize;
  }
  .split.v::after {
    content: "";
    position: absolute;
    inset: 0 -3px;
  }
  .split.h::after {
    content: "";
    position: absolute;
    inset: -3px 0;
  }
  .split.v:hover,
  .split.h:hover {
    background: var(--accent-default);
  }
</style>
