<script lang="ts">
  // One level of a schemaless protobuf message (see crates/wirecraft-core/src/grpc.rs).
  import type { PbField } from "../lib/api";
  import Icon from "./Icon.svelte";
  import Self from "./PbFields.svelte";

  let { fields, depth = 0 }: { fields: PbField[]; depth?: number } = $props();
  let closed = $state<Record<number, boolean>>({});

  function shown(f: PbField): string {
    if (f.type === "message") return `{${(f.value as PbField[]).length} fields}`;
    if (f.type === "string") return JSON.stringify(f.value);
    if (f.type === "bytes") return f.packed ? `[${f.packed.join(", ")}]` : `bytes(${f.size}) ${String(f.value).slice(0, 80)}${String(f.value).length > 80 ? "…" : ""}`;
    return String(f.value);
  }
  function alt(f: PbField): string {
    const a: string[] = [];
    if (f.sint != null) a.push(`sint ${f.sint}`);
    if (f.int64 != null) a.push(`int64 ${f.int64}`);
    if (f.double != null) a.push(`double ${f.double}`);
    if (f.float != null) a.push(`float ${f.float}`);
    if (f.type === "bytes" && f.packed) a.push("packed varints");
    return a.join(" · ");
  }
  function color(f: PbField): string {
    if (f.type === "string") return "var(--syntax-string)";
    if (f.type === "message") return "var(--text-tertiary)";
    if (f.type === "bytes") return "var(--syntax-keyword)";
    return "var(--syntax-number)";
  }
</script>

{#each fields as f, i (i)}
  {@const nested = f.type === "message"}
  <div class="node" style:padding-left="{8 + depth * 16}px" role="treeitem" aria-selected="false" aria-expanded={nested ? !closed[i] : undefined}>
    {#if nested}
      <button class="tw" onclick={() => (closed[i] = !closed[i])}><Icon name={closed[i] ? "chevron-right" : "chevron-down"} size={12} color="var(--text-tertiary)" /></button>
    {:else}
      <span class="tw"></span>
    {/if}
    <span class="k">{f.no}</span><span class="p">:</span>
    <span class="v trunc selectable" style:color={color(f)}>{shown(f)}</span>
    {#if alt(f)}<span class="alt">{alt(f)}</span>{/if}
    <span class="meta">{f.type ?? f.wire}</span>
  </div>
  {#if nested && !closed[i]}
    <Self fields={f.value as PbField[]} depth={depth + 1} />
  {/if}
{/each}

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
    width: 14px;
    flex: none;
    display: grid;
    place-items: center;
  }
  .k {
    color: var(--syntax-key);
  }
  .p {
    color: var(--syntax-punct);
    margin-right: 4px;
  }
  .v {
    min-width: 0;
  }
  .alt {
    color: var(--text-tertiary);
    font-size: 11px;
    white-space: nowrap;
  }
  .meta {
    margin-left: auto;
    padding-left: 12px;
    font: 10.5px var(--font-ui);
    color: var(--text-tertiary);
    flex: none;
  }
</style>
