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
                           resample → 16 kHz
                                  ↓
                     ┌────────────┴────────────┐
                  Groq backend           Local backend
                  (cloud)                (offline)
                     ↓                        ↓
              POST WAV to Groq        whisper-rs runs
              whisper-large-v3-turbo  selected GGML model
                     └────────────┬────────────┘
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
- [Rust](https://rustup.rs) stable toolchain — install with `rustup`
- [Node.js](https://nodejs.org) v18+
- [pnpm](https://pnpm.io) — `npm i -g pnpm`

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
  cmake \
  libclang-dev \
  xdotool
  # or: ydotool   # Wayland paste (also needs ydotoold daemon)
```

> `cmake` and `libclang-dev` are required to compile `whisper.cpp` (the local Whisper backend) from source. `xdotool` is for X11 paste injection.

### Windows

Install the [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the **Desktop development with C++** workload — this provides MSVC, CMake, and the Windows SDK.

```powershell
# Or install via winget:
winget install Microsoft.VisualStudio.2022.BuildTools
winget install Kitware.CMake
winget install LLVM.LLVM   # provides libclang for bindgen
```

After installing LLVM, set the env var so bindgen can find it:
```powershell
[System.Environment]::SetEnvironmentVariable("LIBCLANG_PATH", "C:\Program Files\LLVM\bin", "User")
```

### macOS

```bash
xcode-select --install        # Xcode command-line tools (clang, make)
brew install cmake            # CMake for building whisper.cpp
```

---

## Setup & run

```bash
# 1. Clone
git clone https://github.com/xeven777/yap
cd yap

# 2. Install JS dependencies
pnpm install

# 3. Dev mode — hot-reload on both Rust and Svelte changes
pnpm tauri dev
```

The app starts in the **system tray**. Click the tray icon to open the settings window.

---

## First launch

### Using Groq (cloud, default)

1. Get a free API key at [console.groq.com](https://console.groq.com)
2. Open Settings (tray icon or **⚙** button) → paste key → **Save**

### Using Local Whisper (offline)

1. Open Settings → switch backend to **Local Whisper**
2. Pick a model and click **Download** — models are fetched from HuggingFace

| Model | Size | Notes |
|-------|------|-------|
| Tiny (Q5) | 31 MB | Fastest; good for quick phrases |
| Base | 142 MB | Balanced speed and accuracy |
| Small (Q5) | 181 MB | Better accuracy, still fast |
| Small | 466 MB | Full-precision small model |
| **Large Turbo (Q5)** | **547 MB** | **Recommended — same arch as Groq, quantized** |
| Large Turbo | 874 MB | Full-precision, highest quality |

3. Once downloaded, select the model and click **Save**
4. The model is cached in memory after first use so subsequent transcriptions are fast

Model files are stored in the app data directory:
- Linux: `~/.local/share/com.yap.app/models/`
- Windows: `%APPDATA%\com.yap.app\models\`
- macOS: `~/Library/Application Support/com.yap.app/models/`

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
| Backend | **Groq** (cloud, needs API key) or **Local Whisper** (offline, needs a downloaded model) |
| Groq API Key | Your `gsk_...` key from console.groq.com |
| Model | Which downloaded GGML model to use for local transcription |
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
| Windows | `.msi` + `.exe` (NSIS installer) |
| macOS | `.dmg` + `.app` |

The release profile uses LTO + dead-code stripping (`opt-level = "z"`, `strip = true`) to keep the binary small.

### Cross-compilation notes

Tauri does not support cross-compilation out of the box — build on the target OS. For CI, use:
- Linux: Ubuntu 22.04+ runner
- Windows: `windows-latest` runner
- macOS: `macos-latest` runner (arm64) or `macos-13` (x86_64)

---

## Project structure

```
yap/
├── index.html                  # Main window entry
├── pill.html                   # Overlay window entry
├── src/
│   ├── App.svelte              # Main UI — settings, hotkey, backend/model selector
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
| `hound 3` | Encode raw PCM → WAV (Groq path) |
| `reqwest 0.12` | HTTP client for Groq API + model downloads |
| `whisper-rs 0.14` | Rust bindings to whisper.cpp for local offline transcription |
| `tauri-plugin-global-shortcut` | System-wide configurable hotkey |
| `tauri-plugin-clipboard-manager` | Write transcript to clipboard |
| `enigo 0.2` | Keyboard injection on Windows + macOS |
| `@tauri-apps/api` | JS ↔ Rust IPC + event bus |
| `svelte 5` | Reactive UI (runes mode) |

---

## Audio pipeline detail

1. **Capture** — `cpal` opens the default input device at 16 kHz mono. Falls back to native rate + resamples afterward.
2. **Noise gate** — Samples below 0.5% amplitude are zeroed during capture to cut background hiss.
3. **VAD trim** — After recording stops, silence is trimmed from both ends (threshold 1%, 150 ms padding).
4. **Resample** — Linear interpolation to 16 kHz if the device captured at a different rate.
5. **Transcribe (Groq)** — `hound` writes a 16-bit PCM WAV to `/tmp/yap_recording.wav`; `reqwest` POSTs it to Groq's `/v1/audio/transcriptions`.
6. **Transcribe (Local)** — Raw `f32` samples are passed directly to `whisper-rs` (no WAV write). The `WhisperContext` is cached in memory so the model loads only once per session. Inference runs in a `spawn_blocking` thread to avoid blocking the async runtime.

---

## Troubleshooting

**App doesn't appear after launching**
It starts in the system tray. Look for the Yap icon in your taskbar/tray area.

**Hotkey not registering**
Another app may have claimed the combo. Change it in Settings — e.g. `Ctrl+Shift+D` or `Alt+F9`.

**Pill overlay not showing**
Make sure the main window is running. Check the terminal for `[yap]` log lines.

**No audio / "No input device"**
Run `arecord -l` (Linux) to list capture devices. Ensure PipeWire or PulseAudio is running: `systemctl --user status pipewire`.

**`xdotool` paste not working (X11)**
```bash
sudo apt-get install xdotool
```

**Wayland paste not working**
```bash
sudo apt-get install ydotool
sudo ydotoold &
```

**Build fails: `Unable to find libclang`**
```bash
# Linux
sudo apt-get install libclang-dev

# macOS
brew install llvm
export LIBCLANG_PATH="$(brew --prefix llvm)/lib"

# Windows — install LLVM from https://releases.llvm.org and set:
# LIBCLANG_PATH=C:\Program Files\LLVM\bin
```

**Build fails: `cmake not found`**
```bash
# Linux
sudo apt-get install cmake

# macOS
brew install cmake

# Windows
winget install Kitware.CMake
```

**Local model download stuck / slow**
HuggingFace occasionally rate-limits. Click Cancel and retry. Large models (547 MB+) take a few minutes on a typical connection.

**Local transcription is slow on first use**
The model is being loaded from disk into memory — this is a one-time cost per session. Subsequent transcriptions use the cached context and are much faster.

**Groq 401 error**
API key is wrong or expired. Generate a new one at [console.groq.com](https://console.groq.com).

**Groq 413 error (payload too large)**
Recording was very long. The free tier accepts up to 25 MB (~25 min at 16 kHz). VAD trimming reduces this automatically for typical short dictations.

---

## Remaining optimisations

- [ ] **Streaming transcription** — pipe audio chunks to a WebSocket ASR service; partial transcript visible while speaking
- [ ] **OS keychain for API key** — use the `keyring` crate instead of a plaintext config file
- [ ] **Auto-start on login** — `tauri-plugin-autostart`
- [ ] **Transcript history** — store last N transcripts with timestamps via `tauri-plugin-store`
- [ ] **Configurable VAD threshold** — expose noise gate and silence threshold in settings
