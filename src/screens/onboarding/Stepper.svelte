<script lang="ts">
  import Icon from "../../components/Icon.svelte";
  let { active, onstep }: { active: number; onstep?: (i: number) => void } = $props();
  const STEPS = ["Root certificate", "Proxy & port", "Ready"];
</script>

<div class="stepper">
  {#each STEPS as lab, i}
    {@const done = i < active}
    {@const on = i === active}
    <button class="st" disabled={!onstep || i === 2} onclick={() => onstep?.(i)}>
      <span class="c" class:on class:done>
        {#if done}<Icon name="check" size={11} weight={2.2} color="var(--success-default)" />{:else}{i + 1}{/if}
      </span>
      <span class="l" class:dim={!on && !done} class:b={on}>{lab}</span>
    </button>
    {#if i < 2}<span class="ln" class:done></span>{/if}
  {/each}
</div>

<style>
  .stepper {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: none;
  }
  .st {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .c {
    width: 20px;
    height: 20px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    font-size: 11px;
    font-weight: 600;
    background: var(--bg-raised);
    border: 1px solid var(--border-default);
    color: var(--text-tertiary);
  }
  .c.on {
    background: var(--accent-default);
    border-color: transparent;
    color: var(--text-on-accent);
  }
  .c.done {
    background: var(--success-soft);
    border-color: var(--success-default);
  }
  .l {
    font-size: 12.5px;
  }
  .l.b {
    font-weight: 500;
  }
  .l.dim {
    color: var(--text-tertiary);
  }
  .ln {
    width: 40px;
    height: 1px;
    background: var(--border-default);
  }
  .ln.done {
    background: var(--success-default);
  }
</style>
