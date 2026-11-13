<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { fmtBytes, fmtInt } from "../lib/format";
  import Kbd from "./Kbd.svelte";
  import { tools } from "../lib/tools.svelte";

  let { compact = false }: { compact?: boolean } = $props();
  const filtering = $derived(!!app.query || app.chip !== "all" || !!app.source);
  const st = $derived(
    app.proxy?.error ? ["Error", "var(--error-default)"] : !app.recording ? ["Stopped", "var(--text-tertiary)"] : app.livePaused ? ["Paused", "var(--warning-default)"] : ["Live", "var(--success-default)"],
  );
</script>

<footer class="sb">
  <span class="g">
    <span class="dot" style="width:7px;height:7px" style:background={st[1]}></span>
    <span style:color={st[1]} style="font-weight:500">{st[0]}</span>
    <span class="mono t2">{app.rate} req/s</span>
    <span class="t3">·</span>
    <span class="mono t2" title={app.proxy?.error ?? ""}>{app.proxy?.addr ?? "not listening"}</span>
  </span>
  <span class="t2">{fmtInt(app.stats.flows)} requests · {fmtBytes(app.stats.bytes)}</span>
  {#if filtering}<span class="t2">{fmtInt(app.filtered.length)} shown</span>{/if}
  {#if app.multi.size > 1}<span class="t2">{app.multi.size} selected</span>{/if}
  {#if app.settings && app.settings.intercept.mode !== "all"}
    <span style="color:var(--warning-default)">HTTPS interception {app.settings.intercept.mode === "off" ? "off" : "for listed hosts"}</span>
  {/if}
  {#if tools.counts.breakpoints || tools.paused.length}
    <button class="lnk" style="color:var(--warning-default)" onclick={() => (tools.paused.length ? app.select(tools.paused[0].flowId) : tools.open("breakpoints"))}>
      Breakpoints {tools.counts.breakpoints}{#if tools.paused.length} · {tools.paused.length} paused{/if}
    </button>
    {#if tools.paused.length > 1}<button class="lnk" onclick={() => tools.resumeAll()}>Resume all</button>{/if}
  {/if}
  {#if !compact}
    {#if tools.counts.mapLocal}<button class="lnk" onclick={() => tools.open("mapLocal")}>Map Local {tools.counts.mapLocal}</button>{/if}
    {#if tools.counts.mapRemote}<button class="lnk" onclick={() => tools.open("mapRemote")}>Map Remote {tools.counts.mapRemote}</button>{/if}
    {#if tools.counts.rewrite}<button class="lnk" onclick={() => tools.open("rewrite")}>Rewrite {tools.counts.rewrite} rule{tools.counts.rewrite === 1 ? "" : "s"}</button>{/if}
    {#if tools.counts.block}<button class="lnk" onclick={() => tools.open("block")}>Blocking {tools.counts.block}</button>{/if}
  {/if}
  <button class="lnk" style:color={tools.rules?.throttle.enabled ? "var(--warning-default)" : undefined} onclick={() => tools.open("throttle")}>
    Throttle {tools.rules?.throttle.enabled ? tools.rules.throttle.preset : "off"}
  </button>
  <span class="spacer"></span>
  {#if !compact}
    <span class="h"><Kbd keys={["Ctrl", "L"]} size={10} /> Filter</span>
    <span class="h"><Kbd keys={["Space"]} size={10} /> Pause</span>
    <span class="h"><Kbd keys={["Ctrl", "K"]} size={10} /> Commands</span>
    <span class="h"><Kbd keys={["?"]} size={10} /> Shortcuts</span>
  {/if}
  <span class="mono t3" style="font-size:10.5px">xwire {app.info?.version ?? ""}</span>
</footer>

<style>
  .sb {
    display: flex;
    align-items: center;
    gap: 14px;
    height: var(--size-statusbar);
    padding: 0 12px;
    background: var(--bg-raised);
    border-top: 1px solid var(--border-default);
    font-size: 11px;
    flex: none;
    white-space: nowrap;
    overflow: hidden;
  }
  .g,
  .h {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .h {
    gap: 5px;
    color: var(--text-tertiary);
  }
  .t2 {
    color: var(--text-secondary);
  }
  .lnk {
    font-size: 11px;
    color: var(--text-secondary);
  }
  .lnk:hover {
    color: var(--text-primary);
    text-decoration: underline;
  }
  .t3 {
    color: var(--text-tertiary);
  }
</style>
