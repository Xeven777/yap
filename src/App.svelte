<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { emit } from "@tauri-apps/api/event";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { register, unregister } from "@tauri-apps/plugin-global-shortcut";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { onMount, onDestroy } from "svelte";
  import "./app.css";

  type Status = "idle" | "recording" | "transcribing" | "done" | "error";

  let status: Status = $state("idle");
  let transcript = $state("");
  let apiKey = $state("");
  let language = $state("");
  let hotkey = $state("Ctrl+Shift+Space");
  let activeHotkey = ""; // currently registered shortcut
  let showSettings = $state(false);
  let errorMsg = $state("");

  const win = getCurrentWebviewWindow();

  // Push every status change to the pill overlay.
  $effect(() => {
    emit("yap://state", status);
  });

  let statusLabel = $derived<Record<Status, string>>({
    idle: `${activeHotkey || hotkey} to record`,
    recording: "Recording… (press again to stop)",
    transcribing: "Transcribing…",
    done: "Done ✓",
    error: "Error",
  });

  async function shortcutHandler(event: { state: string }) {
    if (event.state !== "Pressed") return;
    if (status === "idle") await startRecording();
    else if (status === "recording") await stopRecording();
  }

  onMount(async () => {
    apiKey = await invoke<string>("get_api_key");
    language = await invoke<string>("get_language");
    hotkey = await invoke<string>("get_hotkey");

    try {
      await register(hotkey, shortcutHandler);
      activeHotkey = hotkey;
      console.log("[yap] shortcut registered:", hotkey);
    } catch (e) {
      console.error("[yap] shortcut registration failed:", e);
      errorMsg = `Shortcut failed: ${e}`;
    }
  });

  onDestroy(async () => {
    if (activeHotkey) await unregister(activeHotkey).catch(() => {});
  });

  async function startRecording() {
    status = "recording";
    transcript = "";
    errorMsg = "";
    try {
      await invoke("start_recording");
      console.log("[yap] recording started");
    } catch (e) {
      console.error("[yap] start_recording:", e);
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
      });
      console.log("[yap] transcript:", transcript);
      status = "done";
      await writeText(transcript);
      await new Promise((r) => setTimeout(r, 80));
      await invoke("paste_text");
    } catch (e) {
      console.error("[yap] stop_recording:", e);
      status = "error";
      errorMsg = String(e);
    } finally {
      setTimeout(() => {
        if (status !== "recording") status = "idle";
      }, 3000);
    }
  }

  async function saveSettings() {
    // Re-register hotkey if it changed.
    if (hotkey !== activeHotkey) {
      try {
        if (activeHotkey) await unregister(activeHotkey);
        await register(hotkey, shortcutHandler);
        await invoke("save_hotkey", { hotkey });
        activeHotkey = hotkey;
      } catch (e) {
        errorMsg = `'${hotkey}' not recognised — reverted. Try e.g. Ctrl+Shift+D`;
        hotkey = activeHotkey;
        return;
      }
    }

    await invoke("save_api_key", { key: apiKey });
    await invoke("save_language", { language });
    showSettings = false;
  }
</script>

<main>
  <header>
    <div class="pill-row {status}">
      <span class="dot"></span>
      <span class="label">{statusLabel[status]}</span>
      <button class="gear" onclick={() => (showSettings = !showSettings)} title="Settings">⚙</button>
      <button class="hide-btn" onclick={() => win.hide()} title="Hide to tray">−</button>
    </div>
  </header>

  {#if transcript}
    <div class="transcript">{transcript}</div>
  {/if}

  {#if errorMsg}
    <p class="err">{errorMsg}</p>
  {/if}

  {#if showSettings}
    <div class="settings">
      <label>
        Groq API Key
        <input type="password" bind:value={apiKey} placeholder="gsk_…" autocomplete="off" />
      </label>
      <label>
        Hotkey <span class="hint">e.g. Ctrl+Shift+Space, Alt+D</span>
        <input type="text" bind:value={hotkey} placeholder="Ctrl+Shift+Space" />
      </label>
      <label>
        Language <span class="hint">e.g. en, fr, de — blank = auto</span>
        <input type="text" bind:value={language} placeholder="auto-detect" maxlength="5" />
      </label>
      <button onclick={saveSettings}>Save</button>
    </div>
  {/if}

  {#if !apiKey}
    <p class="warn">⚠ Open settings (⚙) and enter your Groq API key</p>
  {/if}

  <p class="tray-hint">Closing this window hides it to the system tray.</p>
</main>

<style>
  main {
    padding: 1.25rem;
    max-width: 480px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .pill-row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.6rem 1rem;
    border-radius: 999px;
    background: #1e1e2e;
    transition: background 0.25s;
  }
  .pill-row.recording    { background: #7f1d1d; }
  .pill-row.transcribing { background: #1e3a5f; }
  .pill-row.done         { background: #14532d; }
  .pill-row.error        { background: #7a1c1c; }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #888;
    flex-shrink: 0;
  }
  .recording .dot {
    background: #f87171;
    animation: pulse 0.9s ease-in-out infinite;
  }
  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50%       { opacity: 0.25; }
  }

  .label { flex: 1; font-size: 0.88rem; }

  .gear, .hide-btn {
    background: none;
    border: none;
    cursor: pointer;
    font-size: 1rem;
    color: #aaa;
    padding: 0;
    line-height: 1;
  }
  .gear:hover, .hide-btn:hover { color: #fff; }

  .transcript {
    padding: 0.75rem;
    background: #1a1a2a;
    border-radius: 8px;
    font-size: 0.9rem;
    line-height: 1.55;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .err  { color: #f87171; font-size: 0.82rem; }
  .warn { color: #fbbf24; font-size: 0.82rem; }
  .tray-hint { color: #444; font-size: 0.75rem; text-align: center; margin-top: 0.25rem; }

  .settings {
    padding: 1rem;
    border: 1px solid #2a2a3a;
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
  }
  .settings label {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    font-size: 0.85rem;
    color: #999;
  }
  .hint { font-size: 0.73rem; color: #555; }
  .settings input {
    padding: 0.4rem 0.6rem;
    border: 1px solid #333;
    border-radius: 5px;
    background: #111;
    color: #eee;
    font-size: 0.88rem;
  }
  .settings button {
    align-self: flex-start;
    padding: 0.4rem 1.1rem;
    background: #2563eb;
    color: #fff;
    border: none;
    border-radius: 5px;
    cursor: pointer;
    font-size: 0.85rem;
  }
  .settings button:hover { background: #1d4ed8; }
</style>
