<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { onMount } from "svelte";

  type Status = "idle" | "recording" | "transcribing" | "done" | "error";

  let status: Status = $state("idle");

  const labels: Record<Status, string> = {
    idle: "",
    recording: "Listening",
    transcribing: "Thinking",
    done: "Copied!",
    error: "Oops",
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
  <div class="loader" aria-hidden="true">
    <span class="ball a"></span>
    <span class="ball b"></span>
  </div>

  {#if labels[status]}
    <span class="text">{labels[status]}</span>
  {/if}
</div>

<style>
  .shell {
    --ink: #231e17;
    --yellow: #fcf300;
    --blue: #058ed9;
    --white: #ffffff;
    --red: #ff5b5b;

    --bg: var(--ink);
    --fg: var(--yellow);
    --ball-1: var(--yellow);
    --ball-2: var(--blue);

    display: inline-flex;
    align-items: center;
    gap: 14px;
    padding: 16px 22px 16px 14px;
    background: var(--bg);
    border: 3px solid var(--fg);
    border-radius: 999px;
    color: var(--fg);
    font-family: 'Cherry Bomb One', 'Poppins', system-ui, sans-serif;
    font-size: 17px;
    font-weight: 400;
    letter-spacing: 0.5px;
    user-select: none;
    white-space: nowrap;
    box-shadow:
      4px 4px 0 var(--fg),
      0 14px 30px rgba(0, 0, 0, 0.45);
    animation: shellEnter 0.45s cubic-bezier(0.34, 1.56, 0.64, 1);
    transition:
      background 0.3s ease,
      color 0.3s ease,
      border-color 0.3s ease,
      box-shadow 0.3s ease;
  }

  .text { display: inline-block; line-height: 1; }

  /* ------------ LOADER (two-ball orbit) ------------ */
  .loader {
    position: relative;
    width: 44px;
    height: 24px;
    flex: none;
    animation: rotate 1.4s linear infinite;
  }
  .ball {
    position: absolute;
    top: 0;
    left: 0;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: 2.5px solid var(--ink);
    box-sizing: border-box;
  }
  .ball.a {
    background: var(--ball-1);
    animation: orbit-a 1.4s ease-in-out infinite;
  }
  .ball.b {
    background: var(--ball-2);
    animation: orbit-b 1.4s ease-in-out infinite;
  }

  /* ------------ STATES ------------ */
  .shell.idle .loader { animation-duration: 4s; }

  /* recording — dark bg, yellow + blue balls bounce */
  .shell.recording {
    --bg: var(--ink);
    --fg: var(--yellow);
    --ball-1: var(--yellow);
    --ball-2: var(--blue);
  }
  .shell.recording .loader { animation-duration: 1.1s; }
  .shell.recording .ball.a,
  .shell.recording .ball.b { animation-duration: 1.1s; }

  /* transcribing — yellow bg, ink + blue balls */
  .shell.transcribing {
    --bg: var(--yellow);
    --fg: var(--ink);
    --ball-1: var(--blue);
    --ball-2: var(--ink);
  }
  .shell.transcribing .ball { border-color: var(--ink); }
  .shell.transcribing .loader { animation-duration: 0.85s; }
  .shell.transcribing .ball.a,
  .shell.transcribing .ball.b { animation-duration: 0.85s; }

  /* done — yellow bg, solid + cheerful */
  .shell.done {
    --bg: var(--yellow);
    --fg: var(--ink);
    --ball-1: var(--ink);
    --ball-2: var(--blue);
  }
  .shell.done .ball { border-color: var(--ink); }
  .shell.done .loader { animation: settle 0.6s cubic-bezier(0.34, 1.56, 0.64, 1) both; }
  .shell.done .ball.a,
  .shell.done .ball.b { animation-play-state: paused; }

  /* error — red bg, white loader */
  .shell.error {
    --bg: var(--red);
    --fg: var(--white);
    --ball-1: var(--white);
    --ball-2: var(--yellow);
  }
  .shell.error .loader {
    animation: shake 0.4s ease-in-out infinite;
  }

  /* ------------ KEYFRAMES ------------ */
  @keyframes rotate {
    0% { transform: rotate(0deg); }
    100% { transform: rotate(360deg); }
  }

  /* ball A: top-left → bottom-right → top-left */
  @keyframes orbit-a {
    0%, 100% {
      transform: translate(0, 0) scale(1);
    }
    50% {
      transform: translate(26px, 6px) scale(0.85);
    }
  }
  /* ball B: bottom-right → top-left → bottom-right (offset start) */
  @keyframes orbit-b {
    0%, 100% {
      transform: translate(26px, 6px) scale(1);
    }
    50% {
      transform: translate(0, 0) scale(0.85);
    }
  }

  @keyframes settle {
    0%   { transform: rotate(0deg) scale(0.8); }
    60%  { transform: rotate(180deg) scale(1.15); }
    100% { transform: rotate(180deg) scale(1); }
  }

  @keyframes shake {
    0%, 100% { transform: translateX(0) rotate(0); }
    25% { transform: translateX(-2px) rotate(-4deg); }
    75% { transform: translateX(2px) rotate(4deg); }
  }

  @keyframes shellEnter {
    from { opacity: 0; transform: translateY(8px) scale(0.92); }
    to   { opacity: 1; transform: translateY(0) scale(1); }
  }
</style>
