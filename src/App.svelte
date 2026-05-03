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
  let hotkeyMode = $state<"toggle" | "hold">("toggle");
  let activeHotkeyMode = $state<"toggle" | "hold">("toggle");
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
    idle: activeHotkeyMode === "hold"
      ? `Hold ${activeHotkey || hotkey} to talk`
      : `Press ${activeHotkey || hotkey} to start`,
    recording: activeHotkeyMode === "hold"
      ? "Release the hotkey to stop"
      : "Press the hotkey again to stop",
    transcribing: "Just a moment…",
    done: "Pasted into the focused field",
    error: errorMsg || "Try again",
  });

  async function shortcutHandler(event: { state: string }) {
    if (activeHotkeyMode === "hold") {
      if (event.state === "Pressed" && status === "idle") {
        await startRecording();
      } else if (event.state === "Released" && status === "recording") {
        await stopRecording();
      }
      return;
    }
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
    hotkeyMode = (await invoke<string>("get_hotkey_mode")) as "toggle" | "hold";
    activeHotkeyMode = hotkeyMode;
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
      const prev = activeHotkey;
      try {
        if (prev) await unregister(prev).catch(() => {});
        await register(hotkey, shortcutHandler);
        await invoke("save_hotkey", { hotkey });
        activeHotkey = hotkey;
      } catch (e) {
        // restore previous registration so the app keeps working
        if (prev) {
          try {
            await register(prev, shortcutHandler);
            activeHotkey = prev;
          } catch {
            activeHotkey = "";
          }
        }
        errorMsg = `'${hotkey}' not recognised (${e}) — reverted.`;
        hotkey = activeHotkey;
        return;
      }
    }
    if (hotkeyMode !== activeHotkeyMode) {
      await invoke("save_hotkey_mode", { mode: hotkeyMode });
      activeHotkeyMode = hotkeyMode;
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

  let capturingHotkey = $state(false);

  function captureKeydown(e: KeyboardEvent) {
    if (!capturingHotkey) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") {
      capturingHotkey = false;
      return;
    }
    const parts: string[] = [];
    if (e.ctrlKey) parts.push("Ctrl");
    if (e.altKey) parts.push("Alt");
    if (e.shiftKey) parts.push("Shift");
    if (e.metaKey) parts.push("Super");
    const k = e.key;
    // Ignore pure modifier presses — wait for a real key
    if (["Control", "Alt", "Shift", "Meta", "OS"].includes(k)) return;
    let main: string;
    if (k === " ") main = "Space";
    else if (k.length === 1) main = k.toUpperCase();
    else main = k.charAt(0).toUpperCase() + k.slice(1); // Enter, Tab, F1, ArrowUp…
    parts.push(main);
    hotkey = parts.join("+");
    capturingHotkey = false;
  }

  function startCapture() {
    errorMsg = "";
    capturingHotkey = true;
  }
</script>

