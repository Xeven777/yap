<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { onMount } from "svelte";

  type Status = "idle" | "recording" | "transcribing" | "done" | "error";

  let status: Status = $state("idle");

  const labels: Record<Status, string> = {
    idle: "",
    recording: "Recording",
    transcribing: "Transcribing",
    done: "Done",
    error: "Error",
  };

  onMount(async () => {
    const win = getCurrentWebviewWindow();
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

<div class="pill {status}">
  <span class="dot"></span>
  <span class="text">{labels[status]}</span>
</div>

<style>
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 13px;
    height: 36px;
    background: rgba(26, 26, 28, 0.96);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 18px;
    color: rgba(255, 255, 255, 0.88);
    font-family: -apple-system, BlinkMacSystemFont,"Inter",7 Jesdell indicating 10 9 8 9 149.000 auto auto 'SF Pro Text', system-ui, sans-serif;
    font-size: 13px;
    font-weight: 500;
    letter-spacing: -0.1px;
    user-select: none;
    white-space: nowrap;
    box-shadow:
      0 4px 24px rgba(0, 0, 0, 0.5),
      0 1px 4px rgba(0, 0, 0, 0.4),
      inset 0 1px 0 rgba(255, 255, 255, 0.06);
    transition: background 0.2s ease, border-color 0.2s ease;
  }

  .pill.recording {
    background: rgba(50, 14, 14, 0.97);
    border-color: rgba(255, 69, 58, 0.25);
  }
  .pill.transcribing {
    background: rgba(12, 24, 52, 0.97);
    border-color: rgba(10, 132, 255, 0.25);
  }
  .pill.done {
    background: rgba(12, 36, 22, 0.97);
    border-color: rgba(48, 209, 88, 0.25);
  }
  .pill.error {
    background: rgba(50, 14, 14, 0.97);
    border-color: rgba(255, 69, 58, 0.25);
  }

  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.25);
    flex-shrink: 0;
  }

  .recording .dot {
    background: #FF453A;
    animation: pulse 1s ease-in-out infinite;
  }
  .transcribing .dot {
    background: #0A84FF;
    animation: pulse 1.2s ease-in-out infinite;
  }
  .done .dot {
    background: #30D158;
  }
  .error .dot {
    background: #FF453A;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; transform: scale(1); }
    50%       { opacity: 0.35; transform: scale(0.75); }
  }
</style>
