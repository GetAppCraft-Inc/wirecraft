<script lang="ts">
  import Icon from "./Icon.svelte";
  import Self from "./JsonTree.svelte";

  let { value, name = null, depth = 0, open: openInit = depth < 2 }: { value: unknown; name?: string | null; depth?: number; open?: boolean } = $props();
  // svelte-ignore state_referenced_locally
  let open = $state(openInit);

  const isObj = $derived(value !== null && typeof value === "object");
  const entries = $derived(isObj ? (Array.isArray(value) ? value.map((v, i) => [String(i), v] as [string, unknown]) : Object.entries(value as object)) : []);
  const kind = $derived(value === null ? "null" : Array.isArray(value) ? "array" : typeof value);
  const color = $derived(
    kind === "string" ? "var(--syntax-string)" : kind === "number" ? "var(--syntax-number)" : kind === "boolean" || kind === "null" ? "var(--syntax-keyword)" : "var(--text-tertiary)",
  );
  const shown = $derived(
    kind === "string" ? JSON.stringify(value) : kind === "array" ? `[${entries.length}]` : kind === "object" ? `{${entries.length}}` : String(value),
  );
</script>

<div class="node" style:padding-left="{8 + depth * 16}px" role="treeitem" aria-selected="false" aria-expanded={isObj ? open : undefined}>
  {#if isObj}
    <button class="tw" onclick={() => (open = !open)}><Icon name={open ? "chevron-down" : "chevron-right"} size={12} color="var(--text-tertiary)" /></button>
  {:else}
    <span class="tw"></span>
  {/if}
  {#if name != null}<span class="k">{name}</span><span class="p">:</span>{/if}
  <span class="v trunc selectable" style:color>{shown}</span>
  <span class="meta">{kind}</span>
</div>
{#if isObj && open}
  {#each entries.slice(0, 2000) as [k, v] (k)}
    <Self value={v} name={k} depth={depth + 1} />
  {/each}
  {#if entries.length > 2000}
    <div class="node more" style:padding-left="{24 + depth * 16}px">… {entries.length - 2000} more</div>
  {/if}
{/if}

<style>
  .node {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 20px;
    padding-right: 10px;
    font: 11.5px var(--font-mono);
  }
  .node:hover {
    background: var(--bg-hover);
  }
  .tw {
    width: 12px;
    height: 12px;
    display: grid;
    place-items: center;
    flex: none;
  }
  .k {
    color: var(--syntax-key);
    flex: none;
  }
  .p {
    color: var(--syntax-punct);
    flex: none;
  }
  .v {
    flex: 1;
  }
  .meta {
    font: 10.5px var(--font-ui);
    color: var(--text-tertiary);
    flex: none;
  }
  .more {
    color: var(--text-tertiary);
  }
</style>
