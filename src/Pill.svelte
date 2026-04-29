<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { onMount } from "svelte";
  import { Mic, LoaderCircle, Check, CircleAlert } from "@lucide/svelte";

  type Status = "idle" | "recording" | "transcribing" | "done" | "error";

  let status: Status = $state("idle");

  const labels: Record<Status, string> = {
    idle: "",
    recording: "Recording",
    transcribing: "Transcribing",
    done: "Copied",
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
  <span class="icon">
    {#if status === "recording"}
      <span class="wave">
        <span></span><span></span><span></span><span></span>
      </span>
    {:else if status === "transcribing"}
      <LoaderCircle size={13} strokeWidth={2.4} class="spin" />
    {:else if status === "done"}
      <Check size={13} strokeWidth={2.6} />
    {:else if status === "error"}
      <CircleAlert size={13} strokeWidth={2.4} />
    {:else}
      <Mic size={13} strokeWidth={2.4} />
    {/if}
  </span>
  <span class="text">{labels[status]}</span>
</div>

<style>
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 0 14px;
    height: 36px;
    background: rgba(20, 20, 24, 0.92);
    backdrop-filter: blur(20px) saturate(160%);
    -webkit-backdrop-filter: blur(20px) saturate(160%);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 18px;
    color: rgba(255, 255, 255, 0.92);
    font-family: 'Geist', -apple-system, BlinkMacSystemFont, system-ui, sans-serif;
    font-size: 12.5px;
    font-weight: 600;
    letter-spacing: -0.15px;
    font-feature-settings: 'ss01';
    animation: pillEnter 0.4s cubic-bezier(0.22, 1, 0.36, 1);
    user-select: none;
    white-space: nowrap;
    box-shadow:
      0 8px 32px rgba(0, 0, 0, 0.55),
      0 1px 3px rgba(0, 0, 0, 0.4),
      inset 0 1px 0 rgba(255, 255, 255, 0.08);
    transition: background 0.2s ease, border-color 0.2s ease, color 0.2s ease;
  }

  .pill.recording {
    background: rgba(50, 14, 14, 0.92);
    border-color: rgba(255, 69, 58, 0.35);
    color: #FFB4AE;
  }
  .pill.transcribing {
    background: rgba(12, 24, 52, 0.92);
    border-color: rgba(10, 132, 255, 0.35);
    color: #9EC9FF;
  }
  .pill.done {
    background: rgba(12, 36, 22, 0.92);
    border-color: rgba(48, 209, 88, 0.35);
    color: #95E8B0;
  }
  .pill.error {
    background: rgba(50, 14, 14, 0.92);
    border-color: rgba(255, 69, 58, 0.35);
    color: #FFB4AE;
  }

  .icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
  }

  .wave { display: inline-flex; align-items: center; gap: 2px; height: 13px; }
  .wave span {
    width: 2px; border-radius: 2px;
    background: currentColor;
    animation: wave 0.9s ease-in-out infinite;
  }
  .wave span:nth-child(1) { height: 50%; animation-delay: -0.3s; }
  .wave span:nth-child(2) { height: 100%; animation-delay: -0.15s; }
  .wave span:nth-child(3) { height: 75%; animation-delay: 0s; }
  .wave span:nth-child(4) { height: 40%; animation-delay: -0.2s; }

  @keyframes wave {
    0%, 100% { transform: scaleY(0.5); }
    50% { transform: scaleY(1); }
  }

  @keyframes pillEnter {
    from { opacity: 0; transform: translateY(6px) scale(0.96); }
    to { opacity: 1; transform: translateY(0) scale(1); }
  }

  :global(.spin) { animation: spin 0.9s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
