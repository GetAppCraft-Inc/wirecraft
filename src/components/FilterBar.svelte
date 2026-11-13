<script lang="ts">
  import { app, CHIPS, chipMatch, type Chip } from "../lib/state.svelte";
  import { completionContext, highlight, KEY_HELP, KEYS, type SpanKind } from "../lib/query";
  import { fmtCount, fmtInt, STATUS_TEXT } from "../lib/format";
  import Icon from "./Icon.svelte";
  import Kbd from "./Kbd.svelte";

  let { compact = false }: { compact?: boolean } = $props();

  let input: HTMLInputElement;
  let mirror: HTMLDivElement;
  let draft = $state(app.query);
  let focused = $state(false);
  let caret = $state(0);
  let acIndex = $state(0);
  let acOpen = $state(false);
  let histOpen = $state(false);
  let timer: ReturnType<typeof setTimeout> | null = null;

  const COLORS: Record<SpanKind, string> = {
    key: "var(--accent-text)", op: "var(--syntax-keyword)", val: "var(--syntax-number)", str: "var(--syntax-string)",
    plain: "var(--text-primary)", neg: "var(--error-default)", glob: "var(--syntax-key)", bad: "var(--warning-default)",
  };
  const spans = $derived(highlight(draft));

  function load(k: string): string[] {
    try {
      return JSON.parse(localStorage.getItem(k) ?? "[]");
    } catch {
      return [];
    }
  }
  function store(k: string, v: string[]) {
    try {
      localStorage.setItem(k, JSON.stringify(v));
    } catch {
      /* ignore */
    }
  }
  let saved = $state<string[]>(load("wirecraft.savedFilters"));
  let recent = $state<string[]>(load("wirecraft.recentFilters"));

  function commit(q = draft) {
    if (timer) clearTimeout(timer);
    app.query = q;
    app.scheduleBodySearch(true);
  }
  function remember() {
    const q = draft.trim();
    if (!q) return;
    recent = [q, ...recent.filter((r) => r !== q)].slice(0, 12);
    store("wirecraft.recentFilters", recent);
  }
  function oninput() {
    caret = input.selectionStart ?? draft.length;
    acOpen = true;
    acIndex = 0;
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => commit(), 120);
  }

  // ---- autocomplete
  interface Sug { insert: string; label: string; detail: string; count?: number }
  const ctx = $derived(completionContext(draft, caret));
  const suggestions = $derived.by((): Sug[] => {
    void app.slowVersion;
    const c = ctx;
    if (!c.key) {
      if (!c.prefix) return [];
      const p = c.prefix.toLowerCase();
      return KEY_HELP.filter((k) => k.key.startsWith(p) && k.key !== p).map((k) => ({ insert: k.key + ":", label: k.key + ":", detail: k.help }));
    }
    const p = c.prefix.toLowerCase();
    let vals: Sug[] = [];
    if (c.key === "status") {
      const counts = new Map<string, number>();
      for (const f of app.byId.values()) {
        const s = f.status == null ? "pending" : String(f.status);
        counts.set(s, (counts.get(s) ?? 0) + 1);
        if (f.status != null) {
          const cls = `${Math.floor(f.status / 100)}xx`;
          counts.set(cls, (counts.get(cls) ?? 0) + 1);
        }
      }
      vals = [...counts].map(([k, n]) => ({
        insert: k, label: k, count: n,
        detail: k === "pending" ? "No response yet" : k.endsWith("xx") ? ({ "1xx": "Informational", "2xx": "Success", "3xx": "Redirects", "4xx": "Client errors", "5xx": "Server errors" } as Record<string, string>)[k] ?? "" : STATUS_TEXT[+k] ?? "",
      }));
      vals.sort((a, b) => (b.label.endsWith("xx") ? 1 : 0) - (a.label.endsWith("xx") ? 1 : 0) || b.label.localeCompare(a.label));
    } else if (c.key === "host" || c.key === "client" || c.key === "method" || c.key === "type") {
      const counts = new Map<string, number>();
      for (const f of app.byId.values()) {
        const v = c.key === "host" ? f.host : c.key === "client" ? f.client ?? "" : c.key === "method" ? f.method : f.kind.toLowerCase();
        if (v && v !== "—") counts.set(v, (counts.get(v) ?? 0) + 1);
      }
      vals = [...counts].sort((a, b) => b[1] - a[1]).map(([k, n]) => ({ insert: /\s/.test(k) ? `"${k}"` : k, label: k, detail: "", count: n }));
    } else {
      vals = (KEY_HELP.find((k) => k.key === c.key)?.values ?? []).map(([v, d]) => ({ insert: v, label: v, detail: d }));
    }
    return vals.filter((v) => v.label.toLowerCase().includes(p) && v.label.toLowerCase() !== p).slice(0, 8);
  });
  const helpFor = $derived(ctx.key ? KEY_HELP.find((k) => k.key === ctx.key) : null);
  const showAc = $derived(focused && acOpen && suggestions.length > 0);

  function complete(s: Sug) {
    const c = ctx;
    const word = draft.slice(c.start, c.end);
    const neg = /^[-!]/.test(word) ? word[0] : "";
    let repl: string;
    if (c.key) {
      const keyPart = word.replace(/^[-!]/, "").split(":")[0];
      repl = `${neg}${keyPart}:${s.insert}`;
    } else repl = neg + s.insert;
    const addSpace = !repl.endsWith(":");
    const before = draft.slice(0, c.start) + repl + (addSpace ? " " : "");
    draft = before + draft.slice(c.end).replace(/^\s+/, "");
    caret = before.length;
    queueMicrotask(() => {
      input.setSelectionRange(caret, caret);
      syncScroll();
    });
    acIndex = 0;
    acOpen = !addSpace;
    commit();
  }

  function onkeydown(e: KeyboardEvent) {
    if (showAc) {
      if (e.key === "ArrowDown") {
        acIndex = (acIndex + 1) % suggestions.length;
        e.preventDefault();
        return;
      }
      if (e.key === "ArrowUp") {
        acIndex = (acIndex - 1 + suggestions.length) % suggestions.length;
        e.preventDefault();
        return;
      }
      if (e.key === "Tab") {
        complete(suggestions[acIndex]);
        e.preventDefault();
        return;
      }
    }
    if (e.key === "Enter") {
      if (showAc) complete(suggestions[acIndex]);
      else {
        commit();
        remember();
        acOpen = false;
      }
      e.preventDefault();
    } else if (e.key === "Escape") {
      if (acOpen && showAc) acOpen = false;
      else if (draft) {
        draft = "";
        commit("");
      } else input.blur();
      e.preventDefault();
      e.stopPropagation();
    }
  }
  function syncScroll() {
    if (mirror && input) mirror.scrollLeft = input.scrollLeft;
  }

  export function focus() {
    input?.focus();
    input?.select();
  }

  const chipCounts = $derived.by(() => {
    void app.slowVersion;
    const out: Record<string, number> = {};
    for (const [c] of CHIPS) out[c] = 0;
    out.errors = 0;
    for (const f of app.byId.values()) {
      for (const [c] of CHIPS) if (c !== "all" && chipMatch(c, f)) out[c]++;
      if (chipMatch("errors", f)) out.errors++;
    }
    return out;
  });
  const chips = $derived(compact ? CHIPS.slice(0, 6) : CHIPS);

  function toggleSaved() {
    const q = draft.trim();
    if (!q) return;
    saved = saved.includes(q) ? saved.filter((s) => s !== q) : [q, ...saved];
    store("wirecraft.savedFilters", saved);
  }
  function pickHistory(q: string) {
    draft = q;
    commit(q);
    histOpen = false;
  }
  function setChip(c: Chip) {
    app.chip = app.chip === c ? "all" : c;
  }
