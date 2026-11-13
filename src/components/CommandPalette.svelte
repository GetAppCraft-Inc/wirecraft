<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { commands, type Command } from "../lib/commands";
  import Icon from "./Icon.svelte";
  import Kbd from "./Kbd.svelte";

  let { hooks }: { hooks: { focusFilter: () => void; toggleSidebar: () => void } } = $props();
  let q = $state("");
  let idx = $state(0);

  const all = $derived(commands(hooks).filter((c) => !c.when || c.when()));
  const list = $derived.by((): Command[] => {
    const s = q.trim().toLowerCase();
    if (!s) return all;
    // subsequence match, ranked by how early the match starts
    const score = (t: string) => {
      t = t.toLowerCase();
      if (t.includes(s)) return t.indexOf(s);
      let j = 0;
      for (const ch of t) if (ch === s[j]) j++;
      return j === s.length ? 100 : -1;
    };
    return all.map((c) => [c, score(c.title)] as const).filter(([, n]) => n >= 0).sort((a, b) => a[1] - b[1]).map(([c]) => c);
  });

  function run(c: Command) {
    app.palette = false;
    c.run();
  }
  function onkeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown") idx = Math.min(list.length - 1, idx + 1);
    else if (e.key === "ArrowUp") idx = Math.max(0, idx - 1);
    else if (e.key === "Enter" && list[idx]) run(list[idx]);
    else if (e.key === "Escape") app.palette = false;
    else return;
    e.preventDefault();
    e.stopPropagation();
  }
</script>

<div class="scrim" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (app.palette = false)}>
  <div class="pal overlay">
    <label class="in">
      <Icon name="command" size={16} color="var(--text-tertiary)" />
      <!-- svelte-ignore a11y_autofocus -->
      <input autofocus placeholder="Type a command…" bind:value={q} oninput={() => (idx = 0)} {onkeydown} />
      <Kbd keys={["Esc"]} />
    </label>
    <div class="list">
      {#each list as c, i (c.id)}
        <button class="it" class:on={i === idx} onmouseenter={() => (idx = i)} onclick={() => run(c)}>
          <Icon name={c.icon} />
          <span class="t">{c.title}</span>
          {#if c.keys}<Kbd keys={c.keys} />{/if}
        </button>
      {:else}
        <div class="none">No matching commands</div>
      {/each}
    </div>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 70;
    background: var(--bg-scrim);
    display: flex;
    justify-content: center;
    padding-top: 14vh;
  }
  .pal {
    width: 560px;
    max-width: calc(100vw - 32px);
    align-self: flex-start;
    border-radius: 10px;
    box-shadow: var(--shadow-3);
    overflow: hidden;
  }
  .in {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 44px;
    padding: 0 14px;
    border-bottom: 1px solid var(--border-default);
  }
  .in input {
    flex: 1;
    background: none;
    border: 0;
    font-size: 14px;
  }
  .list {
    max-height: 360px;
    overflow: auto;
    padding: 4px;
  }
  .it {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 30px;
    padding: 0 10px;
    border-radius: 6px;
    color: var(--text-secondary);
    text-align: left;
  }
  .it .t {
    flex: 1;
    color: var(--text-primary);
    font-size: 12.5px;
  }
  .it.on {
    background: var(--bg-selected);
  }
  .none {
    padding: 12px;
    color: var(--text-tertiary);
  }
</style>
