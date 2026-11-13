<script lang="ts">
  import { copyText } from "../../lib/actions";
  import Icon from "../../components/Icon.svelte";
  let { cmds }: { cmds: string[] } = $props();
  const KW = new Set(["sudo", "security", "certutil", "update-ca-certificates", "update-ca-trust", "trust", "cp", "adb", "export", "curl"]);
  function spans(line: string): [string, string][] {
    return line.split(/(\s+)/).filter(Boolean).map((w) => [
      w,
      /^\s+$/.test(w) ? "" : w.startsWith("-") ? "var(--syntax-key)" : /^["']/.test(w) ? "var(--syntax-string)" : KW.has(w) || w === "&&" ? "var(--syntax-keyword)" : w.includes("/") || w.includes(".") ? "var(--syntax-string)" : "var(--text-primary)",
    ]);
  }
</script>

<div class="sh">
  <div class="lines mono selectable">
    {#each cmds as c}
      <div><span class="p">$ </span>{#each spans(c) as [t, col]}<span style:color={col || undefined}>{t}</span>{/each}</div>
    {/each}
  </div>
  <button class="icon-btn" style="width:22px;height:22px" title="Copy" onclick={() => copyText(cmds.join("\n"), "Copied command")}><Icon name="copy" /></button>
</div>

<style>
  .sh {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    width: 100%;
    padding: 8px 10px;
    background: var(--bg-sunken);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
  }
  .lines {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 11.5px;
    line-height: 1.6;
    word-break: break-all;
  }
  .p {
    color: var(--text-tertiary);
  }
</style>