</script>

<div class="fb">
  <div class="r1">
    <div class="qf" class:focus={focused}>
      <Icon name="filter" color={draft ? "var(--accent-text)" : "var(--text-tertiary)"} />
      <div class="field">
        <div class="mirror mono" bind:this={mirror} aria-hidden="true">
          {#if !draft}<span class="ph">Filter  —  status:5xx  host:*.api.com  method:POST  body:"error"</span>{/if}
          {#each spans as s}<span style:color={COLORS[s.kind]}>{s.text}</span>{/each}
        </div>
        <input
          bind:this={input}
          class="mono"
          bind:value={draft}
          spellcheck="false"
          autocomplete="off"
          {oninput}
          {onkeydown}
          onscroll={syncScroll}
          onkeyup={() => { caret = input.selectionStart ?? 0; syncScroll(); }}
          onclick={() => { caret = input.selectionStart ?? 0; acOpen = true; }}
          onfocus={() => { focused = true; }}
          onblur={() => { focused = false; remember(); }} />
      </div>
      {#if app.query}
        <span class="count mono">{fmtInt(app.filtered.length)} of {fmtInt(app.total)}</span>
        <button class="x" title="Clear filter" onclick={() => { draft = ""; commit(""); }}><Icon name="x" size={12} /></button>
      {/if}
      <Kbd keys={["Ctrl", "L"]} />

      {#if showAc}
        <div class="ac overlay" role="listbox">
          {#if helpFor}
            <div class="ac-h"><span class="mono" style="color:var(--accent-text);font-weight:700">{helpFor.key}:</span> {helpFor.help}</div>
          {/if}
          {#each suggestions as s, i}
            <button class="ac-i" class:on={i === acIndex} onmousedown={(e) => { e.preventDefault(); complete(s); }} onmouseenter={() => (acIndex = i)}>
              <span class="mono v" style:color={ctx.key === "status" ? "var(--syntax-number)" : ctx.key ? "var(--text-primary)" : "var(--accent-text)"}>{s.label}</span>
              <span class="d trunc">{s.detail}</span>
              {#if s.count != null}<span class="mono c">{fmtInt(s.count)}</span>{/if}
            </button>
          {/each}
          <div class="ac-f">
            <Kbd keys={["↑"]} /><Kbd keys={["↓"]} /> Navigate
            <Kbd keys={["Tab"]} /> Complete
            <Kbd keys={["↵"]} /> Apply
            <Kbd keys={["Esc"]} /> Close
          </div>
        </div>
      {/if}
    </div>
    <button class="icon-btn" class:active={!!draft.trim() && saved.includes(draft.trim())} title="Save this filter" onclick={toggleSaved}><Icon name="star" size={16} /></button>
    <div class="hist">
      <button class="icon-btn" class:active={histOpen} title="Saved and recent filters" onclick={() => (histOpen = !histOpen)}><Icon name="history" size={16} /></button>
      {#if histOpen}
        <div class="backdrop" role="presentation" onclick={() => (histOpen = false)}></div>
        <div class="menu overlay">
          {#if saved.length}<div class="mh">Saved</div>{/if}
          {#each saved as q}<button class="mi mono trunc" onclick={() => pickHistory(q)}>{q}</button>{/each}
          {#if recent.length}<div class="mh">Recent</div>{/if}
          {#each recent as q}<button class="mi mono trunc" onclick={() => pickHistory(q)}>{q}</button>{/each}
          {#if !saved.length && !recent.length}<div class="mh" style="text-transform:none;letter-spacing:0">No saved or recent filters yet</div>{/if}
        </div>
      {/if}
    </div>
  </div>
  <div class="r2">
    {#each chips as [c, label]}
      <button class="chip" class:on={app.chip === c} onclick={() => setChip(c)}>
        {label}{#if c !== "all"}<span class="n">{fmtCount(chipCounts[c] ?? 0)}</span>{/if}
      </button>
    {/each}
    <button class="chip" class:on={app.chip === "errors"} style:color={app.chip === "errors" ? undefined : "var(--error-default)"} onclick={() => setChip("errors")}>
      <Icon name="alert" size={12} />Errors<span class="n" style:color={app.chip === "errors" ? undefined : "var(--error-default)"}>{fmtCount(chipCounts.errors ?? 0)}</span>
    </button>
  </div>
</div>

<style>
  .fb {
    background: var(--bg-panel);
    border-bottom: 1px solid var(--border-subtle);
    flex: none;
  }
  .r1 {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 8px 6px;
  }
  .r2 {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 8px 8px;
    overflow: hidden;
  }
  .qf {
    position: relative;
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 8px;
    border-radius: var(--radius-md);
    background: var(--bg-sunken);
    border: 1px solid var(--border-default);
  }
  .qf.focus {
    border-color: var(--accent-default);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent-default) 25%, transparent);
  }
  .field {
    position: relative;
    flex: 1;
    min-width: 0;
    height: 100%;
  }
  .mirror,
  .field input {
    position: absolute;
    inset: 0;
    font-size: 12px;
    line-height: 26px;
    white-space: pre;
    padding: 0;
    margin: 0;
    border: 0;
    letter-spacing: 0;
  }
  .mirror {
    overflow: hidden;
    pointer-events: none;
  }
  .mirror .ph {
    color: var(--text-tertiary);
  }
  .field input {
    width: 100%;
    background: transparent;
    color: transparent;
    caret-color: var(--accent-default);
  }
  .field input::selection {
    background: color-mix(in srgb, var(--accent-default) 35%, transparent);
    color: transparent;
  }
  .count {
    font-size: 11px;
    font-weight: 500;
    color: var(--text-tertiary);
    flex: none;
  }
  .x {
    color: var(--text-tertiary);
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    border-radius: 3px;
  }
  .x:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }
  .ac {
    position: absolute;
    left: -1px;
    top: 32px;
    width: min(430px, 100%);
    z-index: 30;
    padding: 4px;
  }
  .ac-h {
    padding: 4px 8px 6px;
    font-size: 11.5px;
    color: var(--text-tertiary);
  }
  .ac-i {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    height: 26px;
    padding: 0 8px;
    border-radius: 5px;
    text-align: left;
  }
  .ac-i.on {
    background: var(--bg-selected);
  }
  .ac-i .v {
    width: 72px;
    font-size: 12px;
    flex: none;
  }
  .ac-i .d {
    flex: 1;
    font-size: 12px;
    color: var(--text-secondary);
  }
  .ac-i .c {
    font-size: 10.5px;
    color: var(--text-tertiary);
  }
  .ac-f {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 4px;
    padding: 6px 8px 2px;
    border-top: 1px solid var(--border-default);
    font-size: 11px;
    color: var(--text-tertiary);
  }
  .hist {
    position: relative;
  }
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 29;
  }
  .menu {
    position: absolute;
    right: 0;
    top: 32px;
    width: 340px;
    padding: 4px;
    z-index: 30;
  }
  .mh {
    padding: 6px 8px 2px;
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-tertiary);
  }
  .mi {
    display: block;
    width: 100%;
    height: 26px;
    padding: 0 8px;
    border-radius: 5px;
    text-align: left;
    font-size: 12px;
  }
  .mi:hover {
    background: var(--accent-default);
    color: var(--text-on-accent);
  }
</style>
