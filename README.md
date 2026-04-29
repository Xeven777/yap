# Yap

Voice-to-text dictation for Linux, Windows, and macOS. Press a global hotkey, speak, press again — your words are typed into whatever app is focused. Runs silently in the system tray with a transparent floating overlay that shows recording state.

Built with Tauri 2 + Rust + Svelte.

---

## How it works

```
Hotkey (press)              Hotkey (press again)
      ↓                           ↓
 start recording             stop recording
      ↓                           ↓
 cpal captures mic         noise gate applied
 at 16 kHz mono                   ↓
                           VAD trims silence
                                  ↓
                           resample → 16 kHz WAV
                                  ↓
                           POST to Groq API
                           (whisper-large-v3-turbo)
                                  ↓
                           transcript → clipboard
                                  ↓
                           xdotool / ydotool / enigo
                           simulates Ctrl+V (Cmd+V on Mac)
```

The pill overlay (transparent, always-on-top) updates in real time via Tauri events from the main app window.

---

## Prerequisites

### All platforms
- [Rust](https://rustup.rs) stable toolchain (`rustup` + `cargo`)
- [Node.js](https://nodejs.org) v18+
- [pnpm](https://pnpm.io) — install with `npm i -g pnpm`
- A [Groq API key](https://console.groq.com) — free tier, very generous limits

### Linux (Debian / Ubuntu / MX Linux)
```bash
sudo apt-get install -y \
  pkg-config \
  libwebkit2gtk-4.1-dev \
  libappindicator3-dev \
  librsvg2-dev \
  libgtk-3-dev \
  libssl-dev \
  libasound2-dev \
  xdotool          # X11 paste
  # or: ydotool   # Wayland paste (also needs ydotoold daemon running)
```

### Windows
No extra system deps. Paste injection uses the `enigo` crate (no `xdotool` needed).

### macOS
```bash
xcode-select --install   # if not already installed
```
Paste injection uses `enigo` with `Cmd+V` automatically.

---

## Setup

```bash
# 1. Clone
git clone https://github.com/xeven777/yap
cd yap

# 2. Install JS dependencies
pnpm install

# 3. Run in development mode (hot-reload on both Rust and Svelte changes)
pnpm tauri dev
```

The app starts in the **system tray** — look for the Yap icon. Click it to open Settings.

On first launch, click the tray icon → **Settings**, paste your Groq API key (`gsk_...`), and click **Save**.

---

## Usage

| Action | Result |
|--------|--------|
| Hotkey (default: `Ctrl+Shift+Space`) | Toggle recording on/off |
| Hotkey pressed once | Starts recording — pill overlay appears |
| Hotkey pressed again | Stops recording, transcribes, pastes |
| Click tray icon | Show settings window |
| Click **−** in settings window | Hide window back to tray |
| Click **×** on settings window | Also hides to tray (does not quit) |
| Tray right-click → Quit | Exit the app |

The pill overlay (bottom-centre of screen, transparent) shows:
- **Red + pulsing dot** — Recording
- **Blue** — Transcribing
- **Green** — Done
- Disappears when idle

---

## Settings

Open via tray icon or the **⚙** button in the main window.

| Setting | Description |
|---------|-------------|
| Groq API Key | Your `gsk_...` key from console.groq.com |
| Hotkey | Any combo recognised by your OS — e.g. `Ctrl+Shift+D`, `Alt+Space` |
| Language | ISO code (`en`, `fr`, `de`…) — skips auto-detect, saves ~100 ms. Leave blank for auto. |

Settings are saved to the OS app-config directory:
- Linux: `~/.config/com.yap.app/`
- Windows: `%APPDATA%\com.yap.app\`
- macOS: `~/Library/Application Support/com.yap.app/`

---

## Build for distribution

```bash
pnpm tauri build
```

Output is in `src-tauri/target/release/bundle/`:

| Platform | Output |
|----------|--------|
| Linux | `.deb` + `.AppImage` |
| Windows | `.msi` + `.exe` |
| macOS | `.dmg` + `.app` |

The release profile uses LTO + dead-code stripping (`opt-level = "z"`, `strip = true`) to keep the binary small.

---

## Project structure

```
yap/
├── index.html                  # Main window entry
├── pill.html                   # Overlay window entry
├── src/
│   ├── App.svelte              # Main UI — settings, hotkey wiring, event emission
│   ├── Pill.svelte             # Floating overlay — listens for yap://state events
│   ├── main.ts                 # Svelte mount for main window
│   ├── pill.ts                 # Svelte mount for pill window
│   └── app.css                 # Global reset
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs              # Tauri builder, tray setup, close-to-tray handler
│   │   └── commands.rs         # All Tauri commands + audio pipeline + transcription
│   ├── Cargo.toml              # Rust deps + release profile
│   ├── tauri.conf.json         # Window config (main + pill), bundle targets
│   └── capabilities/
│       └── default.json        # Tauri 2 permission grants for both windows
├── vite.config.ts              # Multi-page build (main + pill entry points)
├── svelte.config.js
└── package.json
```

---

## Key dependencies

| Crate / Package | Purpose |
|---|---|
| `tauri 2` | Native window, IPC bridge, system tray |
| `cpal 0.15` | Cross-platform audio capture (ALSA / PipeWire / WASAPI / CoreAudio) |
| `hound 3` | Encode raw PCM → WAV |
| `reqwest 0.12` | HTTP client for Groq API (connection pool reused across requests) |
| `tauri-plugin-global-shortcut` | System-wide configurable hotkey |
| `tauri-plugin-clipboard-manager` | Write transcript to clipboard |
| `enigo 0.2` | Keyboard injection on Windows + macOS |
| `@tauri-apps/api` | JS ↔ Rust IPC + event bus |
| `svelte 5` | Reactive UI (runes mode) |

---

## Audio pipeline detail

1. **Capture** — `cpal` opens the default input device at 16 kHz mono. If the device doesn't support 16 kHz, the native rate is used and the samples are resampled afterward via linear interpolation.
2. **Noise gate** — Samples below 0.5% amplitude are zeroed during capture to cut background hiss before it reaches the buffer.
3. **VAD trim** — After recording stops, silence is trimmed from both ends of the buffer (threshold 1%, 150 ms padding). Whisper gets clean speech edges.
4. **Resample** — If captured at a rate other than 16 kHz, the buffer is resampled to 16 kHz using linear interpolation.
5. **WAV encode** — `hound` writes a 16-bit signed PCM WAV to `/tmp/yap_recording.wav`.
6. **Transcribe** — `reqwest` POSTs the WAV to Groq's `/v1/audio/transcriptions` endpoint. The HTTP client is shared (connection pool persists across requests).

---

## Troubleshooting

**App doesn't appear after launching**
It starts in the system tray. Look for the Yap icon in your taskbar/tray area.

**Hotkey not registering**
Another app may have claimed the combo. Change it in Settings to something unused — e.g. `Ctrl+Shift+D` or `Alt+F9`.

**Pill overlay not showing**
Make sure the main window is running (not crashed). Check the terminal for `[yap]` log lines.

**No audio / "No input device"**
Run `arecord -l` (Linux) to list capture devices. Ensure PipeWire or PulseAudio is running: `systemctl --user status pipewire`.

**`xdotool` paste not working (X11)**
```bash
sudo apt-get install xdotool
```

**Wayland paste not working**
Install `ydotool` and start the daemon:
```bash
sudo apt-get install ydotool
sudo ydotoold &
```

**Groq 401 error**
API key is wrong or expired. Generate a new one at [console.groq.com](https://console.groq.com).

**Groq 413 error (payload too large)**
Recording was very long. The free tier accepts up to 25 MB (~25 min at 16 kHz). VAD trimming reduces this automatically for typical short dictations.

---

## Remaining optimisations

- [ ] **Streaming transcription** — pipe audio chunks in real time; visible latency drops significantly
- [ ] **Local Whisper fallback** — bundle `whisper.cpp` via `whisper-rs` for offline use
- [ ] **OS keychain for API key** — use the `keyring` crate instead of a plaintext config file
- [ ] **Auto-start on login** — `tauri-plugin-autostart`
- [ ] **Transcript history** — store last N transcripts with timestamps via `tauri-plugin-store`
- [ ] **Configurable VAD threshold** — expose noise gate and silence threshold in settings
