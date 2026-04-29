<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { onMount } from "svelte";

  type Status = "idle" | "recording" | "transcribing" | "done" | "error";

  let status: Status = $state("idle");
  const win = getCurrentWebviewWindow();

  const label: Record<Exclude<Status, "idle">, string> = {
    recording: "Recording…",
    transcribing: "Transcribing…",
    done: "Done ✓",
    error: "Error",
  };

  onMount(async () => {
    await listen<Status>("yap://state", async (e) => {
      status = e.payload;
      if (status === "idle") {
        await win.hide();
      } else {
        await win.show();
      }
    });
  });
</script>

{#if status !== "idle"}
  <div class="pill {status}">
    <span class="dot"></span>
    <span class="text">{label[status]}</span>
  </div>
{/if}

<style>
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 10px 18px;
    border-radius: 999px;
    background: #1e1e2e;
    color: #fff;
    font-family: system-ui, -apple-system, sans-serif;
    font-size: 13px;
    font-weight: 500;
    white-space: nowrap;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.5);
    transition: background 0.2s ease;
    user-select: none;
  }

  .pill.recording    { background: #7f1d1d; }
  .pill.transcribing { background: #1e3a5f; }
  .pill.done         { background: #14532d; }
  .pill.error        { background: #7f1d1d; }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.5);
    flex-shrink: 0;
  }

  .recording .dot {
    background: #f87171;
    animation: pulse 0.9s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; transform: scale(1); }
    50%       { opacity: 0.3; transform: scale(0.85); }
  }
</style>
