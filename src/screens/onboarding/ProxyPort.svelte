<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "../../lib/state.svelte";
  import { api, errMsg, type InterceptMode, type PortCheck, type SystemProxy } from "../../lib/api";
  import Icon from "../../components/Icon.svelte";
  import Kbd from "../../components/Kbd.svelte";
  import Stepper from "./Stepper.svelte";
  import Footer from "./Footer.svelte";
  import { SHORTCUTS } from "../../lib/commands";
  import type { IconName } from "../../lib/icons";

  let { onback, onfinish }: { onback: () => void; onfinish: () => void } = $props();

  let sysProxy = $state<SystemProxy | null>(null);
  let bypass = $state(app.settings?.systemProxyBypass ?? "");
  let portText = $state(String(app.settings?.port ?? 9090));
  let listenAll = $state(app.settings?.listenAll ?? false);
  let portCheck = $state<PortCheck | null>(null);
  let busy = $state<string | null>(null);
  let timer: ReturnType<typeof setTimeout> | null = null;

  const intercept = $derived(app.settings?.intercept ?? { mode: "all" as InterceptMode, listed: [], never: [] });
  const port = $derived(parseInt(portText, 10));
  const portValid = $derived(Number.isInteger(port) && port >= 1 && port <= 65535);
  const dirty = $derived(portValid && (port !== app.proxy?.port || listenAll !== app.proxy?.listenAll || !app.proxy?.running));
  const lan = $derived(app.proxy?.lanIp);

  onMount(async () => {
    sysProxy = await api.systemProxy().catch(() => null);
    checkPort();
  });

  function checkPort() {
    if (timer) clearTimeout(timer);
    portCheck = null;
    if (!portValid) return;
    timer = setTimeout(async () => (portCheck = await api.checkPort(port)), 250);
  }

  async function apply() {
    if (!portValid) return;
    busy = "apply";
    try {
      app.proxy = await api.restartProxy(port, listenAll);
      await app.refreshState();
      if (app.proxy.error) app.toast({ kind: "error", title: "Proxy didn't start", body: app.proxy.error }, 7000);
      else app.toast({ kind: "success", title: `Listening on ${app.proxy.addr}` }, 2500);
      sysProxy = await api.systemProxy().catch(() => sysProxy);
      checkPort();
    } finally {
      busy = null;
    }
  }

  async function toggleSysProxy() {
    if (!sysProxy?.supported) return;
    busy = "sys";
    try {
      sysProxy = await api.setSystemProxy(!sysProxy.enabled);
    } catch (e) {
      app.toast({ kind: "error", title: "Couldn't change the system proxy", body: errMsg(e) });
    } finally {
      busy = null;
    }
  }

  async function saveBypass() {
    if (app.settings && bypass !== app.settings.systemProxyBypass) {
      app.settings = await api.setPrefs({ systemProxyBypass: bypass });
      if (sysProxy?.enabled) sysProxy = await api.setSystemProxy(true).catch(() => sysProxy);
    }
  }

  async function setIntercept(patch: Partial<typeof intercept>) {
    if (!app.settings) return;
    const cfg = { ...intercept, ...patch };
    await api.setIntercept(cfg);
    app.settings = { ...app.settings, intercept: cfg };
  }

  function addHost(list: "never" | "listed", host: string) {
    const h = host.trim();
    if (h) setIntercept({ [list]: [...new Set([...intercept[list], h])] });
  }

  const MODES: [InterceptMode, string, string][] = [
    ["all", "All hosts", "Decrypt everything except the never-decrypt list."],
    ["listed", "Only listed hosts", "Tunnel everything else untouched."],
    ["off", "Off", "Inspect plain HTTP only."],
  ];

  const status = $derived.by((): [IconName, string, string, string][] => {
    const sys = app.trust?.find((t) => t.id === "system");
    const ff = app.trust?.find((t) => t.id === "firefox");
    const ok = "var(--success-default)", warn = "var(--warning-default)", off = "var(--text-tertiary)";
    return [
      ["shield-check", "Root CA", sys ? (sys.state === "trusted" ? "Trusted by system" : sys.detail) : "Checking…", sys?.state === "trusted" ? ok : warn],
      ["plug", "Proxy", app.proxy?.error ? "Not running" : app.proxy?.addr ?? "—", app.proxy?.error ? "var(--error-default)" : ok],
      ["monitor", "System proxy", sysProxy == null ? "Checking…" : !sysProxy.supported ? "Not supported here" : sysProxy.enabled ? `On · 127.0.0.1:${sysProxy.port}` : "Off", sysProxy?.enabled ? ok : off],
      ["wifi", "LAN listener", app.proxy?.listenAll ? `${lan ?? "0.0.0.0"}:${app.proxy.port}` : "Off", app.proxy?.listenAll ? ok : off],
      ["globe", "Firefox", ff?.detail ?? "—", ff?.state === "trusted" ? ok : ff?.state === "untrusted" ? warn : off],
    ];
  });
