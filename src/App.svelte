<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "./lib/state.svelte";
  import { errMsg } from "./lib/api";
  import { copyFlowAs, exportHar } from "./lib/actions";
  import { SHORTCUTS } from "./lib/commands";
  import Workspace from "./screens/Workspace.svelte";
  import Onboarding from "./screens/Onboarding.svelte";
  import Toasts from "./components/Toasts.svelte";
  import CommandPalette from "./components/CommandPalette.svelte";
  import Modal from "./components/Modal.svelte";
  import Kbd from "./components/Kbd.svelte";
  import RulesSheet from "./components/RulesSheet.svelte";
  import Compose from "./components/Compose.svelte";
  import Diff from "./components/Diff.svelte";
  import { tools } from "./lib/tools.svelte";

  let ws = $state<Workspace>();
  let sidebarOpen = $state(true);
  let fatal = $state<string | null>(null);

  const hooks = {
    focusFilter: () => ws?.focusFilter(),
    toggleSidebar: () => (sidebarOpen = !sidebarOpen),
  };

  onMount(() => {
    app.init().then(() => tools.init()).catch((e) => (fatal = errMsg(e)));
  });

  function typing(e: KeyboardEvent): boolean {
    const t = e.target as HTMLElement;
    return t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable;
  }

  function onkeydown(e: KeyboardEvent) {
    if (app.screen !== "workspace") return;
    const ctrl = e.ctrlKey || e.metaKey;
    const k = e.key.toLowerCase();
    if (ctrl && k === "k") return void (e.preventDefault(), (app.palette = !app.palette));
    if (ctrl && k === "l") return void (e.preventDefault(), ws?.focusFilter());
    if (ctrl && k === "r") return void (e.preventDefault(), app.toggleRecording());
    if (ctrl && k === "b") return void (e.preventDefault(), hooks.toggleSidebar());
    if (ctrl && k === "e") return void (e.preventDefault(), exportHar());
    if (ctrl && e.shiftKey && k === "x") return void (e.preventDefault(), app.clear());
    if (ctrl && e.shiftKey && k === "c" && app.selected != null && !typing(e)) return void (e.preventDefault(), copyFlowAs(app.selected, "curl"));
    if (typing(e) || app.palette || app.shortcuts || tools.sheet || tools.compose || tools.diff || ctrl || e.altKey) return;
    const sel = app.selected;
    switch (e.key) {
      case " ": e.preventDefault(); app.toggleLive(); break;
      case "ArrowDown": e.preventDefault(); ws?.move(1); break;
      case "ArrowUp": e.preventDefault(); ws?.move(-1); break;
      case "PageDown": e.preventDefault(); ws?.move(20); break;
      case "PageUp": e.preventDefault(); ws?.move(-20); break;
      case "Home": e.preventDefault(); ws?.move(-1e9); break;
      case "End": e.preventDefault(); ws?.end(); break;
      case "?": app.shortcuts = true; break;
      case "Escape":
        if (app.multi.size) app.multi = new Set();
        else if (app.query || app.chip !== "all" || app.source) { app.query = ""; app.chip = "all"; app.source = null; }
        break;
      case "r": case "R": if (sel != null) app.replay(sel); break;
      case "p": case "P": if (sel != null) app.togglePin(sel); break;
      case "e": case "E": if (sel != null) tools.composeFrom(sel); break;
      case "d": case "D": tools.compare(app.selection()); break;
      case "b": case "B": { const f = sel != null ? app.byId.get(sel) : undefined; if (f) tools.addBreakpoint(f); break; }
      case "Delete": case "Backspace": app.remove(app.selection()); break;
    }
  }
</script>

<svelte:window {onkeydown} oncontextmenu={(e) => { if (!(e.target as HTMLElement).closest(".selectable, input, textarea")) e.preventDefault(); }} />

{#if fatal}
  <div class="fatal"><b>xwire couldn't start</b><span class="mono">{fatal}</span></div>
{:else if !app.ready}
  <div class="fatal"></div>
{:else if app.screen === "onboarding"}
  <Onboarding />
{:else}
  <Workspace bind:this={ws} bind:sidebarOpen />
{/if}

{#if app.palette}<CommandPalette {hooks} />{/if}
{#if tools.sheet && tools.rules}<RulesSheet />{/if}
{#if tools.compose}<Compose draft={tools.compose} />{/if}
{#if tools.diff}{#key tools.diff.join()}<Diff ids={tools.diff} />{/key}{/if}
{#if app.shortcuts}
  <Modal title="Keyboard shortcuts" width={440} onclose={() => (app.shortcuts = false)}>
    {#each SHORTCUTS as [keys, label]}
      <div style="display:flex;align-items:center;gap:8px"><span style="flex:1;color:var(--text-secondary)">{label}</span><Kbd {keys} /></div>
    {/each}
  </Modal>
{/if}
<Toasts />

<style>
  .fatal {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
  }
</style>
