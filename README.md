# Yap

Voice-to-text dictation for Linux, Windows, and macOS. Press a hotkey, speak, press again — your words are typed into whatever app is focused.

Built with Tauri 2 + Rust + Svelte. Audio is captured locally via `cpal`, transcribed by Groq's Whisper API, and injected via clipboard + simulated paste.

---

## How it works

```
Ctrl+Shift+Space        Ctrl+Shift+Space
      ↓                       ↓
 start recording          stop recording
      ↓                       ↓
 cpal captures mic      write 16kHz WAV
 at 16kHz mono               ↓
                        POST to Groq API
                        (whisper-large-v3-turbo)
                             ↓
                        copy transcript → clipboard
                             ↓
                        xdotool / ydotool
                        simulates Ctrl+V
```

---

## Prerequisites

### All platforms
- [Rust](https://rustup.rs) (stable toolchain)
- [Node.js](https://nodejs.org) v18+
- [pnpm](https://pnpm.io) (`npm i -g pnpm`)
- A [Groq API key](https://console.groq.com) (free tier is generous)

### Linux (Debian/Ubuntu)
```bash
sudo apt-get install -y \
  pkg-config \
  libwebkit2gtk-4.1-dev \
  libappindicator3-dev \
  librsvg2-dev \
  libgtk-3-dev \
  libssl-dev \
  libasound2-dev \
  xdotool          # X11
  # or: ydotool   # Wayland
```

### Windows
No extra system deps. Rust + pnpm is enough. Text injection uses the `enigo` crate (planned — currently clipboard-only on Windows).

### macOS
```bash
# Xcode command line tools (if not already installed)
xcode-select --install
```
Text injection uses `enigo` (planned — currently clipboard-only on macOS).

---

## Setup

```bash
# 1. Clone
git clone https://github.com/yourname/yap
cd yap

# 2. Install JS dependencies
pnpm install

# 3. Run in development mode
pnpm tauri dev
```

On first launch, click **⚙** in the app window and paste your Groq API key (`gsk_...`), then click **Save**.

---

## Usage

| Action | Result |
|--------|--------|
| `Ctrl+Shift+Space` | Start recording (pill turns red) |
| `Ctrl+Shift+Space` again | Stop → transcribe → paste into focused window |
| `⚙` button | Open/close settings |

The transcript also appears in the app window for reference.

---

## Build for distribution

```bash
pnpm tauri build
```

Output is in `src-tauri/target/release/bundle/`:
- Linux: `.deb` and `.AppImage`
- Windows: `.msi` and `.exe`
- macOS: `.dmg` and `.app`

---

## Project structure

```
yap/
├── src/                        # Svelte frontend
│   ├── App.svelte              # Main UI — pill, settings, hotkey wiring
│   ├── main.ts                 # Svelte mount
│   └── app.css                 # Global reset
├── src-tauri/                  # Rust backend
│   ├── src/
│   │   ├── lib.rs              # App entry, Tauri builder, plugin registration
│   │   └── commands.rs         # All Tauri commands + audio + transcription logic
│   ├── Cargo.toml
│   ├── tauri.conf.json         # Window config, bundle targets
│   └── capabilities/
│       └── default.json        # Tauri 2 permission grants
├── vite.config.ts
├── svelte.config.js
└── package.json
```

---

## Key dependencies

| Crate / Package | Purpose |
|---|---|
| `tauri 2` | Native window + IPC bridge |
| `cpal 0.15` | Cross-platform audio capture |
| `hound 3` | Encode raw PCM → WAV |
| `reqwest 0.12` | HTTP client for Groq API |
| `tauri-plugin-global-shortcut` | System-wide hotkey |
| `tauri-plugin-clipboard-manager` | Write transcript to clipboard |
| `@tauri-apps/api` | JS ↔ Rust IPC |
| `svelte 5` | Reactive UI |

---

## Optimisations (planned / in progress)

### Audio quality
- [ ] **VAD (Voice Activity Detection)** — trim leading/trailing silence before sending to Groq. Reduces API payload and improves accuracy on short recordings.
- [ ] **Dynamic sample rate fallback** — if 16 kHz is not supported by the device, resample from 44.1/48 kHz using `rubato` or `dasp` instead of erroring out.
- [ ] **Noise gate** — discard frames below an amplitude threshold to cut background hiss.

### Transcription
- [ ] **Streaming transcription** — pipe audio chunks in real time instead of waiting for the full WAV. Groq supports chunked uploads; visible latency drops significantly.
- [ ] **Language hint** — pass `language` param to Whisper API to skip language detection and shave ~100 ms off every request.
- [ ] **Local fallback** — bundle `whisper.cpp` via a Rust binding (`whisper-rs`) so the app works offline. Groq stays as the fast path.
- [ ] **reqwest client reuse** — move `Client::new()` to app state so the HTTP connection pool is reused across requests (currently a new client is created every transcription).

### Paste / injection
- [ ] **Windows support** — replace the `xdotool` shell-out with the `enigo` crate for true cross-platform paste.
- [ ] **macOS support** — use `enigo` for `Cmd+V` simulation on macOS.
- [ ] **Direct text injection** — on Wayland, `ydotool` requires a running daemon. Add `wl-clipboard` + `xdg-portal` path as a fallback.
- [ ] **Focus-before-paste delay** — configurable delay (default 80 ms) between clipboard write and Ctrl+V so slow apps (Electron, browsers) don't miss the paste.

### UX
- [ ] **Overlay pill window** — second Tauri window: transparent, no decorations, always-on-top, positioned at screen bottom-centre. Shows recording/transcribing state without switching focus to the main window.
- [ ] **System tray** — tray icon with right-click menu: Settings, Quit. Main window hidden by default; shown only from tray.
- [ ] **Configurable hotkey** — let the user pick their own shortcut in settings instead of hard-coding `Ctrl+Shift+Space`.
- [ ] **Auto-start on login** — `tauri-plugin-autostart` to register the app as a login item.
- [ ] **Transcript history** — store last N transcripts locally with timestamps using `tauri-plugin-store`.

### Performance
- [ ] **Release build size** — enable LTO + `opt-level = "z"` + `strip = true` in `Cargo.toml` `[profile.release]` to shrink the binary.
- [ ] **Faster cold start** — lazy-initialise the cpal host on first recording rather than at app startup.

### Security
- [ ] **Encrypted key storage** — use the OS keychain (`keyring` crate) instead of a plaintext `api_key.txt` file.
- [ ] **Capability tightening** — scope Tauri capabilities to only the permissions actually used.

---

## Troubleshooting

**Shortcut not registering**
Another app may have claimed `Ctrl+Shift+Space`. Check with `xdotool` (`xdotool key ctrl+shift+space` in a terminal) or try a different combo.

**No input device**
Run `arecord -l` (Linux) to list capture devices. If none appear, check PipeWire/PulseAudio is running.

**`xdotool` paste not working**
Install it: `sudo apt-get install xdotool`. On Wayland, install `ydotool` and ensure `ydotoold` daemon is running.

**Groq API error 401**
Your API key is wrong or expired. Get a new one at [console.groq.com](https://console.groq.com).

**Groq API error 413 (payload too large)**
Recording was too long. Groq's free tier accepts up to 25 MB audio files (~25 min at 16 kHz 16-bit mono). Trim silence (VAD optimisation above) to stay well under.
