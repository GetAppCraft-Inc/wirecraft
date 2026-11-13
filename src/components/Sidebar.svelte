<script lang="ts">
  import { app, baseDomain, deviceOf, type SourceSel } from "../lib/state.svelte";
  import { fmtInt } from "../lib/format";
  import Icon from "./Icon.svelte";
  import type { IconName } from "../lib/icons";
  import { save } from "@tauri-apps/plugin-dialog";
  import { api, errMsg } from "../lib/api";
  import { tools } from "../lib/tools.svelte";
  import { importHar, saveSession, deleteSession } from "../lib/sessions";
  import Menu, { type MenuItem } from "./Menu.svelte";

  let naming = $state(false);
  let sessName = $state("");
  let sessMenu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  function relDate(ms: number) {
    const d = new Date(ms);
    const today = new Date();
    return d.toDateString() === today.toDateString()
      ? d.toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" })
      : d.toLocaleDateString("en-GB", { day: "numeric", month: "short" });
  }
  async function commitName() {
    if (sessName.trim() && (await saveSession(sessName.trim()))) {
      naming = false;
      sessName = "";
    }
  }

  let { onCollapse }: { onCollapse: () => void } = $props();

  type Mode = "app" | "domain" | "device";
  let mode = $state<Mode>("app");
  let find = $state("");
  let expanded = $state<Record<string, boolean>>({});

  interface Child { key: string; count: number; tls: boolean }
  interface Group { key: string; count: number; children: Child[] }

  const CLIENT_ICON: Record<string, IconName> = { curl: "terminal", wget: "terminal", httpie: "terminal", python: "terminal", node: "terminal", "WireCraft · repeat": "repeat" };

  const groups = $derived.by((): Group[] => {
    void app.slowVersion;
    const g = new Map<string, Map<string, { n: number; tls: boolean }>>();
    for (const id of app.order) {
      const f = app.byId.get(id);
      if (!f) continue;
      let outer: string, inner: string;
      if (mode === "app") {
        outer = f.client ?? "Unknown";
        inner = f.host;
      } else if (mode === "domain") {
        outer = baseDomain(f.host);
        inner = f.host;
      } else {
        outer = deviceOf(f);
        inner = f.client ?? "Unknown";
      }
      let m = g.get(outer);
      if (!m) g.set(outer, (m = new Map()));
      const c = m.get(inner);
      if (c) {
        c.n++;
        c.tls &&= f.tls;
      } else m.set(inner, { n: 1, tls: f.tls });
    }
    const q = find.trim().toLowerCase();
    const out: Group[] = [];
    for (const [key, m] of g) {
      let children = [...m].map(([k, v]) => ({ key: k, count: v.n, tls: v.tls }));
      if (q && !key.toLowerCase().includes(q)) children = children.filter((c) => c.key.toLowerCase().includes(q));
      if (q && !children.length && !key.toLowerCase().includes(q)) continue;
      children.sort((a, b) => b.count - a.count);
      out.push({ key, count: [...m.values()].reduce((s, v) => s + v.n, 0), children });
    }
    return out.sort((a, b) => b.count - a.count);
  });

  const hostCounts = $derived.by(() => {
    void app.slowVersion;
    const m = new Map<string, number>();
    for (const f of app.byId.values()) m.set(f.host, (m.get(f.host) ?? 0) + 1);
    return m;
  });

  const pinnedFlows = $derived([...app.pinned].map((id) => app.byId.get(id)).filter((f) => !!f));

  function sel(s: SourceSel) {
    app.source = same(app.source, s) ? null : s;
  }
  function same(a: SourceSel | null, b: SourceSel): boolean {
    return !!a && JSON.stringify(a) === JSON.stringify(b);
  }
  function outerSel(key: string): SourceSel {
    return mode === "app" ? { kind: "client", client: key } : mode === "domain" ? { kind: "domain", domain: key } : { kind: "device", device: key };
  }
  function innerSel(outer: string, inner: string): SourceSel {
    return mode === "app"
      ? { kind: "client", client: outer, host: inner }
      : mode === "domain"
        ? { kind: "domain", domain: outer, host: inner }
        : { kind: "device", device: outer, client: inner };
  }
  function groupIcon(key: string): IconName {
    if (mode === "domain") return "globe";
    if (mode === "device") return key === "This computer" ? "monitor" : key === "WireCraft" ? "repeat" : "phone";
    return CLIENT_ICON[key.toLowerCase()] ?? "app";
  }
  function isOpen(key: string, i: number) {
    return expanded[mode + key] ?? i === 0;
  }

  async function exportAll() {
    const path = await save({ defaultPath: "wirecraft-session.har", filters: [{ name: "HAR", extensions: ["har"] }] });
    if (!path) return;
    try {
      const n = await api.exportHar(path);
      app.toast({ kind: "success", title: `Exported ${n} requests`, body: path });
    } catch (e) {
      app.toast({ kind: "error", title: "Export failed", body: errMsg(e) });
    }
  }
