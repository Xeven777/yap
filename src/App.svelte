<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { emit, listen } from "@tauri-apps/api/event";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { register, unregister } from "@tauri-apps/plugin-global-shortcut";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { onMount, onDestroy } from "svelte";
  import {
    Mic,
    Settings,
    Minus,
    Cloud,
    HardDrive,
    Globe,
    Keyboard,
    KeyRound,
    Sparkles,
    Download,
    Trash2,
    X,
    Check,
    LoaderCircle,
    TriangleAlert,
    CircleCheck,
    CircleAlert,
    ArrowLeft,
  } from "@lucide/svelte";
  import "./app.css";

  type Status = "idle" | "recording" | "transcribing" | "done" | "error";

  interface ModelStatus {
    id: string;
    label: string;
    filename: string;
    size_bytes: number;
    recommended: boolean;
    downloaded: boolean;
  }

  interface DownloadProgressEvent {
    model_id: string;
    downloaded: number;
    total: number;
  }

  let status: Status = $state("idle");
  let transcript = $state("");
  let apiKey = $state("");
  let language = $state("");
  let hotkey = $state("Ctrl+Shift+Space");
  let activeHotkey = $state("");
  let showSettings = $state(false);
  let errorMsg = $state("");

  let backend = $state("groq");
  let activeModel = $state("ggml-large-v3-turbo-q5_0");
  let models: ModelStatus[] = $state([]);
  let downloadingModelId = $state<string | null>(null);
  let downloadProgress = $state<{ downloaded: number; total: number } | null>(null);
  let downloadError = $state("");

  let win: ReturnType<typeof getCurrentWebviewWindow> | null = null;
  let unlistenProgress: (() => void) | null = null;
  let unlistenDone: (() => void) | null = null;
  let unlistenError: (() => void) | null = null;

  $effect(() => {
    emit("yap://state", status);
  });

  let statusLabel = $derived<Record<Status, string>>({
    idle: "Ready",
    recording: "Recording",
    transcribing: backend === "local" ? "Transcribing locally" : "Transcribing",
    done: "Copied to clipboard",
    error: "Something went wrong",
  });

  let statusSubtitle = $derived<Record<Status, string>>({
    idle: `Press ${activeHotkey || hotkey} to start`,
    recording: "Press the hotkey again to stop",
    transcribing: "Just a moment…",
    done: "Pasted into the focused field",
    error: errorMsg || "Try again",
  });

  async function shortcutHandler(event: { state: string }) {
    if (event.state !== "Pressed") return;
    if (status === "idle") await startRecording();
    else if (status === "recording") await stopRecording();
  }

  async function loadModels() {
    models = await invoke<ModelStatus[]>("list_models");
  }

  onMount(async () => {
    win = getCurrentWebviewWindow();
    apiKey = await invoke<string>("get_api_key");
    language = await invoke<string>("get_language");
    hotkey = await invoke<string>("get_hotkey");
    backend = await invoke<string>("get_backend");
    activeModel = await invoke<string>("get_active_model");
    await loadModels();

    try {
      await register(hotkey, shortcutHandler);
      activeHotkey = hotkey;
    } catch (e) {
      errorMsg = `Shortcut failed: ${e}`;
    }

    unlistenProgress = await listen<DownloadProgressEvent>("download_progress", (e) => {
      downloadingModelId = e.payload.model_id;
      downloadProgress = { downloaded: e.payload.downloaded, total: e.payload.total };
    });

    unlistenDone = await listen<{ model_id: string }>("download_done", async () => {
      downloadingModelId = null;
      downloadProgress = null;
      downloadError = "";
      await loadModels();
    });

    unlistenError = await listen<{ model_id: string; error: string }>("download_error", (e) => {
      if (e.payload.error !== "Cancelled") {
        downloadError = `Download failed: ${e.payload.error}`;
      }
      downloadingModelId = null;
      downloadProgress = null;
    });
  });

  onDestroy(async () => {
    if (activeHotkey) await unregister(activeHotkey).catch(() => {});
    unlistenProgress?.();
    unlistenDone?.();
    unlistenError?.();
  });

  async function startRecording() {
    status = "recording";
    transcript = "";
    errorMsg = "";
    try {
      await invoke("start_recording");
    } catch (e) {
      status = "error";
      errorMsg = String(e);
      setTimeout(() => (status = "idle"), 3000);
    }
  }

  async function stopRecording() {
    status = "transcribing";
    try {
      transcript = await invoke<string>("stop_recording", {
        apiKey,
        language: language || null,
        backend,
        modelId: activeModel,
      });
      status = "done";
      await writeText(transcript);
      await new Promise((r) => setTimeout(r, 80));
      await invoke("paste_text");
    } catch (e) {
      status = "error";
      errorMsg = String(e);
    } finally {
      setTimeout(() => {
        if (status !== "recording") status = "idle";
      }, 3000);
    }
  }

  async function saveSettings() {
    if (hotkey !== activeHotkey) {
      try {
        if (activeHotkey) await unregister(activeHotkey);
        await register(hotkey, shortcutHandler);
        await invoke("save_hotkey", { hotkey });
        activeHotkey = hotkey;
      } catch (e) {
        errorMsg = `'${hotkey}' not recognised — reverted.`;
        hotkey = activeHotkey;
        return;
      }
    }
    await invoke("save_api_key", { key: apiKey });
    await invoke("save_language", { language });
    await invoke("save_backend", { backend });
    await invoke("save_active_model", { modelId: activeModel });
    showSettings = false;
  }

  async function startDownload(modelId: string) {
    downloadError = "";
    try {
      await invoke("download_model", { modelId });
    } catch (e) {
      if (String(e) !== "Cancelled") downloadError = String(e);
    }
  }

  async function cancelDownload() { await invoke("cancel_download"); }

  async function deleteModel(modelId: string) {
    await invoke("delete_model", { modelId });
    await loadModels();
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const gb = bytes / 1_073_741_824;
    if (gb >= 1) return gb.toFixed(1) + " GB";
    const mb = bytes / 1_048_576;
    if (mb >= 1) return mb.toFixed(0) + " MB";
    return (bytes / 1024).toFixed(0) + " KB";
  }

  function progressPct(downloaded: number, total: number): number {
    if (!total) return 0;
    return Math.round((downloaded / total) * 100);
  }

  let activeModelDownloaded = $derived(
    models.find((m) => m.id === activeModel)?.downloaded ?? false
  );
  let localReady = $derived(backend === "local" && activeModelDownloaded);
  let groqReady = $derived(backend === "groq" && !!apiKey);
  let canRecord = $derived(localReady || groqReady);

  function formatHotkey(s: string): string[] {
    return s.split("+").map((p) => p.trim()).filter(Boolean);
  }
