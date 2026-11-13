<script lang="ts">
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { tools, newBreakpoint, newMapLocal, newMapRemote, newRewrite, newScript, newId, matcher, THROTTLE_PRESETS, headersToText, textToHeaders, type Tool } from "../lib/tools.svelte";
  import type { Matcher, RewriteAction, RewriteTarget } from "../lib/api";
  import Icon from "./Icon.svelte";
  import Kbd from "./Kbd.svelte";
  import ScriptConsole from "./ScriptConsole.svelte";
  import { api, errMsg } from "../lib/api";
  import { app } from "../lib/state.svelte";
  import type { IconName } from "../lib/icons";

  const TABS: [Tool, string, IconName][] = [
    ["breakpoints", "Breakpoints", "breakpoint"],
    ["mapLocal", "Map Local", "map"],
    ["mapRemote", "Map Remote", "remote"],
    ["rewrite", "Rewrite", "rewrite"],
    ["scripts", "Scripts", "code"],
    ["block", "Block", "x"],
    ["throttle", "Throttle", "throttle"],
  ];
  const HELP: Record<Tool, string> = {
    breakpoints: "Pause matching requests or responses so you can edit them before they continue.",
    mapLocal: "Answer matching requests with a canned response — the server is never contacted.",
    mapRemote: "Send matching requests to a different server, e.g. production → localhost.",
    rewrite: "Change headers, query params, URLs, bodies or status codes on the fly.",
    block: "Refuse matching requests with 403 Forbidden.",
    scripts: "Run JavaScript on matching requests and responses to change them, or answer them yourself.",
    throttle: "Simulate a slow network with added latency and limited bandwidth.",
  };
  const METHODS = ["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD"];
  const TARGETS: [RewriteTarget, string][] = [
    ["requestHeader", "Request header"], ["queryParam", "Query parameter"], ["url", "URL"], ["requestBody", "Request body"],
    ["responseHeader", "Response header"], ["status", "Status code"], ["responseBody", "Response body"],
  ];
  const ACTIONS: [RewriteAction, string][] = [["set", "Set"], ["add", "Add"], ["remove", "Remove"], ["replace", "Find & replace"]];

  const tab = $derived(tools.sheet!);
  const r = $derived(tools.rules!);
  type AnyRule = { id: string; enabled: boolean; match: Matcher; name?: string };
  const list = $derived.by((): AnyRule[] => {
    switch (tab) {
      case "breakpoints": return r.breakpoints;
      case "mapLocal": return r.mapLocal;
      case "mapRemote": return r.mapRemote;
      case "rewrite": return r.rewrite;
      case "scripts": return r.scripts;
      case "block": return r.block;
      default: return [];
    }
  });
  let selId = $state<string | null>(tools.focus);
  $effect(() => {
    // keep a valid selection when switching tabs or deleting
    if (!list.some((x) => x.id === selId)) selId = list[0]?.id ?? null;
  });
  const masterKey = $derived(({ breakpoints: "breakpointsOn", mapLocal: "mapLocalOn", mapRemote: "mapRemoteOn", rewrite: "rewriteOn", scripts: "scriptsOn" } as Record<string, "breakpointsOn" | "mapLocalOn" | "mapRemoteOn" | "rewriteOn" | "scriptsOn">)[tab]);

  const sel = $derived(list.find((x) => x.id === selId) ?? null);
  const bp = $derived(tab === "breakpoints" ? r.breakpoints.find((x) => x.id === selId) : undefined);
  const ml = $derived(tab === "mapLocal" ? r.mapLocal.find((x) => x.id === selId) : undefined);
  const mr = $derived(tab === "mapRemote" ? r.mapRemote.find((x) => x.id === selId) : undefined);
  const rw = $derived(tab === "rewrite" ? r.rewrite.find((x) => x.id === selId) : undefined);
  const bl = $derived(tab === "block" ? r.block.find((x) => x.id === selId) : undefined);
  const sc = $derived(tab === "scripts" ? r.scripts.find((x) => x.id === selId) : undefined);

  // ---- script syntax check (debounced while typing)
  let check = $state<{ id: string; ok: boolean; text: string } | null>(null);
  let checkTimer: ReturnType<typeof setTimeout> | null = null;
  function scheduleCheck(id: string, code: string, delay = 400) {
    if (checkTimer) clearTimeout(checkTimer);
    checkTimer = setTimeout(async () => {
      try {
        const fns = await api.checkScript(code);
        check = { id, ok: fns.length > 0, text: fns.length ? `Defines ${fns.join(" and ")}` : "Defines neither onRequest nor onResponse — it will never run" };
      } catch (e) {
        check = { id, ok: false, text: errMsg(e) };
      }
    }, delay);
  }
  $effect(() => {
    if (sc) scheduleCheck(sc.id, sc.code, 0);
  });

  function codeKeys(e: KeyboardEvent & { currentTarget: HTMLTextAreaElement }) {
    if (e.key !== "Tab" || !sc) return;
    e.preventDefault();
    const ta = e.currentTarget;
    const [a, b] = [ta.selectionStart, ta.selectionEnd];
    sc.code = sc.code.slice(0, a) + "  " + sc.code.slice(b);
    requestAnimationFrame(() => ta.setSelectionRange(a + 2, a + 2));
    save();
  }

  function add() {
    let id: string;
    switch (tab) {
      case "breakpoints": { const x = newBreakpoint(); r.breakpoints.push(x); id = x.id; r.breakpointsOn = true; break; }
      case "mapLocal": { const x = newMapLocal(); r.mapLocal.push(x); id = x.id; r.mapLocalOn = true; break; }
      case "mapRemote": { const x = newMapRemote(); r.mapRemote.push(x); id = x.id; r.mapRemoteOn = true; break; }
      case "rewrite": { const x = newRewrite(); r.rewrite.push(x); id = x.id; r.rewriteOn = true; break; }
      case "scripts": { const x = newScript(); r.scripts.push(x); id = x.id; r.scriptsOn = true; break; }
      case "block": { const x = { id: newId(), enabled: true, match: matcher() }; r.block.push(x); id = x.id; break; }
      default: return;
    }
    selId = id;
    tools.save(true);
  }

  function remove(id: string) {
    switch (tab) {
      case "breakpoints": r.breakpoints = r.breakpoints.filter((x) => x.id !== id); break;
      case "mapLocal": r.mapLocal = r.mapLocal.filter((x) => x.id !== id); break;
      case "mapRemote": r.mapRemote = r.mapRemote.filter((x) => x.id !== id); break;
      case "rewrite": r.rewrite = r.rewrite.filter((x) => x.id !== id); break;
      case "scripts": r.scripts = r.scripts.filter((x) => x.id !== id); break;
      case "block": r.block = r.block.filter((x) => x.id !== id); break;
    }
    tools.save(true);
  }

  function duplicate(id: string) {
    const src = list.find((x) => x.id === id);
    if (!src) return;
    const copy = { ...$state.snapshot(src), id: newId() } as AnyRule;
    (list as AnyRule[]).push(copy);
    selId = copy.id;
    tools.save(true);
  }

  function summary(x: AnyRule): string {
    const m = x.match;
    const u = m.url || "Any URL";
    return (m.methods.length ? m.methods.join(",") + " " : "") + u;
  }

  function toggleMethod(m: Matcher, method: string) {
    m.methods = m.methods.includes(method) ? m.methods.filter((x) => x !== method) : [...m.methods, method];
    tools.save();
  }

  async function pickFile() {
    if (!ml) return;
    const f = await openDialog({ multiple: false, directory: false });
    if (typeof f === "string") {
      ml.file = f;
      tools.save();
    }
  }

  function close() {
    tools.save(true);
    tools.sheet = null;
  }
  const save = () => tools.save();
