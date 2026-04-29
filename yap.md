
# Yap — Project Plan

**Voice-to-text dictation app · Tauri 2 + Rust · Linux / Windows / macOS**

---

## Tech Stack
- **Tauri 2**  
- **Rust**  
- **Svelte (UI)**  
- **cpal (audio)**  
- **whisper.cpp**  
- **Groq API**  
- **xdotool / ydotool**  
- **Global hotkey**  
- **System tray**

---

## Project Phases

### Phase 01: Environment setup & Tauri hello world *(Week 1)*
**Tasks:**
- Install Rust toolchain — rustup, cargo — the Rust package manager. This is your build tool.
- Install Tauri prerequisites — Node.js + pnpm, then `cargo install tauri-cli`. Tauri wraps your frontend in a native window.
- Scaffold the project — Run `cargo tauri init`, pick Svelte as frontend. This generates the folder structure.
- Understand the folder layout — `src-tauri/` is Rust backend. `src/` is your Svelte UI. They talk via Tauri commands.
- Run the dev server — `cargo tauri dev` — hot-reloads both Rust and UI simultaneously.
- Write your first Tauri command — A Rust function tagged `#[tauri::command]` that returns a string to Svelte. Just to feel the IPC.

✅ **Deliverable:** App window opens, button in Svelte calls Rust and gets a response

---

### Phase 02: Audio capture from microphone *(Week 2)*
**Tasks:**
- Add `cpal` to Cargo.toml — cpal is a cross-platform audio I/O library for Rust. Works on ALSA, PipeWire, WASAPI, CoreAudio.
- List available input devices — cpal lets you enumerate mics. Print them to console — learn how devices are named per OS.
- Record audio on keypress — Start a recording stream, collect raw f32 samples into a Vec. Stop on key release.
- Convert samples to WAV — Use the `hound` crate to write a .wav file from raw PCM. Groq API expects audio files.
- Expose record/stop as Tauri commands — Svelte calls `invoke('start_recording')` and `invoke('stop_recording')`. Rust handles the cpal logic.
- Test audio quality — Record 5 seconds, play it back with VLC. Make sure sample rate is 16kHz (Whisper's preferred rate).

✅ **Deliverable:** Press a button → Rust records from mic → saves a .wav file you can play back

---

### Phase 03: Transcription via Groq API *(Week 3)*
**Tasks:**
- Sign up for Groq and get an API key — Free tier is very generous. Store the key in a local config file (not hardcoded).
- Send WAV to Groq using `reqwest` — `reqwest` is Rust's HTTP client. Send a multipart/form-data POST to Groq's transcription endpoint.
- Parse the JSON response — Groq returns `{ text: '...' }`. Use `serde_json` to deserialize it.
- Return transcript to Svelte — Tauri command returns the transcript string. Display it in a text area in the UI first.
- Add API key settings screen — Svelte form that saves the key to a local JSON config via `tauri-plugin-store`.
- Handle errors gracefully — No network? Bad key? Show a friendly error in the overlay — don't crash.

✅ **Deliverable:** Record voice → transcript appears in the UI window

---

### Phase 04: Global hotkey + text injection *(Week 4)*
**Tasks:**
- Add `tauri-plugin-global-shortcut` — Lets you register a hotkey (Right Ctrl, for example) that fires even when the app is not focused.
- Wire hotkey to start/stop recording — Hold = start recording, release = stop and transcribe. Use the plugin's `on_shortcut` callback in Rust.
- Write transcript to clipboard — Use `tauri-plugin-clipboard-manager` to set the system clipboard to the transcript text.
- Simulate Ctrl+V paste — On Linux: shell out to `xdotool key ctrl+v`. On Windows: use `enigo` crate. On macOS: `enigo` too.
- Test in different apps — Try pasting into VS Code, a browser textarea, terminal. Fix edge cases per app.
- Add Wayland fallback detection — Check `$WAYLAND_DISPLAY` env var. If set, use `wl-paste` + ydotool instead of xdotool.

✅ **Deliverable:** Hold hotkey anywhere → speak → release → text appears in focused app

---

### Phase 05: Overlay pill UI + system tray *(Week 5–6)*
**Tasks:**
- Create a second Tauri window for the overlay — In tauri.conf.json add a second window: transparent, no decorations, always_on_top. This is the pill.
- Build the pill in Svelte — A small horizontal bar with a pulsing dot and state label (Recording… / Transcribing… / Done). CSS animation.
- Show/hide overlay via Tauri events — Rust emits events like `recording-start` and `transcription-done`. Svelte listens and reacts.
- Add system tray icon — `tauri-plugin-tray` gives you a tray icon with a right-click menu: Settings, Quit.
- Add settings UI in main window — Hotkey selector, API key input, model selector (Groq / local). Show/hide via tray.
- Bundle and test on Linux + Windows — `cargo tauri build` creates platform-specific installers. Test the .deb on MX Linux, .exe on Windows.

✅ **Deliverable:** Full end-to-end app with pill overlay, tray icon, hotkey, and working injection

---

## Core Data Flow
1. **Hotkey pressed**  
   → `tauri-plugin-global-shortcut`

2. **Audio capture**  
   → `cpal → WAV buffer in Rust`

3. **Transcription**  
   → `Groq API (v1) → whisper.cpp (v2)`

4. **Text injection**  
   → `Clipboard + Ctrl+V simulation`

5. **Overlay pill**  
   → `Svelte · always-on-top window`

---

## OS Support
- 🔵 **Linux (primary)**
- 🔵 **Windows**
- 🔵 **macOS**
