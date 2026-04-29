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
  let activeHotkey = "";
  let showSettings = $state(false);
  let errorMsg = $state("");

  let win: ReturnType<typeof getCurrentWebviewWindow> | null = null;

  $effect(() => {
    emit("yap://state", status);
  });

  let statusLabel = $derived<Record<Status, string>>({
    idle: `${activeHotkey || hotkey} to record`,
    recording: "Recording… press again to stop",
    transcribing: "Transcribing…",
    done: "Copied to clipboard",
    error: "Error",
  });

  async function shortcutHandler(event: { state: string }) {
    if (event.state !== "Pressed") return;
    if (status === "idle") await startRecording();
    else if (status === "recording") await stopRecording();
  }

  onMount(async () => {
    win = getCurrentWebviewWindow();
    apiKey = await invoke<string>("get_api_key");
    language = await invoke<string>("get_language");
    hotkey = await invoke<string>("get_hotkey");

    try {
      await register(hotkey, shortcutHandler);
      activeHotkey = hotkey;
    } catch (e) {
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
    showSettings = false;
  }
</script>

<main>
  <div class="status-row {status}">
    <span class="dot"></span>
    <span class="label">{statusLabel[status]}</span>
    <div class="actions">
      <button
        class="icon-btn {showSettings ? 'active' : ''}"
        onclick={() => (showSettings = !showSettings)}
        title="Settings"
      >
        <svg width="15" height="15" viewBox="0 0 15 15" fill="none">
          <path d="M7.5 9.5a2 2 0 1 0 0-4 2 2 0 0 0 0 4z" fill="currentColor"/>
          <path fill-rule="evenodd" clip-rule="evenodd" d="M6.07 1.27a1.5 1.5 0 0 1 1.86 0l.7.57a.5.5 0 0 0 .45.09l.87-.25a1.5 1.5 0 0 1 1.61.8l.38.8a.5.5 0 0 0 .36.28l.88.16a1.5 1.5 0 0 1 1.22 1.46v.88a.5.5 0 0 0 .18.38l.67.56a1.5 1.5 0 0 1 .32 1.9l-.47.76a.5.5 0 0 0-.04.46l.35.83a1.5 1.5 0 0 1-.64 1.84l-.78.44a.5.5 0 0 0-.26.4l-.07.9a1.5 1.5 0 0 1-1.44 1.37l-.9-.02a.5.5 0 0 0-.43.22l-.5.74a1.5 1.5 0 0 1-1.86.42l-.8-.4a.5.5 0 0 0-.46 0l-.8.4a1.5 1.5 0 0 1-1.86-.42l-.5-.74a.5.5 0 0 0-.43-.22l-.9.02A1.5 1.5 0 0 1 .94 12.3l-.07-.9a.5.5 0 0 0-.26-.4l-.78-.44a1.5 1.5 0 0 1-.64-1.84l.35-.83a.5.5 0 0 0-.04-.46l-.47-.76a1.5 1.5 0 0 1 .32-1.9l.67-.56A.5.5 0 0 0 .2 4.83v-.88A1.5 1.5 0 0 1 1.42 2.5l.88-.16a.5.5 0 0 0 .36-.28l.38-.8a1.5 1.5 0 0 1 1.61-.8l.87.25a.5.5 0 0 0 .45-.09l.7-.57zm.93 1.23-.7.57a1.5 1.5 0 0 1-1.35.27l-.87-.25-.38.8a1.5 1.5 0 0 1-1.09.84l-.88.16v.88a1.5 1.5 0 0 1-.54 1.16l-.67.56.47.76a1.5 1.5 0 0 1 .13 1.38l-.35.83.78.44a1.5 1.5 0 0 1 .77 1.2l.07.9.9-.02a1.5 1.5 0 0 1 1.28.66l.5.74.8-.4a1.5 1.5 0 0 1 1.38 0l.8.4.5-.74a1.5 1.5 0 0 1 1.28-.66l.9.02.07-.9a1.5 1.5 0 0 1 .77-1.2l.78-.44-.35-.83a1.5 1.5 0 0 1 .13-1.38l.47-.76-.67-.56A1.5 1.5 0 0 1 13.8 6.8v-.88l-.88-.16a1.5 1.5 0 0 1-1.09-.84l-.38-.8-.87.25a1.5 1.5 0 0 1-1.35-.27l-.7-.57z" fill="currentColor"/>
        </svg>
      </button>
      <button class="icon-btn" onclick={() => win?.hide()} title="Hide to tray">
        <svg width="12" height="2" viewBox="0 0 12 2" fill="none">
          <rect x="0" y="0" width="12" height="2" rx="1" fill="currentColor"/>
        </svg>
      </button>
    </div>
  </div>

  {#if !apiKey && !showSettings}
    <div class="notice">
      <span class="notice-icon">⚠</span>
      Add your Groq API key in settings to get started
    </div>
  {/if}

  {#if transcript && !showSettings}
    <div class="transcript">{transcript}</div>
  {/if}

  {#if errorMsg}
    <p class="err">{errorMsg}</p>
  {/if}

  {#if showSettings}
    <div class="settings">
      <div class="settings-title">Settings</div>
      <div class="field">
        <label for="apikey">Groq API Key</label>
        <input id="apikey" type="password" bind:value={apiKey} placeholder="gsk_…" autocomplete="off" />
      </div>
      <div class="field">
        <label for="hotkey">
          Hotkey
          <span class="hint">e.g. Ctrl+Shift+Space</span>
        </label>
        <input id="hotkey" type="text" bind:value={hotkey} placeholder="Ctrl+Shift+Space" />
      </div>
      <div class="field">
        <label for="lang">
          Language
          <span class="hint">blank = auto-detect</span>
        </label>
        <input id="lang" type="text" bind:value={language} placeholder="en, fr, de…" maxlength="5" />
      </div>
      <div class="settings-actions">
        <button class="btn-secondary" onclick={() => (showSettings = false)}>Cancel</button>
        <button class="btn-primary" onclick={saveSettings}>Save</button>
      </div>
    </div>
  {/if}

  <p class="tray-hint">Closing this window hides it to the system tray</p>
</main>

<style>
  :global(html, body) {
    margin: 0;
    padding: 0;
    background: #111113;
    color: #f5f5f7;
    font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', system-ui, sans-serif;
    -webkit-font-smoothing: antialiased;
    height: 100%;
  }

  main {
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 100vh;
  }

  .status-row {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 9px 12px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 11px;
    transition: background 0.2s ease, border-color 0.2s ease;
  }
  .status-row.recording {
    background: rgba(50, 14, 14, 0.7);
    border-color: rgba(255, 69, 58, 0.2);
  }
  .status-row.transcribing {
    background: rgba(12, 24, 52, 0.7);
    border-color: rgba(10, 132, 255, 0.2);
  }
  .status-row.done {
    background: rgba(12, 36, 22, 0.7);
    border-color: rgba(48, 209, 88, 0.2);
  }

  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.2);
    flex-shrink: 0;
    transition: background 0.2s ease;
  }
  .recording .dot {
    background: #FF453A;
    animation: pulse 1s ease-in-out infinite;
  }
  .transcribing .dot {
    background: #0A84FF;
    animation: pulse 1.2s ease-in-out infinite;
  }
  .done .dot { background: #30D158; }
  .error .dot { background: #FF453A; }

  @keyframes pulse {
    0%, 100% { opacity: 1; transform: scale(1); }
    50%       { opacity: 0.35; transform: scale(0.75); }
  }

  .label {
    flex: 1;
    font-size: 13px;
    font-weight: 450;
    color: rgba(255, 255, 255, 0.72);
    letter-spacing: -0.1px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .actions {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
  }

  .icon-btn {
    background: none;
    border: none;
    cursor: pointer;
    width: 26px;
    height: 26px;
    border-radius: 7px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: rgba(255, 255, 255, 0.3);
    transition: background 0.15s, color 0.15s;
  }
  .icon-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.75);
  }
  .icon-btn.active {
    background: rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.8);
  }

  .notice {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 12px;
    background: rgba(255, 159, 10, 0.08);
    border: 1px solid rgba(255, 159, 10, 0.2);
    border-radius: 10px;
    font-size: 12.5px;
    color: rgba(255, 200, 80, 0.9);
    line-height: 1.4;
  }
  .notice-icon { font-size: 13px; flex-shrink: 0; }

  .transcript {
    padding: 11px 13px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 11px;
    font-size: 13.5px;
    line-height: 1.6;
    color: rgba(255, 255, 255, 0.82);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .err {
    padding: 8px 12px;
    background: rgba(255, 69, 58, 0.08);
    border: 1px solid rgba(255, 69, 58, 0.2);
    border-radius: 10px;
    color: #FF6B63;
    font-size: 12px;
    line-height: 1.45;
  }

  .settings {
    padding: 13px 14px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .settings-title {
    font-size: 13px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.7);
    letter-spacing: -0.1px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .field label {
    display: flex;
    align-items: baseline;
    gap: 6px;
    font-size: 11px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.38);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .hint {
    font-size: 10.5px;
    font-weight: 400;
    color: rgba(255, 255, 255, 0.22);
    text-transform: none;
    letter-spacing: 0;
  }

  .field input {
    padding: 8px 10px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.09);
    border-radius: 8px;
    color: rgba(255, 255, 255, 0.88);
    font-size: 13px;
    font-family: inherit;
    outline: none;
    transition: border-color 0.15s, background 0.15s;
  }
  .field input:focus {
    border-color: rgba(10, 132, 255, 0.5);
    background: rgba(10, 132, 255, 0.05);
  }
  .field input::placeholder {
    color: rgba(255, 255, 255, 0.2);
  }

  .settings-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
    padding-top: 2px;
  }

  .btn-primary, .btn-secondary {
    padding: 7px 15px;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    font-size: 13px;
    font-weight: 500;
    font-family: inherit;
    transition: opacity 0.15s;
  }
  .btn-primary {
    background: #0A84FF;
    color: #fff;
  }
  .btn-primary:hover { opacity: 0.85; }
  .btn-secondary {
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.65);
  }
  .btn-secondary:hover { background: rgba(255, 255, 255, 0.12); }

  .tray-hint {
    color: rgba(255, 255, 255, 0.16);
    font-size: 11px;
    text-align: center;
    margin-top: auto;
    padding-top: 4px;
    letter-spacing: -0.1px;
  }
</style>