</script>

<svelte:window onkeydown={(e) => { if (e.key === "Escape") { e.stopPropagation(); close(); } }} />

{#snippet match(m: Matcher)}
  <div class="field">
    <span class="lbl">URL</span>
    <label class="input"><input class="mono" bind:value={m.url} oninput={save} placeholder="api.example.com/v1/*  — empty matches every request" spellcheck="false" /></label>
    <span class="hint">Use <span class="mono">*</span> as a wildcard. Without a scheme it matches <span class="mono">host/path?query</span>; a bare host matches every path on it.</span>
  </div>
  <div class="field">
    <span class="lbl">Methods</span>
    <div class="row" style="gap:4px;flex-wrap:wrap">
      <button class="chip" class:on={!m.methods.length} onclick={() => { m.methods = []; save(); }}>Any</button>
      {#each METHODS as meth}
        <button class="chip mono" class:on={m.methods.includes(meth)} onclick={() => toggleMethod(m, meth)}>{meth}</button>
      {/each}
    </div>
  </div>
{/snippet}

<div class="scrim" role="presentation" onmousedown={(e) => e.target === e.currentTarget && close()}>
  <div class="sheet" role="dialog" aria-label="Rules">
    <div class="hd">
      <div class="tabs md" style="border:0;padding:0;flex:1;height:44px">
        {#each TABS as [k, l, ic]}
          {@const n = k === "throttle" ? (r.throttle.enabled ? 1 : 0) : tools.counts[k as keyof typeof tools.counts]}
          <button class:on={tab === k} onclick={() => (tools.sheet = k)}><Icon name={ic} />{l}{#if n}<span class="n">{n}</span>{/if}</button>
        {/each}
      </div>
      <Kbd keys={["Esc"]} />
      <button class="icon-btn" title="Close" onclick={close}><Icon name="x" size={16} /></button>
    </div>
    <div class="sub">
      <span class="help">{HELP[tab]}</span>
      {#if masterKey}
        <span class="lbl" style="margin-left:auto">{r[masterKey] ? "On" : "Off"}</span>
        <button class="toggle" class:on={r[masterKey]} aria-label="Enable {tab}" onclick={() => { r[masterKey] = !r[masterKey]; tools.save(true); }}></button>
      {/if}
    </div>

    {#if tab === "throttle"}
      <div class="body single">
        <div class="ed">
          <div class="row" style="gap:12px">
            <b style="flex:1">Throttle {r.throttle.enabled ? "on" : "off"}</b>
            <button class="toggle" class:on={r.throttle.enabled} aria-label="Throttle" onclick={() => { r.throttle.enabled = !r.throttle.enabled; tools.save(true); }}></button>
          </div>
          <div class="field">
            <span class="lbl">Preset</span>
            <div class="row" style="gap:4px;flex-wrap:wrap">
              {#each THROTTLE_PRESETS as p}
                <button class="chip" class:on={r.throttle.preset === p.name}
                  onclick={() => { Object.assign(r.throttle, { preset: p.name, latencyMs: p.latencyMs, downloadKbps: p.downloadKbps, uploadKbps: p.uploadKbps }); tools.save(true); }}>{p.name}</button>
              {/each}
            </div>
          </div>
          <div class="row" style="gap:12px">
            {#each [["latencyMs", "Latency (ms)"], ["downloadKbps", "Download (kbit/s)"], ["uploadKbps", "Upload (kbit/s)"]] as [k, l]}
              {@const key = k as "latencyMs" | "downloadKbps" | "uploadKbps"}
              <div class="field" style="flex:1">
                <span class="lbl">{l}</span>
                <label class="input"><input class="mono" type="number" min="0" bind:value={r.throttle[key]} oninput={() => { r.throttle.preset = "Custom"; save(); }} /></label>
              </div>
            {/each}
          </div>
          <span class="hint">0 means unlimited. Latency is added before each request is sent upstream.</span>
          <div class="field">
            <span class="lbl">Only these hosts</span>
            <label class="input"><input class="mono" value={r.throttle.hosts.join(", ")} oninput={(e) => { r.throttle.hosts = e.currentTarget.value.split(",").map((s) => s.trim()).filter(Boolean); save(); }} placeholder="Empty throttles every host — or e.g. *.example.com, api.foo.dev" spellcheck="false" /></label>
          </div>
        </div>
      </div>
    {:else}
      <div class="body">
        <div class="list">
          {#each list as x (x.id)}
            <div class="li" class:on={x.id === selId} role="button" tabindex="0" onclick={() => (selId = x.id)} onkeydown={(e) => e.key === "Enter" && (selId = x.id)}>
              <input type="checkbox" bind:checked={x.enabled} onchange={() => tools.save(true)} onclick={(e) => e.stopPropagation()} aria-label="Enabled" />
              <div class="col" style="flex:1;min-width:0">
                {#if x.name}<span class="nm trunc">{x.name}</span>{/if}
                <span class="mono trunc" class:dim={!x.enabled} style="font-size:11.5px">{summary(x)}</span>
              </div>
            </div>
          {:else}
            <div class="empty">No rules yet.</div>
          {/each}
          <button class="btn sm ghost" style="margin:6px" onclick={add}><Icon name="plus" />Add rule</button>
        </div>

        <div class="ed">
          {#if !sel}
            <div class="empty" style="margin:auto">Add a rule, or right-click a request in the table to create one from it.</div>
          {:else}
            <div class="row" style="gap:8px">
              {#if tab !== "block"}
                <label class="input" style="flex:1"><input bind:value={(sel as { name: string }).name} oninput={save} placeholder="Name (optional)" /></label>
              {:else}
                <span style="flex:1"></span>
              {/if}
              <button class="btn sm ghost" onclick={() => duplicate(sel.id)}><Icon name="copy" />Duplicate</button>
              <button class="btn sm danger" onclick={() => remove(sel.id)}><Icon name="trash" />Delete</button>
            </div>
            {@render match(sel.match)}

            {#if bp}
              <div class="field">
                <span class="lbl">Pause on</span>
                <div class="row" style="gap:16px">
                  <label class="row" style="gap:6px"><input type="checkbox" bind:checked={bp.request} onchange={save} />Request (before it's sent)</label>
                  <label class="row" style="gap:6px"><input type="checkbox" bind:checked={bp.response} onchange={save} />Response (before the client gets it)</label>
                </div>
              </div>
              <span class="hint">Paused exchanges appear in the inspector with Execute and Abort. Clients may time out if you leave them waiting.</span>
            {:else if ml}
              <div class="row" style="gap:12px">
                <div class="field" style="width:110px">
                  <span class="lbl">Status</span>
                  <label class="input"><input class="mono" type="number" min="100" max="599" bind:value={ml.status} oninput={save} /></label>
                </div>
                <div class="field" style="flex:1">
                  <span class="lbl">Body from file (optional)</span>
                  <div class="row" style="gap:6px">
                    <label class="input" style="flex:1"><input class="mono" bind:value={ml.file} oninput={save} placeholder="Use the body below" spellcheck="false" /></label>
                    <button class="btn sm secondary" onclick={pickFile}><Icon name="folder" />Choose…</button>
                  </div>
                </div>
              </div>
              <div class="field">
                <span class="lbl">Headers <span class="hint">one per line, <span class="mono">name: value</span></span></span>
                <textarea class="mono" rows="4" value={headersToText(ml.headers)} oninput={(e) => { ml.headers = textToHeaders(e.currentTarget.value); save(); }} spellcheck="false"></textarea>
              </div>
              {#if !ml.file.trim()}
                <div class="field grow">
                  <span class="lbl">Body</span>
                  <textarea class="mono grow" bind:value={ml.body} oninput={save} spellcheck="false"></textarea>
                </div>
              {/if}
            {:else if mr}
              <div class="field">
                <span class="lbl">Send to</span>
                <label class="input"><input class="mono" bind:value={mr.to} oninput={save} placeholder="http://localhost:3000" spellcheck="false" /></label>
              </div>
              <label class="row" style="gap:6px"><input type="checkbox" bind:checked={mr.replacePath} onchange={save} />Replace the path (otherwise it's appended to the target's path)</label>
              <span class="hint">e.g. <span class="mono">https://api.example.com/v1/users?x=1</span> → <span class="mono">{mr.to.replace(/\/$/, "") || "…"}{mr.replacePath ? "?x=1" : "/v1/users?x=1"}</span></span>
            {:else if rw}
              <div class="row" style="gap:12px">
                <div class="field" style="flex:1">
                  <span class="lbl">Change</span>
                  <select bind:value={rw.target} onchange={save}>{#each TARGETS as [k, l]}<option value={k}>{l}</option>{/each}</select>
                </div>
                <div class="field" style="flex:1">
                  <span class="lbl">Action</span>
                  <select bind:value={rw.action} onchange={save}>
                    {#each ACTIONS.filter(([a]) => !((rw.target === "url" || rw.target === "status") && (a === "add" || a === "remove"))) as [k, l]}<option value={k}>{l}</option>{/each}
                  </select>
                </div>
              </div>
              {#if rw.target === "requestHeader" || rw.target === "responseHeader" || rw.target === "queryParam"}
                <div class="field">
                  <span class="lbl">{rw.target === "queryParam" ? "Parameter" : "Header"} name{rw.action === "replace" ? " (empty = all)" : ""}</span>
                  <label class="input"><input class="mono" bind:value={rw.key} oninput={save} spellcheck="false" placeholder={rw.target === "queryParam" ? "page" : "authorization"} /></label>
                </div>
              {/if}
              {#if rw.action === "replace"}
                <div class="field">
                  <span class="lbl row" style="gap:12px">Find <label class="row" style="gap:4px;font-weight:400"><input type="checkbox" bind:checked={rw.regex} onchange={save} />Regular expression</label></span>
                  <label class="input"><input class="mono" bind:value={rw.find} oninput={save} spellcheck="false" /></label>
                </div>
              {/if}
              {#if rw.action !== "remove"}
                <div class="field grow">
                  <span class="lbl">{rw.action === "replace" ? "Replace with" + (rw.regex ? " ($1 = first group)" : "") : rw.target === "status" ? "Status code" : "Value"}</span>
                  {#if rw.target === "requestBody" || rw.target === "responseBody"}
                    <textarea class="mono grow" bind:value={rw.value} oninput={save} spellcheck="false"></textarea>
                  {:else}
                    <label class="input"><input class="mono" bind:value={rw.value} oninput={save} spellcheck="false" /></label>
                  {/if}
                </div>
              {/if}
              {#if rw.target === "responseBody" || rw.target === "responseHeader" || rw.target === "status"}
                <span class="hint">Response rules buffer the whole response (up to 32 MB) and remove compression so text edits apply.</span>
              {/if}
            {:else if sc}
              <div class="row" style="gap:16px;flex-wrap:wrap">
                <label class="row" style="gap:6px"><input type="checkbox" bind:checked={sc.request} onchange={save} />Run <span class="mono">onRequest</span></label>
                <label class="row" style="gap:6px"><input type="checkbox" bind:checked={sc.response} onchange={save} />Run <span class="mono">onResponse</span></label>
                <label class="row" style="gap:6px;margin-left:auto">Timeout
                  <span class="input" style="width:84px"><input class="mono" type="number" min="0" max="30000" step="100" value={sc.timeoutMs || ""} oninput={(e) => { sc.timeoutMs = Math.max(0, Math.round(Number(e.currentTarget.value) || 0)); save(); }} placeholder="1000" /></span>ms</label>
              </div>
              <div class="field grow code">
                <span class="lbl row" style="gap:8px">
                  Code
                  {#if check && check.id === sc.id}
                    <span class="chk" class:bad={!check.ok}><Icon name={check.ok ? "check" : "alert"} size={12} />{check.text}</span>
                  {/if}
                  <button class="btn sm ghost" style="margin-left:auto" title="Forget context.state for this script" onclick={() => api.resetScriptState(sc.id).then(() => app.toast({ kind: "info", title: "Script state cleared", body: "context.state starts empty on the next run." }))}>Reset state</button>
                </span>
                <textarea class="mono grow" bind:value={sc.code} oninput={() => { save(); scheduleCheck(sc.id, sc.code); }} onkeydown={codeKeys} spellcheck="false" autocomplete="off" autocapitalize="off"></textarea>
              </div>
              <span class="hint">Sandboxed QuickJS: no network, files or timers; 64 MB memory. A script that throws or times out leaves the exchange unchanged and is flagged in the table. Response scripts buffer the body (≤ 32 MB) and remove compression.</span>
              <ScriptConsole script={sc.name} />
            {:else if bl}
              <span class="hint">Matching requests get <span class="mono">403 Forbidden</span> from WireCraft.</span>
            {/if}
          {/if}
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: var(--bg-scrim);
    display: grid;
    place-items: center;
  }
  .sheet {
    width: min(960px, calc(100vw - 48px));
    height: min(640px, calc(100vh - 80px));
    display: flex;
    flex-direction: column;
    background: var(--bg-overlay);
    border: 1px solid var(--border-strong);
    border-radius: 10px;
    box-shadow: var(--shadow-3);
    overflow: hidden;
  }
  .hd {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 8px 0 12px;
    border-bottom: 1px solid var(--border-subtle);
  }
  .sub {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 16px;
    background: var(--bg-raised);
    border-bottom: 1px solid var(--border-subtle);
  }
  .help {
    font-size: 12px;
    color: var(--text-secondary);
  }
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .list {
    width: 300px;
    flex: none;
    border-right: 1px solid var(--border-subtle);
    overflow: auto;
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-radius: 6px;
    min-height: 32px;
  }
  .li:hover {
    background: var(--bg-hover);
  }
  .li.on {
    background: var(--bg-selected);
  }
  .nm {
    font-size: 12.5px;
    font-weight: 500;
  }
  .dim {
    color: var(--text-tertiary);
    text-decoration: line-through;
  }
  .ed {
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .single .ed {
    max-width: 640px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .field.grow {
    flex: 1;
    min-height: 140px;
  }
  .field.code {
    min-height: 260px;
  }
  .field.code textarea {
    tab-size: 2;
    line-height: 1.5;
  }
  .chk {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-weight: 400;
    font-size: 11.5px;
    color: var(--success-default);
  }
  .chk.bad {
    color: var(--error-default);
  }
  .lbl {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-secondary);
  }
  .hint {
    font-size: 11.5px;
    font-weight: 400;
    color: var(--text-tertiary);
    line-height: 1.45;
  }
  .empty {
    padding: 12px;
    color: var(--text-tertiary);
    font-size: 12px;
    text-align: center;
  }
  textarea,
  select {
    background: var(--bg-sunken);
    border: 1px solid var(--border-default);
    border-radius: 6px;
    padding: 6px 8px;
    font-size: 12px;
    resize: vertical;
  }
  textarea.grow {
    flex: 1;
    resize: none;
  }
  select {
    height: 28px;
    padding: 0 6px;
  }
  textarea:focus,
  select:focus {
    border-color: var(--accent-default);
  }
  input[type="checkbox"] {
    accent-color: var(--accent-default);
  }
  label.row {
    font-size: 12.5px;
  }
</style>