</script>

<main>
  <header class="topbar">
    <div class="brand">
      <div class="brand-mark">
        <Mic size={14} strokeWidth={2.4} />
      </div>
      <div class="brand-text">
        <span class="brand-name">Yap</span>
        <span class="brand-sub">Voice → text</span>
      </div>
    </div>
    <div class="topbar-actions">
      <button
        class="icon-btn {showSettings ? 'is-active' : ''}"
        onclick={() => (showSettings = !showSettings)}
        title="Settings"
        aria-label="Settings"
      >
        {#if showSettings}
          <ArrowLeft size={15} strokeWidth={2} />
        {:else}
          <Settings size={15} strokeWidth={2} />
        {/if}
      </button>
      <button
        class="icon-btn"
        onclick={() => win?.hide()}
        title="Hide to tray"
        aria-label="Hide"
      >
        <Minus size={15} strokeWidth={2.4} />
      </button>
    </div>
  </header>

  {#if !showSettings}
    <section class="hero {status}" class:disabled={!canRecord} class:idle={status === 'idle'}>
      <div class="hero-glow"></div>

      <div class="hero-icon">
        {#if status === "recording"}
          <div class="wave">
            <span></span><span></span><span></span><span></span><span></span>
          </div>
        {:else if status === "transcribing"}
          <LoaderCircle size={26} strokeWidth={2.2} class="spin" />
        {:else if status === "done"}
          <CircleCheck size={26} strokeWidth={2} />
        {:else if status === "error"}
          <CircleAlert size={26} strokeWidth={2} />
        {:else}
          <Mic size={26} strokeWidth={2} />
        {/if}
      </div>

      <div class="hero-text">
        <div class="hero-title">{statusLabel[status]}</div>
        <div class="hero-sub">{statusSubtitle[status]}</div>
      </div>

      {#if status === "idle" && activeHotkey}
        <div class="kbd-row">
          {#each formatHotkey(activeHotkey) as key, i}
            {#if i > 0}<span class="kbd-plus">+</span>{/if}
            <kbd>{key}</kbd>
          {/each}
        </div>
      {/if}
    </section>

    {#if !canRecord}
      <div class="notice">
        <div class="notice-icon"><TriangleAlert size={14} strokeWidth={2.2} /></div>
        <div class="notice-body">
          {#if backend === "groq"}
            <strong>Groq API key required.</strong>
            <span>Add one in Settings to start transcribing.</span>
          {:else}
            <strong>No model downloaded.</strong>
            <span>Open Settings → Local Whisper to grab one.</span>
          {/if}
        </div>
        <button class="notice-cta" onclick={() => (showSettings = true)}>
          Open
        </button>
      </div>
    {/if}

    {#if transcript}
      <div class="transcript">
        <div class="transcript-head">
          <span class="transcript-label">Last transcript</span>
          <span class="transcript-meta">{transcript.length} chars</span>
        </div>
        <div class="transcript-body">{transcript}</div>
      </div>
    {/if}

    {#if errorMsg && status === "error"}
      <div class="err">
        <CircleAlert size={13} strokeWidth={2.2} />
        <span>{errorMsg}</span>
      </div>
    {/if}
  {:else}
    <section class="settings">
      <div class="setting-block">
        <div class="block-head">
          <span class="block-title">Backend</span>
          <span class="block-hint">Where audio gets transcribed</span>
        </div>
        <div class="segmented">
          <button
            class="seg-btn {backend === 'groq' ? 'is-active' : ''}"
            onclick={() => (backend = "groq")}
          >
            <Cloud size={14} strokeWidth={2} />
            <span>Groq</span>
            <em>cloud</em>
          </button>
          <button
            class="seg-btn {backend === 'local' ? 'is-active' : ''}"
            onclick={() => (backend = "local")}
          >
            <HardDrive size={14} strokeWidth={2} />
            <span>Whisper</span>
            <em>local</em>
          </button>
        </div>
      </div>

      {#if backend === "groq"}
        <div class="setting-block">
          <div class="block-head">
            <span class="block-title"><KeyRound size={12} strokeWidth={2.2} /> Groq API key</span>
            <a class="block-link" href="https://console.groq.com/keys" target="_blank" rel="noreferrer">Get one ↗</a>
          </div>
          <input
            type="password"
            bind:value={apiKey}
            placeholder="gsk_…"
            autocomplete="off"
            class="input"
          />
        </div>
      {:else}
        <div class="setting-block">
          <div class="block-head">
            <span class="block-title"><Sparkles size={12} strokeWidth={2.2} /> Models</span>
            <span class="block-hint">Stored in app data folder</span>
          </div>

          {#if downloadError}
            <div class="err">
              <CircleAlert size={13} strokeWidth={2.2} />
              <span>{downloadError}</span>
            </div>
          {/if}

          <div class="model-list">
            {#each models as model (model.id)}
              {@const isDownloading = downloadingModelId === model.id}
              {@const isActive = activeModel === model.id}
              <div class="model-card" class:is-active={isActive && model.downloaded} class:is-downloading={isDownloading}>
                <div class="model-info">
                  <div class="model-name-row">
                    <span class="model-name">{model.label}</span>
                    {#if model.recommended}
                      <span class="badge badge-recommended" title="Recommended">
                        <Sparkles size={9} strokeWidth={2.4} /> Best
                      </span>
                    {/if}
                    {#if isActive && model.downloaded}
                      <span class="badge badge-active">
                        <Check size={9} strokeWidth={3} /> In use
                      </span>
                    {:else if model.downloaded}
                      <span class="badge badge-ready">Ready</span>
                    {/if}
                  </div>
                  <div class="model-meta">{formatBytes(model.size_bytes)}</div>

                  {#if isDownloading && downloadProgress}
                    <div class="progress-wrap">
                      <div class="progress-track">
                        <div
                          class="progress-fill"
                          style="width: {progressPct(downloadProgress.downloaded, downloadProgress.total)}%"
                        ></div>
                      </div>
                      <span class="progress-label">
                        {progressPct(downloadProgress.downloaded, downloadProgress.total)}%
                        · {formatBytes(downloadProgress.downloaded)} / {formatBytes(downloadProgress.total)}
                      </span>
                    </div>
                  {/if}
                </div>

                <div class="model-actions">
                  {#if isDownloading}
                    <button class="btn btn-danger" onclick={cancelDownload}>
                      <X size={11} strokeWidth={2.4} />
                      Cancel
                    </button>
                  {:else if model.downloaded}
                    {#if !isActive}
                      <button class="btn btn-primary-sm" onclick={() => (activeModel = model.id)}>
                        Use
                      </button>
                    {/if}
                    <button
                      class="btn btn-icon btn-delete"
                      onclick={() => deleteModel(model.id)}
                      title="Delete model"
                      aria-label="Delete model"
                    >
                      <Trash2 size={12} strokeWidth={2} />
                    </button>
                  {:else}
                    <button
                      class="btn btn-ghost"
                      onclick={() => startDownload(model.id)}
                      disabled={!!downloadingModelId}
                    >
                      <Download size={11} strokeWidth={2.2} />
                      Download
                    </button>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        </div>
      {/if}

      <div class="setting-block">
        <div class="block-head">
          <span class="block-title"><Globe size={12} strokeWidth={2.2} /> Language</span>
          <span class="block-hint">Blank = auto-detect</span>
        </div>
        <input
          type="text"
          bind:value={language}
          placeholder="en, fr, de…"
          maxlength="5"
          class="input"
        />
      </div>

      <div class="setting-block">
        <div class="block-head">
          <span class="block-title"><Keyboard size={12} strokeWidth={2.2} /> Hotkey</span>
          <span class="block-hint">Global shortcut</span>
        </div>
        <input
          type="text"
          bind:value={hotkey}
          placeholder="Ctrl+Shift+Space"
          class="input"
        />
      </div>

      <div class="settings-actions">
        <button class="btn btn-secondary" onclick={() => (showSettings = false)}>Cancel</button>
        <button class="btn btn-primary" onclick={saveSettings}>
          <Check size={13} strokeWidth={2.4} />
          Save changes
        </button>
      </div>
    </section>
  {/if}

  <footer class="tray-hint">Closing this window hides Yap to the system tray</footer>
</main>

<style>
  :global(:root) {
    --font-sans: 'Geist', -apple-system, BlinkMacSystemFont, system-ui, sans-serif;
    --font-mono: 'Geist Mono', 'SF Mono', ui-monospace, Menlo, monospace;
    --ease-out-quint: cubic-bezier(0.22, 1, 0.36, 1);
    --ease-out-quart: cubic-bezier(0.25, 1, 0.5, 1);
  }

  :global(html, body) {
    margin: 0;
    padding: 0;
    background:
      radial-gradient(1200px 600px at 0% -10%, rgba(10, 132, 255, 0.10), transparent 60%),
      radial-gradient(900px 500px at 110% 110%, rgba(191, 90, 242, 0.07), transparent 60%),
      #0c0c0f;
    color: #f5f5f7;
    font-family: var(--font-sans);
    font-feature-settings: 'ss01', 'cv11';
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
    height: 100%;
  }

  main {
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 100vh;
    animation: appEnter 0.5s var(--ease-out-quint);
  }

  @keyframes appEnter {
    from { opacity: 0; transform: translateY(4px); }
    to { opacity: 1; transform: translateY(0); }
  }

  /* ---------- Top bar ---------- */
  .topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 2px 4px 0;
  }
  .brand { display: flex; align-items: center; gap: 9px; }
  .brand-mark {
    width: 26px; height: 26px;
    display: grid; place-items: center;
    border-radius: 8px;
    background: linear-gradient(140deg, #0A84FF, #5E5CE6);
    color: white;
    box-shadow:
      0 4px 12px rgba(10, 132, 255, 0.35),
      inset 0 1px 0 rgba(255, 255, 255, 0.25);
  }
  .brand-text { display: flex; flex-direction: column; line-height: 1; }
  .brand-name {
    font-size: 17px;
    font-weight: 400;
    font-style: italic;
    color: rgba(255, 255, 255, 0.95);
    letter-spacing: -0.3px;
    line-height: 1;
  }
  .brand-sub {
    font-family: var(--font-mono);
    font-size: 9px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.32);
    margin-top: 3px;
    letter-spacing: 0.6px;
    text-transform: uppercase;
  }
  .topbar-actions { display: flex; gap: 4px; }

  .icon-btn {
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.06);
    cursor: pointer;
    width: 28px; height: 28px;
    border-radius: 8px;
    display: grid; place-items: center;
    color: rgba(255, 255, 255, 0.55);
    transition: all 0.15s ease;
  }
  .icon-btn:hover {
    background: rgba(255, 255, 255, 0.09);
    color: rgba(255, 255, 255, 0.95);
    border-color: rgba(255, 255, 255, 0.12);
  }
  .icon-btn.is-active {
    background: rgba(10, 132, 255, 0.15);
    border-color: rgba(10, 132, 255, 0.35);
    color: #7eb6ff;
  }

  /* ---------- Hero status card ---------- */
  .hero {
    position: relative;
    overflow: hidden;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 18px 16px;
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.04), rgba(255, 255, 255, 0.02));
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 14px;
    transition:
      border-color 0.45s var(--ease-out-quart),
      background 0.45s var(--ease-out-quart);
    animation: heroEnter 0.6s var(--ease-out-quint) both;
  }
  @keyframes heroEnter {
    from { opacity: 0; transform: translateY(8px) scale(0.985); }
    to { opacity: 1; transform: translateY(0) scale(1); }
  }
  .hero-glow {
    position: absolute; inset: -40%;
    background: radial-gradient(circle at 20% 30%, rgba(10, 132, 255, 0.18), transparent 55%);
    opacity: 0.6;
    pointer-events: none;
    transition: opacity 0.3s ease, background 0.3s ease;
  }
  .hero.recording {
    border-color: rgba(255, 69, 58, 0.28);
    background: linear-gradient(180deg, rgba(60, 12, 12, 0.55), rgba(40, 8, 8, 0.25));
  }
  .hero.recording .hero-glow {
    background: radial-gradient(circle at 20% 30%, rgba(255, 69, 58, 0.28), transparent 55%);
    opacity: 1;
    animation: heroGlow 1.6s ease-in-out infinite;
  }
  .hero.transcribing {
    border-color: rgba(10, 132, 255, 0.3);
    background: linear-gradient(180deg, rgba(10, 24, 56, 0.55), rgba(8, 16, 40, 0.25));
  }
  .hero.transcribing .hero-glow {
    background: radial-gradient(circle at 20% 30%, rgba(10, 132, 255, 0.3), transparent 55%);
    opacity: 1;
  }
  .hero.done {
    border-color: rgba(48, 209, 88, 0.28);
    background: linear-gradient(180deg, rgba(12, 44, 26, 0.55), rgba(8, 30, 18, 0.25));
  }
  .hero.done .hero-glow {
    background: radial-gradient(circle at 20% 30%, rgba(48, 209, 88, 0.22), transparent 55%);
    opacity: 1;
  }
  .hero.error {
    border-color: rgba(255, 69, 58, 0.28);
    background: linear-gradient(180deg, rgba(60, 12, 12, 0.45), rgba(40, 8, 8, 0.2));
  }
  .hero.disabled { opacity: 0.78; }

  @keyframes heroGlow {
    0%, 100% { opacity: 0.6; }
    50% { opacity: 1; }
  }

  .hero-icon {
    position: relative;
    width: 48px; height: 48px;
    border-radius: 12px;
    display: grid; place-items: center;
    color: rgba(255, 255, 255, 0.85);
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.08);
    flex-shrink: 0;
  }
  .hero.recording .hero-icon {
    color: #FF6B63;
    background: rgba(255, 69, 58, 0.12);
    border-color: rgba(255, 69, 58, 0.3);
    box-shadow: 0 0 0 0 rgba(255, 69, 58, 0.5);
    animation: pulseRing 1.6s ease-out infinite;
  }
  .hero.transcribing .hero-icon { color: #7eb6ff; background: rgba(10, 132, 255, 0.14); border-color: rgba(10, 132, 255, 0.32); }
  .hero.done .hero-icon { color: #6BE192; background: rgba(48, 209, 88, 0.14); border-color: rgba(48, 209, 88, 0.32); }
  .hero.error .hero-icon { color: #FF6B63; background: rgba(255, 69, 58, 0.12); border-color: rgba(255, 69, 58, 0.3); }

  @keyframes pulseRing {
    0% { box-shadow: 0 0 0 0 rgba(255, 69, 58, 0.45); }
    100% { box-shadow: 0 0 0 14px rgba(255, 69, 58, 0); }
  }

  :global(.spin) { animation: spin 0.9s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }

  .wave {
    display: flex; align-items: center; gap: 3px; height: 22px;
  }
  .wave span {
    width: 3px; border-radius: 2px;
    background: #FF6B63;
    animation: wave 1s ease-in-out infinite;
  }
  .wave span:nth-child(1) { height: 40%; animation-delay: -0.4s; }
  .wave span:nth-child(2) { height: 75%; animation-delay: -0.2s; }
  .wave span:nth-child(3) { height: 100%; animation-delay: 0s; }
  .wave span:nth-child(4) { height: 70%; animation-delay: -0.3s; }
  .wave span:nth-child(5) { height: 45%; animation-delay: -0.1s; }
  @keyframes wave {
    0%, 100% { transform: scaleY(0.5); }
    50% { transform: scaleY(1); }
  }

  .hero-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    z-index: 1;
  }
  .hero-title {
    font-size: 15px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.96);
    letter-spacing: -0.3px;
  }
  .hero-sub {
    font-size: 11.5px;
    color: rgba(255, 255, 255, 0.48);
    letter-spacing: -0.05px;
    font-weight: 450;
  }
  .hero.idle .hero-icon {
    animation: breathe 3.6s var(--ease-out-quint) infinite;
  }
  @keyframes breathe {
    0%, 100% { transform: scale(1); opacity: 1; }
    50% { transform: scale(1.04); opacity: 0.92; }
  }

  .kbd-row {
    display: flex; align-items: center; gap: 3px;
    z-index: 1;
  }
  .kbd-plus {
    color: rgba(255, 255, 255, 0.3);
    font-size: 10px;
  }
  kbd {
    display: inline-flex; align-items: center; justify-content: center;
    min-width: 22px; height: 22px;
    padding: 0 7px;
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.10), rgba(255, 255, 255, 0.04));
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-bottom-color: rgba(255, 255, 255, 0.06);
    border-radius: 6px;
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.88);
    box-shadow:
      inset 0 -1px 0 rgba(0, 0, 0, 0.3),
      inset 0 1px 0 rgba(255, 255, 255, 0.06);
    letter-spacing: 0;
    transition: transform 0.18s var(--ease-out-quart), box-shadow 0.18s var(--ease-out-quart);
    animation: kbdEnter 0.5s var(--ease-out-quint) both;
  }
  .kbd-row:hover kbd {
    transform: translateY(-1px);
    box-shadow:
      inset 0 -2px 0 rgba(0, 0, 0, 0.35),
      inset 0 1px 0 rgba(255, 255, 255, 0.08),
      0 2px 8px rgba(0, 0, 0, 0.35);
  }
  .kbd-row kbd:nth-child(1) { animation-delay: 0.05s; }
  .kbd-row kbd:nth-child(3) { animation-delay: 0.10s; }
  .kbd-row kbd:nth-child(5) { animation-delay: 0.15s; }
  @keyframes kbdEnter {
    from { opacity: 0; transform: translateY(-3px); }
    to { opacity: 1; transform: translateY(0); }
  }

  /* ---------- Notice / Error ---------- */
  .notice {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    background:
      linear-gradient(180deg, rgba(255, 159, 10, 0.08), rgba(255, 159, 10, 0.04));
    border: 1px solid rgba(255, 159, 10, 0.22);
    border-radius: 11px;
    font-size: 12px;
    color: rgba(255, 220, 160, 0.95);
    line-height: 1.4;
    animation: noticeEnter 0.55s var(--ease-out-quint) 0.15s both;
  }
  @keyframes noticeEnter {
    from { opacity: 0; transform: translateY(6px); }
    to { opacity: 1; transform: translateY(0); }
  }
  .notice-icon {
    width: 24px; height: 24px;
    border-radius: 7px;
    display: grid; place-items: center;
    background: rgba(255, 159, 10, 0.14);
    color: #FFB84D;
    flex-shrink: 0;
  }
  .notice-body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .notice-body strong { color: rgba(255, 232, 192, 0.98); font-weight: 600; font-size: 12px; }
  .notice-body span { color: rgba(255, 220, 160, 0.7); font-size: 11.5px; }
  .notice-cta {
    background: rgba(255, 159, 10, 0.18);
    border: 1px solid rgba(255, 159, 10, 0.3);
    color: #FFC880;
    border-radius: 7px;
    padding: 5px 10px;
    font-size: 11.5px; font-weight: 600;
    cursor: pointer;
    flex-shrink: 0;
    transition: background 0.15s;
  }
  .notice-cta:hover { background: rgba(255, 159, 10, 0.28); }

  .err {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 8px 11px;
    background: rgba(255, 69, 58, 0.08);
    border: 1px solid rgba(255, 69, 58, 0.22);
    border-radius: 9px;
    color: #FF8B82;
    font-size: 11.5px;
    line-height: 1.4;
  }

  /* ---------- Transcript ---------- */
  .transcript {
    display: flex; flex-direction: column; gap: 6px;
    padding: 11px 13px;
    background: rgba(255, 255, 255, 0.035);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 11px;
    animation: transcriptEnter 0.6s var(--ease-out-quint);
  }
  @keyframes transcriptEnter {
    from { opacity: 0; transform: translateY(6px); filter: blur(2px); }
    to { opacity: 1; transform: translateY(0); filter: blur(0); }
  }
  .transcript-head {
    display: flex; justify-content: space-between; align-items: baseline;
  }
  .transcript-label {
    font-family: var(--font-mono);
    font-size: 9.5px; font-weight: 600;
    color: rgba(255, 255, 255, 0.42);
    text-transform: uppercase;
    letter-spacing: 0.8px;
  }
  .transcript-meta {
    font-family: var(--font-mono);
    font-size: 9.5px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.32);
    font-variant-numeric: tabular-nums;
    letter-spacing: 0;
  }
  .transcript-body {
    font-size: 13px;
    line-height: 1.55;
    color: rgba(255, 255, 255, 0.85);
    white-space: pre-wrap;
    word-break: break-word;
  }

  /* ---------- Settings ---------- */
  .settings {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 4px 2px;
    overflow-y: auto;
    max-height: calc(100vh - 80px);
  }
  .settings::-webkit-scrollbar { width: 6px; }
  .settings::-webkit-scrollbar-track { background: transparent; }
  .settings::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.08); border-radius: 3px; }

  .setting-block {
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 11px 12px;
    background: rgba(255, 255, 255, 0.025);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 11px;
    animation: blockEnter 0.55s var(--ease-out-quint) both;
    transition: border-color 0.2s var(--ease-out-quart), background 0.2s var(--ease-out-quart);
  }
  .setting-block:hover { border-color: rgba(255, 255, 255, 0.1); }
  .setting-block:nth-of-type(1) { animation-delay: 0.02s; }
  .setting-block:nth-of-type(2) { animation-delay: 0.07s; }
  .setting-block:nth-of-type(3) { animation-delay: 0.12s; }
  .setting-block:nth-of-type(4) { animation-delay: 0.17s; }
  @keyframes blockEnter {
    from { opacity: 0; transform: translateY(6px); }
    to { opacity: 1; transform: translateY(0); }
  }
  .block-head {
    display: flex; align-items: center; justify-content: space-between;
    gap: 8px;
  }
  .block-title {
    display: inline-flex; align-items: center; gap: 6px;
    font-size: 11px; font-weight: 650;
    color: rgba(255, 255, 255, 0.88);
    letter-spacing: -0.05px;
  }
  .block-title :global(svg) { color: rgba(255, 255, 255, 0.5); }
  .block-hint {
    font-size: 10.5px;
    color: rgba(255, 255, 255, 0.32);
  }
  .block-link {
    font-size: 10.5px;
    color: #7eb6ff;
    text-decoration: none;
  }
  .block-link:hover { text-decoration: underline; }

  /* Segmented control */
  .segmented {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px;
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 9px;
    padding: 3px;
  }
  .seg-btn {
    display: inline-flex; align-items: center; justify-content: center;
    gap: 6px;
    padding: 8px 10px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: rgba(255, 255, 255, 0.45);
    font-size: 12px;
    font-weight: 550;
    font-family: inherit;
    cursor: pointer;
    transition: all 0.15s;
  }
  .seg-btn em {
    font-style: normal;
    font-size: 9.5px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.3);
    letter-spacing: 0.4px;
    text-transform: uppercase;
  }
  .seg-btn:hover:not(.is-active) { color: rgba(255, 255, 255, 0.7); }
  .seg-btn.is-active {
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.10), rgba(255, 255, 255, 0.06));
    color: rgba(255, 255, 255, 0.95);
    box-shadow:
      0 1px 0 rgba(255, 255, 255, 0.06) inset,
      0 1px 6px rgba(0, 0, 0, 0.3);
  }
  .seg-btn.is-active em { color: rgba(255, 255, 255, 0.5); }

  /* Inputs */
  .input {
    width: 100%;
    padding: 9px 11px;
    background: rgba(0, 0, 0, 0.25);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    color: rgba(255, 255, 255, 0.92);
    font-size: 12.5px;
    font-family: inherit;
    outline: none;
    transition: all 0.15s;
  }
  .input:focus {
    border-color: rgba(10, 132, 255, 0.55);
    background: rgba(10, 132, 255, 0.06);
    box-shadow: 0 0 0 3px rgba(10, 132, 255, 0.12);
  }
  .input::placeholder { color: rgba(255, 255, 255, 0.22); }

  /* Model cards */
  .model-list {
    display: flex; flex-direction: column; gap: 5px;
  }
  .model-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 9px 10px;
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 9px;
    transition: all 0.15s;
  }
  .model-card {
    animation: cardEnter 0.5s var(--ease-out-quint) both;
  }
  .model-card:nth-child(1) { animation-delay: 0.04s; }
  .model-card:nth-child(2) { animation-delay: 0.09s; }
  .model-card:nth-child(3) { animation-delay: 0.14s; }
  .model-card:nth-child(4) { animation-delay: 0.19s; }
  .model-card:nth-child(5) { animation-delay: 0.24s; }
  .model-card:nth-child(6) { animation-delay: 0.29s; }
  @keyframes cardEnter {
    from { opacity: 0; transform: translateY(4px); }
    to { opacity: 1; transform: translateY(0); }
  }
  .model-card:hover {
    border-color: rgba(255, 255, 255, 0.12);
    transform: translateY(-1px);
  }
  .model-card.is-active {
    border-color: rgba(10, 132, 255, 0.4);
    background: linear-gradient(180deg, rgba(10, 132, 255, 0.07), rgba(10, 132, 255, 0.02));
  }
  .model-card.is-downloading {
    border-color: rgba(10, 132, 255, 0.3);
  }
  .model-info { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px; }
  .model-name-row { display: flex; align-items: center; flex-wrap: wrap; gap: 5px; }
  .model-name {
    font-size: 12.5px; font-weight: 550;
    color: rgba(255, 255, 255, 0.9);
    letter-spacing: -0.1px;
  }
  .model-meta {
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.36);
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.1px;
  }

  .badge {
    display: inline-flex; align-items: center; gap: 3px;
    font-size: 9.5px;
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 999px;
    letter-spacing: 0.2px;
    text-transform: uppercase;
  }
  .badge-recommended {
    color: #FFD60A;
    background: rgba(255, 214, 10, 0.1);
    border: 1px solid rgba(255, 214, 10, 0.25);
  }
  .badge-active {
    color: #7eb6ff;
    background: rgba(10, 132, 255, 0.14);
    border: 1px solid rgba(10, 132, 255, 0.3);
  }
  .badge-ready {
    color: #6BE192;
    background: rgba(48, 209, 88, 0.1);
    border: 1px solid rgba(48, 209, 88, 0.25);
  }

  .progress-wrap { display: flex; flex-direction: column; gap: 4px; margin-top: 4px; }
  .progress-track {
    height: 4px;
    background: rgba(255, 255, 255, 0.06);
    border-radius: 99px;
    overflow: hidden;
  }
  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #0A84FF, #5E5CE6);
    border-radius: 99px;
    transition: width 0.2s ease;
    box-shadow: 0 0 8px rgba(10, 132, 255, 0.4);
  }
  .progress-label {
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.45);
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.1px;
  }

  .model-actions {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }

  /* Buttons */
  .btn {
    display: inline-flex; align-items: center; justify-content: center;
    gap: 5px;
    padding: 6px 11px;
    border: 1px solid transparent;
    border-radius: 7px;
    font-size: 11.5px;
    font-weight: 600;
    font-family: inherit;
    cursor: pointer;
    transition: all 0.15s;
    letter-spacing: -0.05px;
  }
  .btn:disabled { opacity: 0.4; cursor: not-allowed; }

  .btn-ghost {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.78);
  }
  .btn-ghost:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.18);
    color: white;
  }

  .btn-primary-sm {
    background: linear-gradient(180deg, rgba(10, 132, 255, 0.85), rgba(10, 132, 255, 0.65));
    color: white;
    box-shadow: 0 1px 0 rgba(255, 255, 255, 0.15) inset, 0 2px 6px rgba(10, 132, 255, 0.25);
  }
  .btn-primary-sm:hover { filter: brightness(1.1); }

  .btn-icon {
    padding: 6px;
    width: 26px; height: 26px;
    background: rgba(255, 255, 255, 0.04);
    border-color: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.55);
  }
  .btn-icon:hover {
    background: rgba(255, 255, 255, 0.08);
    color: white;
  }
  .btn-delete:hover {
    color: #FF6B63 !important;
    border-color: rgba(255, 69, 58, 0.3) !important;
    background: rgba(255, 69, 58, 0.1) !important;
  }

  .btn-danger {
    background: rgba(255, 69, 58, 0.12);
    border-color: rgba(255, 69, 58, 0.3);
    color: #FF8B82;
  }
  .btn-danger:hover { background: rgba(255, 69, 58, 0.2); color: #FF6B63; }

  .btn-primary {
    padding: 9px 14px;
    background: linear-gradient(180deg, #2196FF, #0A84FF);
    color: white;
    font-size: 12.5px;
    box-shadow:
      0 1px 0 rgba(255, 255, 255, 0.18) inset,
      0 4px 12px rgba(10, 132, 255, 0.3);
  }
  .btn-primary:hover { filter: brightness(1.08); transform: translateY(-1px); box-shadow: 0 1px 0 rgba(255, 255, 255, 0.2) inset, 0 6px 16px rgba(10, 132, 255, 0.4); }
  .btn-primary:active { transform: translateY(0); }

  .btn-secondary {
    padding: 9px 14px;
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.7);
    font-size: 12.5px;
  }
  .btn-secondary:hover {
    background: rgba(255, 255, 255, 0.1);
    color: white;
  }

  .settings-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
    padding-top: 2px;
  }

  /* Footer */
  .tray-hint {
    color: rgba(255, 255, 255, 0.22);
    font-size: 10.5px;
    text-align: center;
    margin-top: auto;
    padding-top: 6px;
    letter-spacing: 0.1px;
  }
</style>