</script>

<aside class="sidebar">
  <div class="top">
    <div class="seg" style="flex:1">
      {#each [["app", "App"], ["domain", "Domain"], ["device", "Device"]] as [k, l]}
        <button class:on={mode === k} style="flex:1" onclick={() => (mode = k as Mode)}>{l}</button>
      {/each}
    </div>
    <button class="icon-btn" style="width:26px;height:26px" title="Hide sidebar" onclick={onCollapse}><Icon name="sidebar" /></button>
  </div>
  <div class="find">
    <label class="input" style="height:26px">
      <Icon name="search" color="var(--text-tertiary)" />
      <input placeholder="Find source…" bind:value={find} spellcheck="false" />
    </label>
  </div>

  <div class="body">
    {#if pinnedFlows.length}
      <button class="section-label as-btn" onclick={() => sel({ kind: "pinned" })}>Pinned<span class="n">{pinnedFlows.length}</span></button>
      {#each pinnedFlows as f (f.id)}
        <button class="item" class:sel={same(app.source, { kind: "pin", id: f.id })} onclick={() => { app.select(f.id); }}>
          <span class="chev"></span>
          <Icon name="pin" color="var(--accent-text)" />
          <span class="label mono trunc">{f.method} {f.path}</span>
          <span class="n">#{f.id}</span>
        </button>
      {/each}
    {/if}

    {#if app.settings?.favourites.length}
      <div class="section-label">Favourites<span class="n">{app.settings.favourites.length}</span></div>
      {#each app.settings.favourites as h}
        <button class="item" class:sel={same(app.source, { kind: "fav", host: h })} onclick={() => sel({ kind: "fav", host: h })}
          oncontextmenu={(e) => { e.preventDefault(); app.toggleFavourite(h); }} title="Right-click to remove">
          <span class="chev"></span>
          <Icon name="star" color="var(--warning-default)" />
          <span class="label mono trunc">{h}</span>
          <span class="n">{fmtInt(hostCounts.get(h) ?? 0)}</span>
        </button>
      {/each}
    {/if}

    <div class="section-label">Sources<span class="n">{fmtInt(app.total)}</span></div>
    {#if !groups.length}
      <div class="empty">{find ? "No matching sources" : "Sources appear here as traffic arrives."}</div>
    {/if}
    {#each groups as g, i (g.key)}
      {@const open = isOpen(g.key, i) || !!find}
      {@const s = outerSel(g.key)}
      <button class="item" class:sel={same(app.source, s)} onclick={() => sel(s)}>
        <span class="chev" role="presentation" onclick={(e) => { e.stopPropagation(); expanded[mode + g.key] = !open; }}>
          <Icon name={open ? "chevron-down" : "chevron-right"} size={12} color="var(--text-tertiary)" />
        </span>
        <Icon name={groupIcon(g.key)} color={same(app.source, s) ? "var(--accent-text)" : "var(--text-secondary)"} />
        <span class="label trunc">{g.key}</span>
        <span class="n">{fmtInt(g.count)}</span>
      </button>
      {#if open}
        {#each g.children as c (c.key)}
          {@const cs = innerSel(g.key, c.key)}
          {@const on = same(app.source, cs)}
          <button class="item d1" class:sel={on} onclick={() => sel(cs)}
            oncontextmenu={(e) => { if (mode !== "device") { e.preventDefault(); app.toggleFavourite(c.key); } }}
            title={mode !== "device" ? "Right-click to add to favourites" : undefined}>
            <span class="chev"></span>
            {#if mode === "device"}
              <Icon name="app" color={on ? "var(--accent-text)" : "var(--text-secondary)"} />
            {:else}
              <Icon name={c.tls ? "lock" : "unlock"} color={c.tls ? (on ? "var(--accent-text)" : "var(--text-secondary)") : "var(--warning-default)"} />
            {/if}
            <span class="label trunc" class:mono={mode !== "device"}>{c.key}</span>
            <span class="n">{fmtInt(c.count)}</span>
          </button>
        {/each}
      {/if}
    {/each}
  </div>

  <div class="foot">
    <div class="section-label" style="padding:0 6px">
      Saved sessions<span class="n">{tools.sessions.length}</span>
      <button class="icon-btn" style="width:20px;height:20px;margin-left:6px" title="Save current capture as a session" disabled={!app.total} onclick={() => { naming = true; sessName = `capture-${new Date().toISOString().slice(0, 16).replace(/[T:]/g, "-")}`; }}><Icon name="plus" size={12} /></button>
    </div>
    {#if naming}
      <form class="item" onsubmit={(e) => { e.preventDefault(); commitName(); }}>
        <Icon name="layers" color="var(--text-secondary)" />
        <!-- svelte-ignore a11y_autofocus -->
        <input class="name" autofocus bind:value={sessName} onkeydown={(e) => e.key === "Escape" && (naming = false)} onblur={() => (naming = false)} />
      </form>
    {/if}
    <div class="sessions">
      {#each tools.sessions as s (s.path)}
        <button class="item" title="{s.path} — click to open, right-click for more" onclick={() => importHar(s.path)}
          oncontextmenu={(e) => { e.preventDefault(); sessMenu = { x: e.clientX, y: e.clientY, items: [
            { label: "Open (adds to the table)", icon: "folder", run: () => importHar(s.path) },
            { label: "Show in folder", icon: "external", run: () => import("@tauri-apps/plugin-opener").then((o) => o.revealItemInDir(s.path)) },
            "-",
            { label: "Delete", icon: "trash", danger: true, run: () => deleteSession(s.path) },
          ] }; }}>
          <span class="chev"></span>
          <Icon name="layers" color="var(--text-secondary)" />
          <span class="label trunc">{s.name}</span>
          <span class="n">{relDate(s.modified)}</span>
        </button>
      {/each}
    </div>
    <button class="item" onclick={() => importHar()}>
      <Icon name="plus" size={12} color="var(--text-tertiary)" />
      <span class="label trunc" style="color:var(--text-tertiary);font-size:11.5px">Open or import HAR…</span>
    </button>
    <button class="item" onclick={exportAll}>
      <Icon name="download" size={12} color="var(--text-tertiary)" />
      <span class="label trunc" style="color:var(--text-tertiary);font-size:11.5px">Export session as HAR…</span>
    </button>
  </div>
</aside>

{#if sessMenu}<Menu items={sessMenu.items} x={sessMenu.x} y={sessMenu.y} width={220} onclose={() => (sessMenu = null)} />{/if}

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg-panel);
    overflow: hidden;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 10px 10px 6px;
  }
  .find {
    padding: 0 10px 6px;
  }
  .body {
    flex: 1;
    overflow-y: auto;
    padding: 0 6px;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .as-btn {
    width: 100%;
  }
  .as-btn:hover {
    color: var(--text-secondary);
  }
  .item {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 24px;
    padding: 0 10px;
    border-radius: 5px;
    width: 100%;
    text-align: left;
    flex: none;
  }
  .item.d1 {
    padding-left: 24px;
  }
  .item:hover {
    background: var(--bg-hover);
  }
  .item.sel {
    background: var(--bg-selected);
  }
  .item.sel .label {
    font-weight: 500;
  }
  .chev {
    width: 12px;
    height: 12px;
    flex: none;
    display: grid;
    place-items: center;
  }
  .label {
    flex: 1;
    font-size: 12px;
  }
  .label.mono {
    font-size: 11.5px;
  }
  .n {
    font: 400 10.5px var(--font-mono);
    color: var(--text-tertiary);
    flex: none;
  }
  .empty {
    padding: 6px 12px;
    color: var(--text-tertiary);
    font-size: 11.5px;
  }
  .foot {
    padding: 6px 6px 8px;
    border-top: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .sessions {
    max-height: 120px;
    overflow-y: auto;
  }
  .name {
    flex: 1;
    min-width: 0;
    background: var(--bg-sunken);
    border: 1px solid var(--accent-default);
    border-radius: 4px;
    padding: 1px 4px;
    font-size: 12px;
  }
</style>
