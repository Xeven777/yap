<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { onMount } from "svelte";

  type Status = "idle" | "recording" | "transcribing" | "done" | "error";

  let status: Status = $state("idle");

  const labels: Record<Status, string> = {
    idle: "",
    recording: "Listening",
    transcribing: "Thinking...",
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

<div class="shell {status}">
  <div class="orb">
    <div class="core"></div>
    <div class="glint"></div>
  </div>

  {#if labels[status]}
    <span class="text">{labels[status]}</span>
  {/if}
</div>

<style>
  .shell {
    display: inline-flex;
    align-items: center;
    gap: 14px;
    padding: 10px 22px 10px 12px;
    background: rgba(14, 14, 18, 0.78);
    backdrop-filter: blur(22px) saturate(170%);
    -webkit-backdrop-filter: blur(22px) saturate(170%);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 999px;
    color: rgba(255, 255, 255, 0.92);
    font-family: 'Geist', -apple-system, BlinkMacSystemFont, system-ui, sans-serif;
    font-size: 13px;
    font-weight: 600;
    letter-spacing: -0.15px;
    user-select: none;
    white-space: nowrap;
    box-shadow:
      0 12px 40px rgba(0, 0, 0, 0.55),
      0 1px 2px rgba(0, 0, 0, 0.4),
      inset 0 1px 0 rgba(255, 255, 255, 0.06);
    animation: shellEnter 0.45s cubic-bezier(0.22, 1, 0.36, 1);
    transition: border-color 0.25s ease, color 0.25s ease;
  }

  .text { display: inline-block; }

  /* ------------ ORB ------------ */
  .orb {
    --hot:  #ffbf48;
    --warm: #ff7a2a;
    --deep: #be4a1d;
    --time: 1.6s;

    position: relative;
    width: 44px;
    height: 44px;
    border-radius: 50%;
    flex: none;
    background:
      radial-gradient(
        circle at 32% 30%,
        var(--hot) 0%,
        var(--warm) 35%,
        var(--deep) 75%,
        #2a0d05 100%
      );
    box-shadow:
      0 0 0 1px rgba(255, 255, 255, 0.05),
      0 0 18px 0 color-mix(in srgb, var(--hot) 50%, transparent),
      0 8px 24px 0 color-mix(in srgb, var(--deep) 60%, transparent),
      inset 0 -6px 12px 0 rgba(0, 0, 0, 0.45),
      inset 0 6px 10px 0 rgba(255, 255, 255, 0.18);
    animation: breathe var(--time) ease-in-out infinite;
    transition: background 0.4s ease, box-shadow 0.4s ease;
  }

  /* swirling inner blob — gives the lava-lamp feel without SVG mask */
  .core {
    position: absolute;
    inset: 14%;
    border-radius: 50%;
    background:
      radial-gradient(circle at 30% 30%, color-mix(in srgb, var(--hot) 90%, white 10%) 0%, transparent 55%),
      radial-gradient(circle at 70% 70%, color-mix(in srgb, var(--deep) 80%, black 10%) 0%, transparent 60%),
      conic-gradient(from 0deg, var(--hot), var(--warm), var(--deep), var(--warm), var(--hot));
    filter: blur(2px) saturate(1.2);
    mix-blend-mode: screen;
    opacity: 0.9;
    animation: swirl calc(var(--time) * 1.6) linear infinite;
  }

  /* glossy specular highlight */
  .glint {
    position: absolute;
    top: 10%;
    left: 18%;
    width: 38%;
    height: 24%;
    border-radius: 50%;
    background: radial-gradient([Motor]
      ellipse at center,
      rgba(255, 255, 255, 0.7) 0%,
      rgba(255, 255, 255, 0.15) 45%,
      transparent 70%
    );
    filter: blur(1px);
    pointer-events: none;
  }

  /* ------------ STATES ------------ */
  /* recording — warm amber, breathe + jiggle */
  .shell.recording { border-color: rgba(255, 120, 60, 0.32); color: #FFD7B0; }
  .shell.recording .orb {
    --hot: #ffd27a;
    --warm: #ff7a2a;
    --deep: #b8401a;
    --time: 1.4s;
    animation:
      breathe var(--time) ease-in-out infinite,
      jiggle 0.55s ease-in-out infinite;
  }

  /* transcribing — cool indigo/cyan, morphing blob */
  .shell.transcribing { border-color: rgba(120, 160, 255, 0.32); color: #C4D6FF; }
  .shell.transcribing .orb {
    --hot: #a8c5ff;
    --warm: #6c7cf0;
    --deep: #3a2bb0;
    --time: 1s;
    animation: morph 1.4s ease-in-out infinite;
  }
  .shell.transcribing .core {
    animation: swirl 0.9s linear infinite;
  }

  /* done — emerald, settled */
  .shell.done { border-color: rgba(80, 220, 140, 0.34); color: #B6F0CB; }
  .shell.done .orb {
    --hot: #b8f0c8;
    --warm: #4cc97a;
    --deep: #1f7a44;
    --time: 2.6s;
    animation:
      breathe var(--time) ease-in-out infinite,
      settle 0.55s cubic-bezier(0.22, 1, 0.36, 1);
  }

  /* error — crimson flicker */
  .shell.error { border-color: rgba(255, 80, 80, 0.42); color: #FFB4AE; }
  .shell.error .orb {
    --hot: #ffb0b0;
    --warm: #e23b3b;
    --deep: #7a0f0f;
    --time: 0.6s;
    animation:
      breathe var(--time) ease-in-out infinite,
      flicker 0.18s steps(2, end) infinite;
  }

  /* ------------ KEYFRAMES ------------ */
  @keyframes breathe {
    0%, 100% { transform: scale(1); }
    50%      { transform: scale(1.06); }
  }

  @keyframes jiggle {
    0%, 100% { translate: 0 0; }
    25%      { translate: 0.8px -0.5px; }
    50%      { translate: -0.6px 0.6px; }
    75%      { translate: 0.5px 0.7px; }
  }

  @keyframes swirl {
    0%   { transform: rotate(0deg); }
    100% { transform: rotate(360deg); }
  }

  @keyframes morph {
    0%, 100% { border-radius: 50%; transform: scale(1) rotate(0deg); }
    25%      { border-radius: 42% 58% 55% 45% / 50% 45% 55% 50%; transform: scale(1.05) rotate(8deg); }
    50%      { border-radius: 60% 40% 45% 55% / 55% 60% 40% 45%; transform: scale(0.96) rotate(-4deg); }
    75%      { border-radius: 45% 55% 60% 40% / 40% 50% 50% 60%; transform: scale(1.04) rotate(6deg); }
  }

  @keyframes settle {
    0%   { transform: scale(0.85); }
    60%  { transform: scale(1.1); }
    100% { transform: scale(1); }
  }

  @keyframes flicker {
    0%, 100% { transform: translate(0, 0); }
    50%      { transform: translate(0.8px, -0.8px); }
  }

  @keyframes shellEnter {
    from { opacity: 0; transform: translateY(8px) scale(0.94); }
    to   { opacity: 1; transform: translateY(0) scale(1); }
  }
</style>
