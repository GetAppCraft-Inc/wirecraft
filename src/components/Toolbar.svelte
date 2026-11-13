<script lang="ts">
  import { app } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";
  import Kbd from "./Kbd.svelte";
  import Logo from "./Logo.svelte";
  import WindowControls from "./WindowControls.svelte";
  import type { IconName } from "../lib/icons-extra";
  import { tools } from "../lib/tools.svelte";

  let { compact = false }: { compact?: boolean } = $props();

  type ToolBtn = { ic: IconName; label: string; run: () => void; count: () => number; disabled?: string };
  const TOOLS: ToolBtn[] = [
    { ic: "breakpoint", label: "Breakpoints", run: () => tools.open("breakpoints"), count: () => tools.counts.breakpoints },
    { ic: "map", label: "Map Local", run: () => tools.open("mapLocal"), count: () => tools.counts.mapLocal },
    { ic: "remote", label: "Map Remote", run: () => tools.open("mapRemote"), count: () => tools.counts.mapRemote },
    { ic: "rewrite", label: "Rewrite", run: () => tools.open("rewrite"), count: () => tools.counts.rewrite },
    { ic: "code", label: "Scripts", run: () => tools.open("scripts"), count: () => tools.counts.scripts },
    { ic: "send", label: "Compose", run: () => tools.composeFrom(null), count: () => 0 },
    { ic: "diff", label: "Diff", run: () => tools.compare(app.selection()), count: () => 0 },
  ];

  const themeSetting = $derived(app.settings?.theme ?? "system");
  const themeIcon = $derived(themeSetting === "light" ? "sun" : themeSetting === "dark" ? "moon" : "contrast");
  const themeLabel = $derived(themeSetting === "light" ? "Light" : themeSetting === "dark" ? "Dark" : "System");
  const running = $derived(app.proxy?.running ?? false);
  const port = $derived(app.proxy?.port ?? app.settings?.port ?? 9090);

  function confirmClear() {
    app.clear();
  }
</script>

<header class="toolbar" data-tauri-drag-region>
  <Logo size={18} wordmark={!compact} />
  <button class="rec" class:off={!app.recording} title="{app.recording ? 'Stop' : 'Start'} recording (Ctrl+R)" onclick={() => app.toggleRecording()}>
    <span class="dot" style="width:8px;height:8px"></span>
    {#if !compact}<span>{app.recording ? "Recording" : "Stopped"}</span>{/if}
  </button>
  <button class="icon-btn" class:active={app.livePaused} title="{app.livePaused ? 'Resume' : 'Pause'} live table (Space)" onclick={() => app.toggleLive()}>
    <Icon name={app.livePaused ? "play" : "pause"} size={16} />
  </button>
  <button class="icon-btn" title="Clear all (Ctrl+Shift+X)" onclick={confirmClear}><Icon name="trash" size={16} /></button>
  <span class="sep"></span>
  {#each TOOLS as t}
    {@const n = t.count()}
    {#if compact}
      <button class="icon-btn" class:active={n > 0} disabled={!!t.disabled} title={t.disabled ?? t.label} onclick={t.run}><Icon name={t.ic} size={16} /></button>
    {:else}
      <button class="tool" class:active={n > 0} disabled={!!t.disabled} title={t.disabled ?? t.label} onclick={t.run}>
        <Icon name={t.ic} />{t.label}{#if n}<span class="cnt">{n}</span>{/if}
      </button>
    {/if}
  {/each}
  <span class="spacer" data-tauri-drag-region></span>
  <button class="status" title={app.proxy?.error ?? `Proxy listening on ${app.proxy?.addr ?? ""}`} onclick={() => { app.onboardingStep = 1; app.screen = "onboarding"; }}>
    <span class="dot" style="width:7px;height:7px" style:background={app.proxy?.error ? "var(--error-default)" : running ? "var(--success-default)" : "var(--text-tertiary)"}></span>
    <span class="mono">:{port}</span>
    {#if !compact}
      <span class="vsep"></span>
      {#if app.caTrusted}
        <Icon name="shield-check" size={13} color="var(--success-default)" /><span class="muted">CA trusted</span>
      {:else if app.caTrusted === false}
        <Icon name="alert" size={13} color="var(--warning-default)" /><span class="muted">CA not trusted</span>
      {:else}
        <Icon name="shield" size={13} color="var(--text-tertiary)" /><span class="muted">CA</span>
      {/if}
    {/if}
  </button>
  <button class="palette" class:compact onclick={() => (app.palette = true)} title="Command palette (Ctrl+K)">
    <Icon name="command" size={13} color="var(--text-tertiary)" />
    {#if !compact}<span class="trunc ph">Search or run a command…</span><Kbd keys={["Ctrl", "K"]} />{/if}
  </button>
  <button class="icon-btn" title="Theme: {themeLabel} (click to switch)" aria-label="Theme: {themeLabel}" onclick={() => app.cycleTheme()}>
    <Icon name={themeIcon} size={16} />
  </button>
  <button class="icon-btn" title="Setup & settings" onclick={() => { app.onboardingStep = 2; app.screen = "onboarding"; }}><Icon name="settings" size={16} /></button>
  <WindowControls />
</header>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 6px;
    height: var(--size-toolbar);
    padding: 0 12px;
    background: var(--bg-raised);
    border-bottom: 1px solid var(--border-default);
    flex: none;
    min-width: 0;
    overflow: hidden;
  }
  .rec {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 10px;
    border-radius: var(--radius-md);
    background: var(--error-soft);
    color: var(--error-default);
    font-size: 12px;
    font-weight: 500;
    flex: none;
  }
  .rec .dot {
    background: var(--error-default);
    animation: pulse 1.6s ease-in-out infinite;
  }
  .rec.off {
    background: var(--bg-sunken);
    color: var(--text-secondary);
  }
  .rec.off .dot {
    background: var(--text-tertiary);
    animation: none;
  }
  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
  .sep {
    width: 1px;
    height: 20px;
    background: var(--border-default);
    margin: 0 2px;
    flex: none;
  }
  .tool {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 28px;
    padding: 0 8px;
    border-radius: var(--radius-md);
    font-size: 12px;
    font-weight: 500;
    color: var(--text-secondary);
    white-space: nowrap;
    flex: none;
  }
  .tool:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .tool.active {
    background: var(--accent-soft);
    color: var(--accent-text);
  }
  .tool:disabled {
    opacity: 0.45;
  }
  .cnt {
    display: inline-grid;
    place-items: center;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    border-radius: 4px;
    background: var(--accent-default);
    color: var(--text-on-accent);
    font-size: 10px;
    font-weight: 600;
  }
  .status {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 28px;
    padding: 0 10px;
    border-radius: var(--radius-md);
    background: var(--bg-sunken);
    border: 1px solid var(--border-subtle);
    font-size: 11.5px;
    flex: none;
  }
  .status:hover {
    border-color: var(--border-default);
  }
  .status .muted {
    color: var(--text-secondary);
  }
  .vsep {
    width: 1px;
    height: 14px;
    background: var(--border-default);
  }
  .palette {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    width: 168px;
    min-width: 32px;
    padding: 0 8px;
    border-radius: var(--radius-md);
    background: var(--bg-sunken);
    border: 1px solid var(--border-default);
    flex: 0 1 auto;
    overflow: hidden;
  }
  .palette.compact {
    width: 36px;
    justify-content: center;
  }
  .palette .ph {
    flex: 1;
    min-width: 0;
    text-align: left;
    color: var(--text-tertiary);
    font-size: 12px;
  }
</style>
