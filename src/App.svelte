<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { register, unregister } from "@tauri-apps/plugin-global-shortcut";
  import { onMount, onDestroy } from "svelte";
  import "./app.css";

  type Status = "idle" | "recording" | "transcribing" | "done" | "error";

  let status: Status = $state("idle");
  let transcript = $state("");
  let apiKey = $state("");
  let showSettings = $state(false);
  let errorMsg = $state("");

  const SHORTCUT = "Ctrl+Shift+Space";

  onMount(async () => {
    apiKey = await invoke<string>("get_api_key");
    console.log("[yap] API key loaded:", apiKey ? "set" : "empty");

    try {
      await register(SHORTCUT, async (event) => {
        console.log("[yap] shortcut event:", event.state, "| current status:", status);
        if (event.state !== "Pressed") return;
        if (status === "idle") {
          await startRecording();
        } else if (status === "recording") {
          await stopRecording();
        }
      });
      console.log("[yap] shortcut registered:", SHORTCUT);
    } catch (e) {
      console.error("[yap] failed to register shortcut:", e);
      errorMsg = `Shortcut registration failed: ${e}`;
    }
  });

  onDestroy(async () => {
    await unregister(SHORTCUT).catch((e) => console.warn("[yap] unregister failed:", e));
  });

  async function startRecording() {
    console.log("[yap] startRecording()");
    status = "recording";
    transcript = "";
    errorMsg = "";
    try {
      await invoke("start_recording");
      console.log("[yap] Rust start_recording OK");
    } catch (e) {
      console.error("[yap] start_recording error:", e);
      status = "error";
      errorMsg = String(e);
      setTimeout(() => (status = "idle"), 3000);
    }
  }

  async function stopRecording() {
    console.log("[yap] stopRecording(), apiKey set:", !!apiKey);
    status = "transcribing";
    try {
      transcript = await invoke<string>("stop_recording", { apiKey });
      console.log("[yap] transcript:", transcript);
      status = "done";
      await writeText(transcript);
      await new Promise((r) => setTimeout(r, 80));
      await invoke("paste_text");
    } catch (e) {
      console.error("[yap] stop_recording error:", e);
      status = "error";
      errorMsg = String(e);
    } finally {
      setTimeout(() => {
        if (status !== "recording") status = "idle";
      }, 3000);
    }
  }

  async function saveKey() {
    await invoke("save_api_key", { key: apiKey });
    showSettings = false;
  }

  const statusLabel: Record<Status, string> = {
    idle: "Ctrl+Shift+Space to record",
    recording: "Recording…",
    transcribing: "Transcribing…",
    done: "Done ✓",
    error: "Error",
  };
</script>

<main>
  <div class="pill {status}">
    <span class="dot"></span>
    <span class="label">{statusLabel[status]}</span>
    <button class="gear" onclick={() => (showSettings = !showSettings)}>⚙</button>
  </div>

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
        <input type="password" bind:value={apiKey} placeholder="gsk_…" />
      </label>
      <button onclick={saveKey}>Save</button>
    </div>
  {/if}

  {#if !apiKey}
    <p class="warn">⚠ Open settings (⚙) and enter your Groq API key</p>
  {/if}
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

  .pill {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.6rem 1rem;
    border-radius: 999px;
    background: #1e1e2e;
    transition: background 0.25s;
  }
  .pill.recording { background: #8b0000; }
  .pill.transcribing { background: #004d99; }
  .pill.done { background: #145214; }
  .pill.error { background: #7a1c1c; }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #ccc;
    flex-shrink: 0;
  }
  .recording .dot {
    background: #ff6b6b;
    animation: pulse 0.9s ease-in-out infinite;
  }
  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.25; }
  }

  .label { flex: 1; font-size: 0.9rem; }

  .gear {
    background: none;
    border: none;
    cursor: pointer;
    font-size: 1rem;
    color: #aaa;
    padding: 0;
  }
  .gear:hover { color: #fff; }

  .transcript {
    padding: 0.75rem;
    background: #1a1a2a;
    border-radius: 8px;
    font-size: 0.9rem;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .err { color: #ff6b6b; font-size: 0.82rem; }
  .warn { color: #e0a030; font-size: 0.82rem; }

  .settings {
    padding: 1rem;
    border: 1px solid #333;
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .settings label {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    font-size: 0.85rem;
    color: #aaa;
  }
  .settings input {
    padding: 0.4rem 0.6rem;
    border: 1px solid #444;
    border-radius: 5px;
    background: #111;
    color: #eee;
    font-size: 0.9rem;
  }
  .settings button {
    align-self: flex-start;
    padding: 0.4rem 1rem;
    background: #2563eb;
    color: white;
    border: none;
    border-radius: 5px;
    cursor: pointer;
    font-size: 0.85rem;
  }
  .settings button:hover { background: #1d4ed8; }
</style>
