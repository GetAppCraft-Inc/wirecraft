<script lang="ts">
  import { onMount } from "svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { app } from "../../lib/state.svelte";
  import { api, errMsg, type TrustState } from "../../lib/api";
  import { fmtDate } from "../../lib/format";
  import { copyText } from "../../lib/actions";
  import Icon from "../../components/Icon.svelte";
  import Kbd from "../../components/Kbd.svelte";
  import Modal from "../../components/Modal.svelte";
  import Stepper from "./Stepper.svelte";
  import Step from "./Step.svelte";
  import Banner from "./Banner.svelte";
  import Shell from "./Shell.svelte";
  import Footer from "./Footer.svelte";

  let { onback, onnext, onskip }: { onback: () => void; onnext: () => void; onskip: () => void } = $props();

  type Platform = "macOS" | "Windows" | "Linux" | "Firefox" | "Android" | "iOS";
  const PLATFORMS: Platform[] = ["macOS", "Windows", "Linux", "Firefox", "Android", "iOS"];
  const DISTROS: [string, string][] = [["debian", "Debian / Ubuntu"], ["fedora", "Fedora / RHEL"], ["arch", "Arch"], ["opensuse", "openSUSE"]];

  const os = $derived(app.info?.os);
  const native: Platform = app.info?.os.os === "macos" ? "macOS" : app.info?.os.os === "windows" ? "Windows" : "Linux";
  let platform = $state<Platform>(native);
  let distro = $state(["debian", "fedora", "arch", "opensuse"].includes(app.info?.os.family ?? "") ? app.info!.os.family : "arch");
  let manual = $state<Record<string, string[]>>({});
  let busy = $state<string | null>(null);
  let verified = $state<{ ok: boolean; msg: string } | null>(null);
  let confirmRegen = $state(false);
  let qr = $state("");
  let rechecking = $state(false);

  const ca = $derived(app.info?.ca);
  const sys = $derived(app.trust?.find((t) => t.id === "system"));
  const sysTrusted = $derived(sys?.state === "trusted");
  const port = $derived(app.proxy?.port ?? app.settings?.port ?? 9090);
  const lan = $derived(app.proxy?.lanIp ?? "your-computer-ip");
  const lanListening = $derived(app.proxy?.listenAll ?? false);
  const certUrl = $derived(`http://${lan}:${port}/cert.${platform === "Android" ? "der" : "pem"}`);
  const distroName = $derived(DISTROS.find(([k]) => k === distro)?.[1] ?? distro);
  const prettyOs = $derived(os?.prettyName || distroName);

  onMount(async () => {
    manual = await api.manualCommands();
  });

  $effect(() => {
    if (platform === "Android" || platform === "iOS") {
      api.qrSvg(certUrl).then((s) => (qr = s)).catch(() => (qr = ""));
    }
  });

  async function act(key: string, fn: () => Promise<string>, okTitle: string) {
    busy = key;
    try {
      const msg = await fn();
      app.toast({ kind: "success", title: okTitle, body: msg || undefined });
    } catch (e) {
      app.toast({ kind: "error", title: "That didn't work", body: errMsg(e) }, 7000);
    } finally {
      busy = null;
      await recheck();
    }
  }

  async function recheck() {
    rechecking = true;
    await app.refreshTrust();
    rechecking = false;
  }

  async function verify() {
    busy = "verify";
    try {
      verified = { ok: true, msg: await api.verifyInterception() };
    } catch (e) {
      verified = { ok: false, msg: errMsg(e) };
    } finally {
      busy = null;
    }
  }

  async function download(ext: "pem" | "crt" | "der") {
    const path = await save({ defaultPath: `xwire-root.${ext}`, filters: [{ name: "Certificate", extensions: [ext] }] });
    if (!path) return;
    try {
      await api.exportCa(ext === "der" ? "der" : "pem", path);
      app.toast({ kind: "success", title: "Certificate saved", body: path }, 2500);
    } catch (e) {
      app.toast({ kind: "error", title: "Couldn't save", body: errMsg(e) });
    }
  }

  async function regenerate() {
    confirmRegen = false;
    busy = "regen";
    try {
      await api.regenerateCa();
      await app.refreshState();
      app.toast({ kind: "warning", title: "New root CA generated", body: "Trust it again on this machine and every device." }, 6000);
    } catch (e) {
      app.toast({ kind: "error", title: "Couldn't regenerate", body: errMsg(e) });
    } finally {
      busy = null;
      await recheck();
    }
  }

  async function listenAll() {
    busy = "lan";
    try {
      app.proxy = await api.restartProxy(port, true);
      await app.refreshState();
    } catch (e) {
      app.toast({ kind: "error", title: "Couldn't restart the proxy", body: errMsg(e) });
    } finally {
      busy = null;
    }
  }

  function tone(s: TrustState): [string, "shield-check" | "alert" | "minus"] {
    return s === "trusted" ? ["var(--success-default)", "shield-check"] : s === "untrusted" ? ["var(--warning-default)", "alert"] : ["var(--text-tertiary)", "minus"];
  }

  function shortSha(s: string) {
    const p = s.split(":");
    return p.length > 8 ? `${p.slice(0, 5).join(":")}:…:${p.slice(-2).join(":")}` : s;
  }

  const manualFor = $derived(
    platform === "Linux" ? manual[distro] : platform === "macOS" ? manual.macos : platform === "Windows" ? manual.windows : platform === "Firefox" ? manual.firefox : [],
  );
  const canAuto = $derived(platform === native);
