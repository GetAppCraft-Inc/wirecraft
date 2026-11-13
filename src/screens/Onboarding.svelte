<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import Logo from "../components/Logo.svelte";
  import WindowControls from "../components/WindowControls.svelte";
  import Welcome from "./onboarding/Welcome.svelte";
  import RootCa from "./onboarding/RootCa.svelte";
  import ProxyPort from "./onboarding/ProxyPort.svelte";

  async function finish() {
    if (app.settings && !app.settings.onboarded) app.settings = await api.setPrefs({ onboarded: true });
    app.screen = "workspace";
    app.refreshTrust();
  }
  function go(step: number) {
    app.onboardingStep = step;
  }
</script>

<svelte:window onkeydown={(e) => { if (e.key === "Escape" && app.settings?.onboarded) finish(); }} />

<div class="win">
  <header class="tb" data-tauri-drag-region>
    <Logo size={16} />
    <span class="sub" data-tauri-drag-region>{app.settings?.onboarded ? "Settings" : "Setup"}</span>
    <span class="spacer" data-tauri-drag-region></span>
    {#if app.settings?.onboarded}
      <button class="btn sm ghost" onclick={finish}>Back to capture</button>
    {/if}
    <WindowControls />
  </header>
  {#if app.onboardingStep === 0}
    <Welcome onnext={() => go(1)} onskip={finish} />
  {:else if app.onboardingStep === 1}
    <RootCa onback={() => go(0)} onnext={() => go(2)} onskip={() => go(2)} />
  {:else}
    <ProxyPort onback={() => go(1)} onfinish={finish} />
  {/if}
</div>

<style>
  .win {
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg-app);
  }
  .tb {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 40px;
    padding: 0 12px;
    background: var(--bg-raised);
    border-bottom: 1px solid var(--border-default);
    flex: none;
  }
  .sub {
    font-size: 12px;
    color: var(--text-tertiary);
  }
</style>
