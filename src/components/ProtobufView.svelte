<script lang="ts">
  // gRPC / gRPC-Web / protobuf bodies, decoded without a schema: field numbers and wire types only.
  import type { GrpcBody } from "../lib/api";
  import { fmtBytes } from "../lib/format";
  import PbFields from "./PbFields.svelte";

  let { data }: { data: GrpcBody } = $props();
  const msgs = $derived(data.messages ?? []);
</script>

<div class="pb" role="tree">
  <div class="note">No .proto schema — showing field numbers and wire types. Nested messages are guessed.</div>
  {#if data.error}
    <div class="err">{data.error}</div>
  {/if}
  {#if data.kind !== "protobuf" && !msgs.length && !data.error}
    <div class="note">No messages.</div>
  {/if}
  {#each msgs as m, i (i)}
    {#if data.kind !== "protobuf"}
      <div class="mh">
        <b>Message {i + 1}</b>
        {#if m.size != null}<span>{fmtBytes(m.size)}</span>{/if}
        {#if m.compressed}<span class="tag">compressed</span>{/if}
      </div>
    {/if}
    {#if m.error}
      <div class="err">{m.error}</div>
      {#if m.hex}<div class="hex mono selectable">{m.hex}</div>{/if}
    {:else if m.fields}
      {#if m.fields.length}<PbFields fields={m.fields} />{:else}<div class="note">Empty message</div>{/if}
    {/if}
  {/each}
  {#if data.trailers}
    <div class="mh"><b>Trailers</b></div>
    {#each Object.entries(data.trailers) as [k, v]}
      <div class="tr mono"><span class="k">{k}</span>: <span class="selectable">{v}</span></div>
    {/each}
  {/if}
</div>

<style>
  .pb {
    padding: 4px 0 12px;
  }
  .note {
    padding: 4px 12px 8px;
    font-size: 11.5px;
    color: var(--text-tertiary);
  }
  .mh {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 8px 12px 4px;
    font-size: 12px;
    color: var(--text-secondary);
  }
  .mh b {
    font-weight: 500;
    color: var(--text-primary);
  }
  .tag {
    font-size: 10.5px;
    padding: 0 6px;
    border-radius: var(--radius-full);
    background: var(--info-soft);
    color: var(--info-default);
  }
  .err {
    margin: 4px 12px;
    font-size: 12px;
    color: var(--error-default);
  }
  .hex {
    margin: 0 12px;
    font-size: 11px;
    color: var(--text-tertiary);
    word-break: break-all;
  }
  .tr {
    padding: 2px 12px 2px 24px;
    font-size: 12px;
  }
  .k {
    color: var(--syntax-key);
  }
</style>