</script>

<svelte:window onkeydown={(e) => { if (e.key === "Enter" && (e.target as HTMLElement).tagName !== "INPUT" && (e.target as HTMLElement).tagName !== "BUTTON") onfinish(); }} />

<div class="wrap">
  <div class="hd">
    <div class="col">
      <h2>Proxy &amp; port</h2>
      <p>How apps reach xwire. Change any of this later from the toolbar's settings button.</p>
    </div>
    <Stepper active={1} onstep={(i) => i === 0 && onback()} />
  </div>

  <div class="body">
    <div class="left">
      <div class="card">
        <div class="row" style="gap:12px">
          <span class="badge"><Icon name="plug" size={16} color="var(--accent-text)" /></span>
          <div class="col">
            <b style="font-size:13.5px">System proxy</b>
            <span class="sub">Routes HTTP and HTTPS from every app that honours the OS proxy. Turned off again when xwire quits.</span>
          </div>
          {#if sysProxy?.enabled}
            <span class="pill" style="background:var(--success-soft);color:var(--success-default)"><span class="dot" style="background:var(--success-default)"></span>On</span>
          {/if}
          <button class="toggle" class:on={sysProxy?.enabled} disabled={!sysProxy?.supported || busy === "sys"} aria-label="System proxy" onclick={toggleSysProxy}></button>
        </div>
        <div style="height:1px;background:var(--border-subtle)"></div>
        <div class="row" style="gap:12px">
          <span class="fl">Bypass</span>
          <label class="input" style="flex:1"><input class="mono" style="font-size:12px" bind:value={bypass} onblur={saveBypass} spellcheck="false" /></label>
        </div>
        <div class="row" style="gap:12px">
          <span class="fl">Applies to</span>
          <span class="sub">{sysProxy ? (sysProxy.supported ? sysProxy.backend : "No supported desktop proxy settings found (GNOME or KDE). Set HTTP_PROXY / HTTPS_PROXY instead.") : "…"}</span>
        </div>
      </div>

      <div class="card">
        <b style="font-size:13.5px">Listener</b>
        <div class="row" style="gap:16px;align-items:flex-start">
          <div class="col" style="flex:none;width:180px;gap:6px">
            <span class="fl">HTTP / HTTPS port</span>
            <label class="input">
              <input class="mono" style="font-size:12px" bind:value={portText} oninput={checkPort} onkeydown={(e) => e.key === "Enter" && apply()} inputmode="numeric" />
              {#if !portValid}
                <span class="hint" style="color:var(--error-default)">invalid</span>
              {:else if portCheck?.free}
                <span class="hint" style="color:var(--success-default)">✓ {portCheck.owner === "xwire" ? "in use by xwire" : "free"}</span>
              {:else if portCheck}
                <span class="hint" style="color:var(--error-default)">in use</span>
              {/if}
            </label>
            {#if portCheck && !portCheck.free}
              <span class="sub" style="font-size:11px">{portText} is used by {portCheck.owner ?? "another program"}</span>
            {/if}
          </div>
          <div class="col" style="gap:6px">
            <span class="fl">Listen on</span>
            <div class="seg">
              <button class:on={!listenAll} onclick={() => (listenAll = false)}><Icon name="monitor" size={13} />This computer only</button>
              <button class:on={listenAll} onclick={() => (listenAll = true)}><Icon name="wifi" size={13} />All interfaces</button>
            </div>
            <span class="sub" style="font-size:11px">{listenAll ? `Phones and other machines can reach ${lan ?? "this computer"}:${portValid ? port : "…"}` : "Only apps on this computer can connect."}</span>
          </div>
        </div>
        <div class="row" style="gap:8px">
          <button class="btn primary sm" disabled={!dirty || busy === "apply" || (portCheck != null && !portCheck.free)} onclick={apply}>{busy === "apply" ? "Restarting…" : "Apply & restart proxy"}</button>
          {#if app.proxy?.error}<span class="sub" style="color:var(--error-default)">{app.proxy.error}</span>{/if}
        </div>
      </div>

      <div class="card">
        <b style="font-size:13.5px">HTTPS interception</b>
        <div class="row" style="gap:10px;align-items:stretch">
          {#each MODES as [m, t, d]}
            <button class="opt" class:on={intercept.mode === m} onclick={() => setIntercept({ mode: m })}>
              <span class="row" style="gap:8px"><span class="radio" class:on={intercept.mode === m}></span><b style="font-size:12.5px;font-weight:500">{t}</b></span>
              <span class="sub">{d}</span>
            </button>
          {/each}
        </div>
        {#each (intercept.mode === "listed" ? ["listed", "never"] : ["never"]) as list}
          {@const l = list as "never" | "listed"}
          <div class="row" style="gap:8px;flex-wrap:wrap">
            <span class="fl">{l === "never" ? "Never decrypt" : "Decrypt only"}</span>
            {#each intercept[l] as h}
              <span class="pill mono" style="background:var(--bg-hover);color:var(--text-secondary)">
                {h}<button class="rm" aria-label="Remove {h}" onclick={() => setIntercept({ [l]: intercept[l].filter((x) => x !== h) })}><Icon name="x" size={10} /></button>
              </span>
            {/each}
            <form class="row" style="gap:6px" onsubmit={(e) => { e.preventDefault(); const inp = e.currentTarget.elements.namedItem("h") as HTMLInputElement; addHost(l, inp.value); inp.value = ""; }}>
              <label class="input" style="height:22px;width:170px"><input name="h" class="mono" style="font-size:11.5px" placeholder="*.example.com" spellcheck="false" /></label>
              <button class="btn sm ghost" type="submit"><Icon name="plus" />Add host</button>
            </form>
          </div>
        {/each}
        <span class="sub" style="font-size:11px">Use this for apps that pin certificates (banking apps, some OS services) — they fail when intercepted.</span>
      </div>
    </div>

    <div class="side">
      <div class="card" style="gap:10px">
        <b style="font-size:13.5px">{app.caTrusted && !app.proxy?.error ? "Ready to capture" : "Almost ready"}</b>
        {#each status as [ic, lab, val, c]}
          <div class="row" style="gap:8px;height:26px">
            <Icon name={ic} color={c} />
            <span style="flex:1;font-size:12.5px">{lab}</span>
            <span class="trunc" style="font-size:11.5px;font-weight:500;max-width:190px" style:color={c}>{val}</span>
          </div>
        {/each}
        {#if !app.caTrusted}
          <button class="btn sm secondary" style="align-self:flex-start" onclick={onback}><Icon name="shield" />Trust the root certificate</button>
        {/if}
      </div>
      <div class="card" style="gap:8px">
        <b style="font-size:12.5px">Keyboard first</b>
        {#each SHORTCUTS.slice(0, 6) as [keys, lab]}
          <div class="row" style="gap:8px"><span style="flex:1;font-size:12px;color:var(--text-secondary)">{lab}</span><Kbd {keys} /></div>
        {/each}
      </div>
    </div>
  </div>

  <Footer {onback}>
    <button class="btn primary" onclick={onfinish}><Icon name="record" />{app.settings?.onboarded ? "Done" : "Finish & start capturing"} <Kbd keys={["↵"]} /></button>
  </Footer>
</div>

<style>
  .wrap {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .hd {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 18px 32px;
    flex: none;
  }
  .col {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  h2 {
    margin: 0 0 4px;
    font-size: 18px;
    font-weight: 600;
  }
  .hd p {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-secondary);
  }
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
    gap: 16px;
    padding: 0 32px 16px;
    overflow: auto;
  }
  .left {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .side {
    width: 360px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .badge {
    width: 32px;
    height: 32px;
    border-radius: 8px;
    background: var(--accent-soft);
    display: grid;
    place-items: center;
    flex: none;
  }
  .sub {
    font-size: 12px;
    color: var(--text-secondary);
    line-height: 1.45;
  }
  .fl {
    width: 110px;
    flex: none;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-secondary);
  }
  .col .fl {
    width: auto;
  }
  .hint {
    font-size: 11px;
    font-weight: 500;
    white-space: nowrap;
  }
  .toggle:disabled {
    opacity: 0.5;
  }
  .opt {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 12px;
    border-radius: 8px;
    border: 1px solid var(--border-default);
    text-align: left;
  }
  .opt.on {
    border-color: var(--accent-default);
    background: var(--accent-soft);
  }
  .opt .sub {
    font-size: 11.5px;
  }
  .rm {
    display: grid;
    place-items: center;
    color: var(--text-tertiary);
  }
  .rm:hover {
    color: var(--error-default);
  }
</style>