<svelte:window on:keydown={captureKeydown} />

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
        <button
          type="button"
          class="input hotkey-capture"
          class:is-capturing={capturingHotkey}
          onclick={startCapture}
          title="Click, then press your shortcut"
        >
          {#if capturingHotkey}
            <span class="hotkey-prompt">Press your combination... (Esc to cancel)</span>
          {:else}
            <span class="kbd-row" style="margin: 0;">
              {#each formatHotkey(hotkey) as key, i}
                {#if i > 0}<span class="kbd-plus">+</span>{/if}
                <kbd>{key}</kbd>
              {/each}
            </span>
          {/if}
        </button>
        <div class="segmented" style="margin-top: 10px;">
          <button
            class="seg-btn {hotkeyMode === 'toggle' ? 'is-active' : ''}"
            onclick={() => (hotkeyMode = "toggle")}
          >
            <span>Tap</span>
            <em>press to start / stop</em>
          </button>
          <button
            class="seg-btn {hotkeyMode === 'hold' ? 'is-active' : ''}"
            onclick={() => (hotkeyMode = "hold")}
          >
            <span>Hold</span>
            <em>push-to-talk</em>
          </button>
        </div>
      </div>

      {#if errorMsg}
        <div class="err">
          <CircleAlert size={13} strokeWidth={2.2} />
          <span>{errorMsg}</span>
        </div>
      {/if}

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
    --ink: #231e17;
    --ink-2: #2e2820;
    --yellow: #fcf300;
    --yellow-soft: #fff96b;
    --blue: #058ed9;
    --blue-soft: #5dc1f1;
    --warm: #a39a92;
    --warm-soft: #d8d2cb;
    --font-display: 'Cherry Bomb One', 'Poppins', system-ui, sans-serif;
    --font-sans: 'Poppins', -apple-system, BlinkMacSystemFont, system-ui, sans-serif;
    --bounce: cubic-bezier(0.34, 1.56, 0.64, 1);
    --ease: cubic-bezier(0.22, 1, 0.36, 1);
  }

  :global(html, body) {
    margin: 0;
    padding: 0;
    background: var(--ink);
    background-image:
      radial-gradient(circle at 1px 1px, rgba(252, 243, 0, 0.10) 1.2px, transparent 0);
    background-size: 20px 20px;
    color: var(--yellow);
    font-family: var(--font-sans);
    font-weight: 500;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
    height: 100%;
  }

  main {
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-height: 100vh;
    animation: appEnter 0.5s var(--ease);
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
    padding: 2px 4px 6px;
  }
  .brand { display: flex; align-items: center; gap: 13px; }
  .brand-mark {
    width: 42px; height: 42px;
    display: grid; place-items: center;
    border-radius: 13px;
    background: var(--yellow);
    color: var(--ink);
    border: 3px solid var(--yellow);
    box-shadow: 3px 3px 0 black;
    transform: rotate(-5deg);
    transition: transform 0.25s var(--bounce);
  }
  .brand-mark :global(svg) { width: 22px; height: 22px; }
  .brand-mark:hover { transform: rotate(5deg) scale(1.08); }
  .brand-text { display: flex; flex-direction: column; line-height: 1; }
  .brand-name {
    font-family: var(--font-display);
    font-size: 30px;
    font-weight: 400;
    color: var(--yellow);
    letter-spacing: 0.5px;
    line-height: 1;
  }
  .brand-sub {
    font-size: 11px;
    font-weight: 800;
    color: var(--yellow);
    opacity: 0.55;
    margin-top: 5px;
    letter-spacing: 1.4px;
    text-transform: uppercase;
  }
  .topbar-actions { display: flex; gap: 9px; }

  .icon-btn {
    background: var(--yellow);
    border: 2.5px solid var(--yellow);
    cursor: pointer;
    width: 40px; height: 40px;
    border-radius: 12px;
    display: grid; place-items: center;
    color: var(--ink);
    transition: all 0.18s var(--bounce);
    box-shadow: 3px 3px 0 black;
  }
  .icon-btn :global(svg) { width: 18px; height: 18px; }
  .icon-btn:hover {
    transform: translate(-1px, -1px);
    box-shadow: 4px 4px 0 black;
  }
  .icon-btn:active {
    transform: translate(1px, 1px);
    box-shadow: 1px 1px 0 black;
  }
  .icon-btn.is-active {
    background: var(--blue);
    border-color: var(--blue);
    color: white;
    box-shadow: 3px 3px 0 var(--blue);
  }

  /* ---------- Hero status card ---------- */
  .hero {
    position: relative;
    overflow: hidden;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 24px 22px;
    background: var(--yellow);
    border: 3px solid var(--yellow);
    border-radius: 22px;
    color: var(--ink);
    box-shadow: 6px 6px 0 rgba(252, 243, 0, 0.25);
    transition:
      background 0.4s var(--ease),
      color 0.4s var(--ease),
      transform 0.3s var(--bounce),
      box-shadow 0.3s var(--bounce);
    animation: heroEnter 0.6s var(--bounce) both;
  }
  @keyframes heroEnter {
    from { opacity: 0; transform: translateY(10px) scale(0.94); }
    to { opacity: 1; transform: translateY(0) scale(1); }
  }
  .hero-glow { display: none; }
  .hero.recording {
    background: var(--blue);
    border-color: var(--blue);
    color: white;
    box-shadow: 6px 6px 0 rgba(5, 142, 217, 0.3);
  }
  .hero.transcribing {
    background: var(--yellow-soft);
    border-color: var(--yellow-soft);
    color: var(--ink);
  }
  .hero.done {
    background: var(--blue);
    border-color: var(--blue);
    color: white;
    box-shadow: 6px 6px 0 rgba(5, 142, 217, 0.3);
  }
  .hero.error {
    background: #ff5b5b;
    border-color: #ff5b5b;
    color: white;
  }
  .hero.disabled { opacity: 0.92; }

  .hero-icon {
    position: relative;
    width: 64px; height: 64px;
    border-radius: 18px;
    display: grid; place-items: center;
    color: var(--ink);
    background: white;
    border: 3px solid var(--ink);
    box-shadow: 3px 3px 0 var(--ink);
    flex-shrink: 0;
    z-index: 1;
    transform: rotate(-3deg);
  }
  .hero-icon :global(svg) { width: 30px; height: 30px; }
  .hero.recording .hero-icon {
    color: var(--blue);
    background: var(--yellow);
    animation: bounceIcon 1.2s var(--bounce) infinite;
  }
  .hero.transcribing .hero-icon { color: var(--ink); background: white; }
  .hero.done .hero-icon { color: var(--ink); background: var(--yellow); }
  .hero.error .hero-icon { color: #ff5b5b; background: white; }

  @keyframes bounceIcon {
    0%, 100% { transform: rotate(-4deg) translateY(0); }
    50% { transform: rotate(4deg) translateY(-3px); }
  }
  .hero.idle .hero-icon {
    animation: breathe 2.6s var(--bounce) infinite;
  }
  @keyframes breathe {
    0%, 100% { transform: rotate(-3deg) scale(1); }
    50% { transform: rotate(3deg) scale(1.05); }
  }

  :global(.spin) { animation: spin 0.9s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }

  .wave {
    display: flex; align-items: center; gap: 3px; height: 22px;
  }
  .wave span {
    width: 4px; border-radius: 99px;
    background: var(--blue);
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
    gap: 4px;
  }
  .hero-title {
    font-family: var(--font-display);
    font-size: 28px;
    font-weight: 400;
    color: inherit;
    letter-spacing: 0.4px;
    line-height: 1;
  }
  .hero-sub {
    font-size: 13.5px;
    color: inherit;
    opacity: 0.78;
    font-weight: 600;
  }

  .kbd-row {
    display: flex; align-items: center; gap: 4px;
    flex-shrink: 0;
  }
  .kbd-plus {
    color: var(--ink);
    opacity: 0.55;
    font-size: 11px;
    font-weight: 800;
  }
  kbd {
    display: inline-flex; align-items: center; justify-content: center;
    min-width: 32px; height: 32px;
    padding: 0 10px;
    background: white;
    border: 2.5px solid var(--ink);
    border-radius: 9px;
    font-family: var(--font-sans);
    font-size: 12px;
    font-weight: 800;
    color: var(--ink);
    box-shadow: 2px 2px 0 var(--ink);
    letter-spacing: 0.3px;
    text-transform: uppercase;
    transition: transform 0.18s var(--bounce), box-shadow 0.18s var(--bounce);
    animation: kbdEnter 0.5s var(--bounce) both;
  }
  .kbd-row:hover kbd {
    transform: translate(-1px, -1px);
    box-shadow: 3px 3px 0 var(--ink);
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
    gap: 13px;
    padding: 14px 16px;
    background: white;
    border: 3px solid white;
    border-radius: 16px;
    box-shadow: 4px 4px 0 rgba(255, 255, 255, 0.2);
    color: var(--ink);
    font-size: 13px;
    line-height: 1.4;
    animation: noticeEnter 0.55s var(--bounce) 0.15s both;
  }
  @keyframes noticeEnter {
    from { opacity: 0; transform: translateY(6px) scale(0.96); }
    to { opacity: 1; transform: translateY(0) scale(1); }
  }
  .notice-icon {
    width: 34px; height: 34px;
    border-radius: 10px;
    display: grid; place-items: center;
    background: var(--yellow);
    border: 2.5px solid var(--ink);
    color: var(--ink);
    flex-shrink: 0;
  }
  .notice-body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .notice-body strong { color: var(--ink); font-weight: 800; font-size: 13px; }
  .notice-body span { color: var(--ink); opacity: 0.65; font-size: 12.5px; font-weight: 500; }
  .notice-cta {
    background: var(--ink);
    border: 2.5px solid var(--ink);
    color: var(--yellow);
    border-radius: 10px;
    padding: 9px 15px;
    font-family: inherit;
    font-size: 12.5px;
    font-weight: 700;
    cursor: pointer;
    flex-shrink: 0;
    box-shadow: 2px 2px 0 var(--ink);
    transition: transform 0.18s var(--bounce), box-shadow 0.18s var(--bounce);
  }
  .notice-cta:hover {
    transform: translate(-1px, -1px);
    box-shadow: 3px 3px 0 var(--ink);
  }
  .notice-cta:active {
    transform: translate(1px, 1px);
    box-shadow: 1px 1px 0 var(--ink);
  }

  .err {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 12px 15px;
    background: #ff5b5b;
    border: 2.5px solid #ff5b5b;
    border-radius: 13px;
    color: white;
    font-size: 12.5px;
    font-weight: 700;
    line-height: 1.4;
  }

  /* ---------- Transcript ---------- */
  .transcript {
    display: flex; flex-direction: column; gap: 10px;
    padding: 16px 18px;
    background: white;
    border: 3px solid white;
    border-radius: 16px;
    animation: heroEnter 0.6s var(--bounce);
  }
  .transcript-head {
    display: flex; justify-content: space-between; align-items: center;
    padding-bottom: 9px;
    border-bottom: 2px dashed rgba(35, 30, 23, 0.25);
  }
  .transcript-label {
    font-family: var(--font-display);
    font-size: 16px; font-weight: 400;
    color: var(--ink);
    letter-spacing: 0.4px;
  }
  .transcript-meta {
    font-size: 11px;
    font-weight: 800;
    color: white;
    background: var(--blue);
    border: 2px solid var(--ink);
    padding: 3px 11px;
    border-radius: 99px;
    font-variant-numeric: tabular-nums;
  }
  .transcript-body {
    font-size: 14px;
    line-height: 1.6;
    color: var(--ink);
    font-weight: 500;
    white-space: pre-wrap;
    word-break: break-word;
  }

  /* ---------- Settings ---------- */
  .settings {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 4px 2px;
    overflow-y: auto;
    max-height: calc(100vh - 90px);
  }
  .settings::-webkit-scrollbar { width: 10px; }
  .settings::-webkit-scrollbar-track { background: transparent; }
  .settings::-webkit-scrollbar-thumb {
    background: var(--yellow);
    border-radius: 99px;
  }

  .setting-block {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 18px 18px;
    background: white;
    border: 3px solid white;
    border-radius: 18px;
    animation: heroEnter 0.55s var(--bounce) both;
  }
  .setting-block:nth-of-type(2) { background: var(--yellow); border-color: var(--yellow); }
  .setting-block:nth-of-type(3) { background: white; border-color: white; }
  .setting-block:nth-of-type(4) { background: var(--yellow); border-color: var(--yellow); }
  .setting-block:nth-of-type(1) { animation-delay: 0.02s; }
  .setting-block:nth-of-type(2) { animation-delay: 0.07s; }
  .setting-block:nth-of-type(3) { animation-delay: 0.12s; }
  .setting-block:nth-of-type(4) { animation-delay: 0.17s; }

  .block-head {
    display: flex; align-items: center; justify-content: space-between;
    gap: 8px;
  }
  .block-title {
    display: inline-flex; align-items: center; gap: 8px;
    font-family: var(--font-display);
    font-size: 18px; font-weight: 400;
    color: var(--ink);
    letter-spacing: 0.4px;
  }
  .block-title :global(svg) { color: var(--ink); width: 16px; height: 16px; }
  .block-hint {
    font-size: 12px;
    font-weight: 700;
    color: var(--ink);
    opacity: 0.55;
  }
  .block-link {
    font-size: 12.5px;
    font-weight: 800;
    color: var(--blue);
    text-decoration: none;
    border-bottom: 2px solid var(--blue);
    padding-bottom: 1px;
  }
  .block-link:hover { color: var(--ink); border-color: var(--ink); }

  /* Segmented control */
  .segmented {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .seg-btn {
    display: inline-flex; align-items: center; justify-content: center;
    gap: 9px;
    padding: 14px 12px;
    border: 2.5px solid var(--ink);
    border-radius: 13px;
    background: white;
    color: var(--ink);
    font-size: 14px;
    font-weight: 700;
    font-family: inherit;
    cursor: pointer;
    box-shadow: 3px 3px 0 var(--ink);
    transition: transform 0.18s var(--bounce), box-shadow 0.18s var(--bounce), background 0.15s;
  }
  .seg-btn :global(svg) { width: 16px; height: 16px; }
  .seg-btn em {
    font-style: normal;
    font-size: 10.5px;
    font-weight: 800;
    opacity: 0.6;
    letter-spacing: 0.7px;
    text-transform: uppercase;
  }
  .seg-btn:hover:not(.is-active) {
    transform: translate(-1px, -1px);
    box-shadow: 4px 4px 0 var(--ink);
  }
  .seg-btn.is-active {
    background: var(--ink);
    color: var(--yellow);
  }
  .seg-btn.is-active em { opacity: 0.7; }

  /* Inputs */
  .input {
    width: 100%;
    padding: 14px 16px;
    background: white;
    border: 2.5px solid var(--ink);
    border-radius: 12px;
    color: var(--ink);
    font-size: 14px;
    font-weight: 600;
    font-family: inherit;
    outline: none;
    box-shadow: 3px 3px 0 var(--ink);
    transition: transform 0.18s var(--bounce), box-shadow 0.18s var(--bounce);
  }
  .input:focus {
    transform: translate(-1px, -1px);
    box-shadow: 4px 4px 0 var(--blue);
    border-color: var(--blue);
  }
  .input::placeholder { color: var(--ink); opacity: 0.35; font-weight: 500; }

  .hotkey-capture {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    cursor: pointer;
    text-align: center;
    min-height: 48px;
  }
  .hotkey-capture.is-capturing {
    border-color: var(--blue);
    box-shadow: 4px 4px 0 var(--blue);
    transform: translate(-1px, -1px);
  }
  .hotkey-prompt {
    color: var(--ink);
    opacity: 0.75;
    font-weight: 600;
    font-size: 13px;
  }

  /* Model cards */
  .model-list {
    display: flex; flex-direction: column; gap: 10px;
  }
  .model-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 14px 16px;
    background: white;
    border: 2.5px solid var(--ink);
    border-radius: 14px;
    box-shadow: 3px 3px 0 var(--ink);
    transition: transform 0.18s var(--bounce), box-shadow 0.18s var(--bounce), background 0.2s;
    animation: heroEnter 0.5s var(--bounce) both;
  }
  .model-card:nth-child(1) { animation-delay: 0.04s; }
  .model-card:nth-child(2) { animation-delay: 0.09s; }
  .model-card:nth-child(3) { animation-delay: 0.14s; }
  .model-card:nth-child(4) { animation-delay: 0.19s; }
  .model-card:nth-child(5) { animation-delay: 0.24s; }
  .model-card:nth-child(6) { animation-delay: 0.29s; }
  .model-card:hover {
    transform: translate(-1px, -1px);
    box-shadow: 4px 4px 0 var(--ink);
  }
  .model-card.is-active {
    background: var(--yellow);
    box-shadow: 3px 3px 0 var(--blue);
  }
  .model-card.is-downloading {
    background: var(--blue);
    color: white;
  }
  .model-card.is-downloading .model-name,
  .model-card.is-downloading .model-meta { color: white; }
  .model-info { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; }
  .model-name-row { display: flex; align-items: center; flex-wrap: wrap; gap: 6px; }
  .model-name {
    font-size: 14.5px; font-weight: 800;
    color: var(--ink);
    letter-spacing: -0.05px;
  }
  .model-meta {
    font-size: 12px;
    font-weight: 700;
    color: var(--ink);
    opacity: 0.55;
    font-variant-numeric: tabular-nums;
  }

  .badge {
    display: inline-flex; align-items: center; gap: 4px;
    font-size: 10.5px;
    font-weight: 800;
    padding: 4px 9px;
    border-radius: 999px;
    letter-spacing: 0.4px;
    text-transform: uppercase;
    border: 2px solid var(--ink);
  }
  .badge-recommended {
    color: var(--ink);
    background: var(--yellow);
  }
  .badge-active {
    color: white;
    background: var(--blue);
  }
  .badge-ready {
    color: var(--ink);
    background: white;
  }

  .progress-wrap { display: flex; flex-direction: column; gap: 6px; margin-top: 8px; }
  .progress-track {
    height: 12px;
    background: white;
    border: 2.5px solid var(--ink);
    border-radius: 99px;
    overflow: hidden;
  }
  .progress-fill {
    height: 100%;
    background: var(--yellow);
    border-radius: 99px;
    transition: width 0.2s ease;
  }
  .model-card.is-downloading .progress-track { background: var(--ink-2); }
  .progress-label {
    font-size: 12px;
    font-weight: 800;
    color: var(--yellow);
    font-variant-numeric: tabular-nums;
  }

  .model-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  /* Buttons */
  .btn {
    display: inline-flex; align-items: center; justify-content: center;
    gap: 7px;
    padding: 10px 16px;
    border: 2.5px solid var(--ink);
    border-radius: 11px;
    font-size: 13px;
    font-weight: 800;
    font-family: inherit;
    cursor: pointer;
    box-shadow: 2px 2px 0 var(--ink);
    transition: transform 0.18s var(--bounce), box-shadow 0.18s var(--bounce), background 0.15s;
  }
  .btn :global(svg) { width: 14px; height: 14px; }
  .btn:hover:not(:disabled) {
    transform: translate(-1px, -1px);
    box-shadow: 3px 3px 0 var(--ink);
  }
  .btn:active:not(:disabled) {
    transform: translate(1px, 1px);
    box-shadow: 1px 1px 0 var(--ink);
  }
  .btn:disabled { opacity: 0.4; cursor: not-allowed; }

  .btn-ghost {
    background: var(--ink);
    color: var(--yellow);
  }
  .btn-primary-sm {
    background: var(--blue);
    color: white;
  }
  .btn-icon {
    padding: 6px;
    width: 36px; height: 36px;
    background: white;
    color: var(--ink);
  }
  .btn-delete:hover {
    color: white !important;
    background: #ff5b5b !important;
  }
  .btn-danger {
    background: #ff5b5b;
    color: white;
  }

  .btn-primary {
    padding: 14px 22px;
    background: var(--yellow);
    color: var(--ink);
    border-color: var(--yellow);
    font-family: var(--font-display);
    font-weight: 400;
    font-size: 16px;
    letter-spacing: 0.4px;
    border-radius: 13px;
    box-shadow: 4px 4px 0 rgba(252, 243, 0, 0.3);
  }
  .btn-primary :global(svg) { width: 16px; height: 16px; }
  .btn-primary:hover:not(:disabled) {
    transform: translate(-2px, -2px);
    box-shadow: 6px 6px 0 rgba(252, 243, 0, 0.4);
  }

  .btn-secondary {
    padding: 14px 22px;
    background: transparent;
    color: var(--yellow);
    border-color: var(--yellow);
    font-size: 14px;
    font-weight: 800;
    border-radius: 13px;
    box-shadow: 4px 4px 0 rgba(252, 243, 0, 0.2);
  }
  .btn-secondary:hover {
    transform: translate(-2px, -2px);
    box-shadow: 6px 6px 0 rgba(252, 243, 0, 0.3);
  }

  .settings-actions {
    display: flex;
    gap: 12px;
    justify-content: flex-end;
    padding-top: 6px;
  }

  /* Footer */
  .tray-hint {
    color: var(--yellow);
    opacity: 0.45;
    font-size: 11.5px;
    font-weight: 700;
    text-align: center;
    margin-top: auto;
    padding-top: 10px;
    letter-spacing: 0.2px;
  }
</style>

