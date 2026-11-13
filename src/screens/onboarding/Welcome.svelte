<script lang="ts">
  import { app } from "../../lib/state.svelte";
  import { fmtDuration, methodColor, statusColor } from "../../lib/format";
  import Icon from "../../components/Icon.svelte";
  import Kbd from "../../components/Kbd.svelte";
  import Logo from "../../components/Logo.svelte";
  import type { IconName } from "../../lib/icons";

  let { onnext, onskip }: { onnext: () => void; onskip: () => void } = $props();

  const STEPS: [IconName, string, string, string][] = [
    ["shield", "Install the xwire root certificate", "So HTTPS can be decrypted on this machine and your devices.", "2 min"],
    ["plug", "Pick a port and turn on the system proxy", "xwire listens locally and restores your settings on quit.", "30 s"],
    ["phone", "Connect phones and simulators", "Optional. Scan a QR code — no cables or USB debugging.", "optional"],
  ];

  // the most recent real traffic, if any arrived while this screen is open
  const recent = $derived.by(() => {
    void app.slowVersion;
    return app.order.slice(-6).map((id) => app.byId.get(id)!).filter(Boolean);
  });
  const addr = $derived(app.proxy?.addr ?? `127.0.0.1:${app.settings?.port ?? 9090}`);
</script>

<svelte:window onkeydown={(e) => { if (e.key === "Enter" && (e.target as HTMLElement).tagName !== "BUTTON") onnext(); }} />

<div class="welcome">
  <div class="left">
    <span class="pill" style="background:var(--accent-soft);color:var(--accent-text);font-size:10.5px;align-self:flex-start">OPEN SOURCE · NATIVE RUST · MIT</span>
    <h1>See every request your apps make.</h1>
    <p class="lede">
      xwire is a native HTTPS debugging proxy. Inspect and replay HTTP/1.1, HTTP/2 and WebSocket traffic from desktop apps, browsers and phones.
    </p>
    <div class="steps">
      {#each STEPS as [ic, t, d, k]}
        <div class="s">
          <span class="ic"><Icon name={ic} size={16} color="var(--accent-text)" /></span>
          <div class="tx"><div class="t">{t}</div><div class="d">{d}</div></div>
          <span class="k">{k}</span>
        </div>
      {/each}
    </div>
    <div class="act">
      <button class="btn primary lg" onclick={onnext}>Set up HTTPS interception <Kbd keys={["↵"]} /></button>
      <button class="btn ghost lg" onclick={onskip}>Skip for now</button>
    </div>
    <div class="meta">
      <span>v{app.info?.version}</span><span>Telemetry off</span><span>Exports .har</span>
    </div>
  </div>

  <div class="art">
    <div class="flow">
      <div class="node"><Icon name="monitor" size={20} color="var(--text-secondary)" /><b>This computer</b><span class="mono">{app.info?.os.prettyName || "Desktop apps"}</span></div>
      <span class="wire"></span>
      <div class="node on"><Logo size={22} wordmark={false} /><b>xwire</b><span class="mono">{addr}</span></div>
      <span class="wire"></span>
      <div class="node"><Icon name="globe" size={20} color="var(--text-secondary)" /><b>Internet</b><span class="mono">TLS 1.3 · h2</span></div>
    </div>
    <div class="stream">
      {#if recent.length}
        {#each recent as f (f.id)}
          <div class="sr" class:err={(f.status ?? 0) >= 500 || f.state === "error"}>
            <span class="mono b" style:color={methodColor(f.method)}>{f.method}</span>
            <span class="mono st" style:color={statusColor(f.status)}>{f.status ?? "···"}</span>
            <span class="mono u trunc">{f.host}{f.method === "CONNECT" ? "" : f.path}</span>
            <span class="mono d">{fmtDuration(f.durationMs)}</span>
          </div>
        {/each}
      {:else}
        <div class="waiting">
          <span class="dot" style="background:var(--success-default)"></span>
          Listening on <span class="mono">{addr}</span> — traffic will stream in here.
        </div>
      {/if}
    </div>
    <div class="foot">Everything stays on this machine. xwire never uploads captured traffic.</div>
  </div>
</div>

<style>
  .welcome {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 64px;
    padding: 24px clamp(24px, 6vw, 112px);
    overflow: auto;
  }
  .left {
    width: 540px;
    flex: 0 1 540px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }
  h1 {
    margin: 0;
    font-size: 34px;
    line-height: 1.18;
    letter-spacing: -0.6px;
    font-weight: 700;
  }
  .lede {
    margin: 0;
    font-size: 14px;
    line-height: 1.55;
    color: var(--text-secondary);
  }
  .steps {
    border: 1px solid var(--border-default);
    border-radius: 10px;
    background: var(--bg-panel);
    overflow: hidden;
  }
  .s {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
  }
  .s + .s {
    border-top: 1px solid var(--border-subtle);
  }
  .ic {
    width: 32px;
    height: 32px;
    border-radius: 8px;
    background: var(--bg-raised);
    border: 1px solid var(--border-default);
    display: grid;
    place-items: center;
    flex: none;
  }
  .tx {
    flex: 1;
  }
  .t {
    font-size: 13px;
    font-weight: 500;
  }
  .d {
    font-size: 12px;
    color: var(--text-secondary);
  }
  .k {
    font-size: 11px;
    color: var(--text-tertiary);
  }
  .act {
    display: flex;
    gap: 10px;
  }
  .meta {
    display: flex;
    gap: 14px;
    font-size: 11.5px;
    color: var(--text-tertiary);
  }
  .art {
    width: 560px;
    flex: 0 1 560px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 24px;
    border-radius: 14px;
    background: var(--bg-panel);
    border: 1px solid var(--border-default);
  }
  .flow {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .node {
    width: 136px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 12px 10px;
    border-radius: 10px;
    background: var(--bg-raised);
    border: 1px solid var(--border-default);
    min-width: 0;
  }
  .node.on {
    background: var(--accent-soft);
    border-color: var(--accent-default);
  }
  .node b {
    font-size: 12.5px;
    font-weight: 600;
  }
  .node span {
    font-size: 10.5px;
    color: var(--text-tertiary);
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .wire {
    flex: 1;
    max-width: 56px;
    height: 2px;
    background: var(--accent-default);
  }
  .stream {
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    background: var(--bg-sunken);
    overflow: hidden;
    min-height: 168px;
  }
  .sr {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 28px;
    padding: 0 12px;
    border-bottom: 1px solid var(--border-subtle);
    font-size: 11.5px;
  }
  .sr.err {
    background: var(--row-error);
  }
  .sr .b {
    font-weight: 700;
    font-size: 11px;
    width: 52px;
  }
  .st {
    width: 30px;
  }
  .u {
    flex: 1;
    color: var(--text-secondary);
  }
  .sr.err .u {
    color: var(--error-default);
  }
  .d {
    font-size: 11px;
    color: var(--text-tertiary);
  }
  .waiting {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    height: 168px;
    color: var(--text-secondary);
    font-size: 12px;
  }
  .foot {
    font-size: 11.5px;
    color: var(--text-tertiary);
  }
</style>