</script>

<div class="wrap">
  <div class="hd">
    <div class="col">
      <h2>Install the xwire root certificate</h2>
      <p>xwire signs certificates on the fly with a CA only this machine holds. Trust it once per device.</p>
    </div>
    <Stepper active={0} onstep={(i) => i === 1 && onnext()} />
  </div>

  <div class="body">
    <div class="main">
      <div class="tabs md" style="padding:0 12px">
        {#each PLATFORMS as p}<button class:on={platform === p} onclick={() => (platform = p)}>{p}</button>{/each}
      </div>
      <div class="content">
        {#if platform === "macOS" || platform === "Windows" || platform === "Linux"}
          {#if !canAuto}
            <Banner tone="info" title="You're on {native}" body="These are the steps for another computer running {platform}. Copy the certificate over with the buttons on the right." />
          {:else if sysTrusted}
            <Banner tone="success" title="Root certificate trusted" body="HTTPS from curl, browsers and most native apps on this machine can now be decrypted." />
          {:else}
            <Banner tone="warning" title="HTTPS traffic can't be decrypted yet" body="Requests will show as encrypted CONNECT tunnels or fail until the CA is trusted." />
          {/if}
          {#if platform === "Linux"}
            <div class="row" style="gap:10px">
              <span class="lbl">Distribution</span>
              <div class="seg">
                {#each DISTROS as [k, l]}<button class:on={distro === k} onclick={() => (distro = k)}>{l}</button>{/each}
              </div>
            </div>
          {/if}
          <Step n={1} done={canAuto && sysTrusted}
            title={platform === "macOS" ? "Trust the certificate in the System keychain" : platform === "Windows" ? "Add the certificate to Trusted Root Certification Authorities" : "Add the certificate to the system trust store"}
            body={!canAuto ? "Run the command below on that machine." : platform === "Linux" ? `Detected ${prettyOs}. xwire asks for your password with pkexec and runs the command below.` : platform === "macOS" ? "Asks for your password once. xwire adds the CA and sets “Always Trust”." : "Windows shows a security prompt — choose Yes."}>
            {#if canAuto}
              <div class="row" style="gap:8px">
                {#if sysTrusted}
                  <button class="btn secondary" onclick={() => act("sys", api.installCaSystem, "Certificate installed")}><Icon name="check" />Trusted</button>
                {:else}
                  <button class="btn primary" disabled={busy === "sys"} onclick={() => act("sys", api.installCaSystem, "Certificate installed")}>
                    <Icon name="shield" />{busy === "sys" ? "Waiting for permission…" : platform === "Linux" ? "Install with pkexec" : "Install automatically"}
                  </button>
                {/if}
                {#if platform === "Linux"}
                  <button class="btn secondary" disabled={busy === "nss"} onclick={() => act("nss", () => api.installNss(false), "Added to Chrome / Chromium")}><Icon name="globe" />Also trust in Chrome (NSS)</button>
                {/if}
              </div>
            {/if}
          </Step>
          <Step n={2} title="Or run it yourself" body="Same result, if you'd rather not grant xwire admin rights.">
            {#if manualFor?.length}<Shell cmds={manualFor} />{/if}
          </Step>
          {#if canAuto}
            <Step n={3} done={verified?.ok} title="Verify" body={verified ? (verified.ok ? `Verified: ${verified.msg}.` : verified.msg) : "Fetches https://xwire.check through the proxy with curl, which uses the system trust store."}>
              <button class="btn sm secondary" disabled={busy === "verify"} onclick={verify}><Icon name="repeat" />{busy === "verify" ? "Checking…" : "Run check"}</button>
            </Step>
          {/if}
        {:else if platform === "Firefox"}
          <Banner tone="info" title="Firefox keeps its own certificate store" body="xwire can import the CA into every Firefox profile's NSS database with certutil. Restart Firefox afterwards." />
          <Step n={1} title="Import into each Firefox profile" body="Works for regular, Snap and LibreWolf profiles. Needs certutil (nss tools).">
            <button class="btn primary" disabled={busy === "ff" || os?.os !== "linux"} onclick={() => act("ff", () => api.installNss(true), "Added to Firefox")}><Icon name="check" />{busy === "ff" ? "Importing…" : "Import into Firefox profiles"}</button>
          </Step>
          <Step n={2} title="Or trust the OS store instead" body="In about:config set security.enterprise_roots.enabled to true. Firefox then uses the system trust store." />
          <Step n={3} title="Or run it yourself">
            {#if manualFor?.length}<Shell cmds={manualFor} />{/if}
          </Step>
        {:else}
          {#if !lanListening}
            <div class="lanwarn">
              <Banner tone="warning" title="xwire only listens on this computer" body="Phones can't reach it until it listens on all interfaces." />
              <button class="btn secondary" disabled={busy === "lan"} onclick={listenAll}><Icon name="wifi" />Listen on all interfaces</button>
            </div>
          {/if}
          <div class="mob">
            <div class="qr">
              <div class="qrbox">{#if qr}{@html qr}{/if}</div>
              <span class="mono" style="color:var(--accent-text);font-size:11px">{certUrl}</span>
              <span style="color:var(--text-tertiary);font-size:11.5px">Scan with the camera app</span>
            </div>
            <div class="msteps">
              <div class="proxy">
                <Icon name="wifi" size={16} color="var(--accent-text)" />
                <div class="col" style="flex:1">
                  <span style="font-size:11.5px;color:var(--text-tertiary)">Wi-Fi proxy · Manual</span>
                  <span class="mono" style="font-size:16px;font-weight:700">{lan} : {port}</span>
                </div>
                <button class="icon-btn" title="Copy" onclick={() => copyText(`${lan}:${port}`, "Copied proxy address")}><Icon name="copy" size={16} /></button>
              </div>
              {#if platform === "Android"}
                <Step n={1} title="Join the same Wi-Fi and set the proxy" body="Settings → Network & internet → Internet → (your network) → Proxy → Manual." />
                <Step n={2} title="Scan the QR code and install the certificate" body="Settings → Security & privacy → More → Encryption & credentials → Install a certificate → CA certificate." />
                <Step n={3} title="Let your debug app trust user CAs" body="Android 7+ apps ignore user CAs unless their network security config allows it.">
                  <pre class="xml mono selectable"><span class="kw">&lt;network-security-config&gt;</span>
  <span class="kw">&lt;debug-overrides&gt;</span>
    <span class="kw">&lt;trust-anchors&gt;</span>
      <span class="kw">&lt;certificates </span><span class="k">src</span>=<span class="s">"user"</span><span class="kw"> /&gt;</span>
    <span class="kw">&lt;/trust-anchors&gt;</span>
  <span class="kw">&lt;/debug-overrides&gt;</span>
<span class="kw">&lt;/network-security-config&gt;</span></pre>
                </Step>
              {:else}
                <Step n={1} title="Set the Wi-Fi proxy on your iPhone or iPad" body="Settings → Wi-Fi → ⓘ next to your network → Configure Proxy → Manual." />
                <Step n={2} title="Scan the QR code to download the profile" body="Safari downloads the certificate and offers to install it as a profile → Allow." />
                <Step n={3} title="Install the profile" body="Settings → General → VPN & Device Management → xwire Root CA → Install." />
                <Step n={4} title="Turn on full trust" body="Settings → General → About → Certificate Trust Settings → enable xwire Root CA. iOS needs this extra step." />
              {/if}
            </div>
          </div>
          <Banner tone="info" title="Remove the proxy when you're done" body="Phones keep the manual proxy after xwire quits." />
        {/if}
      </div>
    </div>

    <div class="side">
      {#if ca}
        <div class="card" style="gap:10px">
          <div class="row" style="gap:10px">
            <span class="badge"><Icon name="shield" size={16} color="var(--accent-text)" /></span>
            <div class="col" style="gap:2px">
              <b style="font-size:13px">xwire Root CA</b>
              <span style="font-size:11.5px;color:var(--text-tertiary)">Generated on this machine · never shared</span>
            </div>
          </div>
          {#each [["Common name", ca.commonName], ["Key", ca.keyAlgorithm], ["Valid", `${fmtDate(ca.notBefore)} → ${fmtDate(ca.notAfter)}`], ["SHA-256", shortSha(ca.sha256)]] as [k, v]}
            <div class="row" style="gap:8px"><span class="ck">{k}</span><span class="mono trunc selectable" style="font-size:11px" title={k === "SHA-256" ? ca.sha256 : v}>{v}</span></div>
          {/each}
          <div class="row" style="gap:6px;flex-wrap:wrap">
            <button class="btn sm secondary" onclick={() => download("pem")}><Icon name="download" />.pem</button>
            <button class="btn sm secondary" onclick={() => download("crt")}>.crt</button>
            <button class="btn sm secondary" onclick={() => download("der")}>.der</button>
          </div>
          <div style="height:1px;background:var(--border-subtle)"></div>
          <div class="row" style="gap:6px">
            <button class="btn sm danger" disabled={busy === "regen"} onclick={() => (confirmRegen = true)}><Icon name="repeat" />Regenerate…</button>
            <span class="spacer"></span>
            <button class="dir trunc" title="Show in file manager" onclick={() => revealItemInDir(ca.certPath)}>{ca.dir.replace(/^\/home\/[^/]+/, "~")}</button>
          </div>
        </div>
      {/if}

      <div class="card" style="gap:8px">
        <div class="row">
          <b style="font-size:13px;flex:1">Trust status</b>
          <button class="btn sm ghost" disabled={rechecking} onclick={recheck}><Icon name="repeat" />{rechecking ? "Checking…" : "Re-check"}</button>
        </div>
        {#each app.trust ?? [] as t}
          {@const [c, ic] = tone(t.state)}
          <div class="row" style="gap:8px;height:26px">
            <Icon name={ic} color={c} />
            <span style="flex:1;font-size:12.5px">{t.name}</span>
            <span style="font-size:11.5px;font-weight:500" style:color={c}>{t.detail}</span>
          </div>
        {:else}
          <div style="color:var(--text-tertiary);font-size:12px">Checking…</div>
        {/each}
      </div>
    </div>
  </div>

  <Footer {onback}>
    <span style="font-size:11.5px;color:var(--text-tertiary)">You can reopen this from the toolbar's proxy status.</span>
    <button class="btn secondary" onclick={onskip}>Skip</button>
    <button class="btn primary" onclick={onnext}>Continue <Kbd keys={["↵"]} /></button>
  </Footer>
</div>

{#if confirmRegen}
  <Modal title="Regenerate the root CA?" onclose={() => (confirmRegen = false)}>
    <p style="margin:0;color:var(--text-secondary);line-height:1.5">
      xwire creates a new root certificate and restarts the proxy. Every browser, machine and phone that trusted the old one must trust the new one before HTTPS can be decrypted again.
    </p>
    {#snippet footer()}
      <button class="btn secondary" onclick={() => (confirmRegen = false)}>Cancel</button>
      <button class="btn primary" style="background:var(--error-default)" onclick={regenerate}>Regenerate</button>
    {/snippet}
  </Modal>
{/if}

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
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border-default);
    border-radius: 10px;
    background: var(--bg-panel);
    overflow: hidden;
  }
  .content {
    flex: 1;
    overflow: auto;
    padding: 20px;
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
    overflow: auto;
  }
  .lbl {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-secondary);
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
  .ck {
    width: 92px;
    flex: none;
    font-size: 11.5px;
    color: var(--text-tertiary);
  }
  .dir {
    font-size: 11px;
    color: var(--text-tertiary);
    max-width: 180px;
  }
  .dir:hover {
    color: var(--accent-text);
    text-decoration: underline;
  }
  .mob {
    display: flex;
    gap: 24px;
    align-items: flex-start;
  }
  .qr {
    width: 200px;
    flex: none;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    text-align: center;
    word-break: break-all;
  }
  .qrbox {
    width: 180px;
    height: 180px;
    border-radius: 8px;
    background: #fff;
    overflow: hidden;
  }
  .qrbox :global(svg) {
    width: 100%;
    height: 100%;
  }
  .msteps {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .proxy {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: 8px;
    background: var(--bg-sunken);
    border: 1px solid var(--border-default);
  }
  .lanwarn {
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-start;
  }
  .lanwarn :global(.banner) {
    align-self: stretch;
  }
  .xml {
    margin: 0;
    padding: 8px 10px;
    background: var(--bg-sunken);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    font-size: 11px;
    line-height: 17px;
    width: 100%;
  }
  .xml .kw {
    color: var(--syntax-keyword);
  }
  .xml .k {
    color: var(--syntax-key);
  }
  .xml .s {
    color: var(--syntax-string);
  }
</style>
