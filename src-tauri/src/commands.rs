use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tauri::{AppHandle, Manager, State};

// --- State -------------------------------------------------------------------

pub struct RecordingState {
    is_recording: Arc<Mutex<bool>>,
    samples: Arc<Mutex<Vec<f32>>>,
    stream: Mutex<Option<cpal::Stream>>,
    /// Actual capture rate — may differ from 16 kHz if device doesn't support it.
    capture_rate: Arc<Mutex<u32>>,
}

impl Default for RecordingState {
    fn default() -> Self {
        Self {
            is_recording: Arc::new(Mutex::new(false)),
            samples: Arc::new(Mutex::new(Vec::new())),
            stream: Mutex::new(None),
            capture_rate: Arc::new(Mutex::new(16_000)),
        }
    }
}

// cpal::Stream is !Send, but access is serialised by the Mutex.
unsafe impl Send for RecordingState {}
unsafe impl Sync for RecordingState {}

/// Shared HTTP client — keeps the connection pool alive across requests.
pub struct HttpClient(pub reqwest::Client);

// --- Commands ----------------------------------------------------------------

#[tauri::command]
pub fn start_recording(state: State<'_, RecordingState>) -> Result<(), String> {
    eprintln!("[yap] start_recording called");
    *state.is_recording.lock().unwrap() = true;
    state.samples.lock().unwrap().clear();

    let host = cpal::default_host();
    eprintln!("[yap] host: {}", host.id().name());

    let device = host
        .default_input_device()
        .ok_or("No input device available")?;
    eprintln!("[yap] device: {}", device.name().unwrap_or_default());

    // Try 16 kHz first; fall back to device default so recording always works.
    let (config, rate) = preferred_config(&device)?;
    *state.capture_rate.lock().unwrap() = rate;
    eprintln!("[yap] capture rate: {rate} Hz");

    let samples_w = Arc::clone(&state.samples);
    let is_rec_w = Arc::clone(&state.is_recording);

    let stream = device
        .build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if *is_rec_w.lock().unwrap() {
                    // Noise gate: zero-fill frames that are pure background hiss.
                    let gated: Vec<f32> = data
                        .iter()
                        .map(|&s| if s.abs() < NOISE_GATE_THRESHOLD { 0.0 } else { s })
                        .collect();
                    samples_w.lock().unwrap().extend_from_slice(&gated);
                }
            },
            |err| eprintln!("[yap] cpal error: {err}"),
            None,
        )
        .map_err(|e| e.to_string())?;

    stream.play().map_err(|e| e.to_string())?;
    *state.stream.lock().unwrap() = Some(stream);
    Ok(())
}

#[tauri::command]
pub async fn stop_recording(
    state: State<'_, RecordingState>,
    http: State<'_, HttpClient>,
    api_key: String,
    language: Option<String>,
) -> Result<String, String> {
    eprintln!("[yap] stop_recording called");
    *state.is_recording.lock().unwrap() = false;
    *state.stream.lock().unwrap() = None;

    let raw = state.samples.lock().unwrap().clone();
    let capture_rate = *state.capture_rate.lock().unwrap();
    eprintln!(
        "[yap] raw: {} samples @ {capture_rate} Hz ({:.1}s)",
        raw.len(),
        raw.len() as f32 / capture_rate as f32
    );

    if raw.is_empty() {
        return Err("No audio captured — try holding the shortcut a bit longer".into());
    }

    // 1. Trim leading/trailing silence.
    let trimmed = trim_silence(&raw, VAD_THRESHOLD, VAD_PADDING_MS, capture_rate);
    eprintln!("[yap] after VAD trim: {} samples", trimmed.len());

    if trimmed.is_empty() {
        return Err("Only silence detected — speak closer to the microphone".into());
    }

    // 2. Resample to 16 kHz if the device captured at a different rate.
    let resampled = resample_to_16k(trimmed, capture_rate);

    let wav_path = write_wav(&resampled)?;
    eprintln!("[yap] WAV: {:?} ({} samples @ 16kHz)", wav_path, resampled.len());

    transcribe(&http.0, &wav_path, &api_key, language.as_deref()).await
}

#[tauri::command]
pub fn get_api_key(app: AppHandle) -> String {
    key_path(&app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default()
        .trim()
        .to_string()
}

#[tauri::command]
pub fn save_api_key(app: AppHandle, key: String) -> Result<(), String> {
    let path = key_path(&app).ok_or("Could not resolve config dir")?;
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(path, key.trim()).map_err(|e| e.to_string())
}

/// Simulate Ctrl+V (or Cmd+V on macOS) in the currently-focused window.
#[tauri::command]
pub fn paste_text() -> Result<(), String> {
    paste_impl()
}

// --- Audio helpers -----------------------------------------------------------

/// Amplitude below which a sample is considered silence for the noise gate.
const NOISE_GATE_THRESHOLD: f32 = 0.005;

/// Amplitude below which a sample is considered silence for VAD edge trimming.
const VAD_THRESHOLD: f32 = 0.01;

/// Padding kept around the detected speech region (milliseconds).
const VAD_PADDING_MS: usize = 150;

/// Try 16 kHz mono; fall back to the device's preferred sample rate.
fn preferred_config(device: &cpal::Device) -> Result<(cpal::StreamConfig, u32), String> {
    const TARGET: u32 = 16_000;
    let config_16k = cpal::StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(TARGET),
        buffer_size: cpal::BufferSize::Default,
    };
    // Build a test stream to see if the device accepts 16 kHz.
    let dummy_samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let dummy_clone = Arc::clone(&dummy_samples);
    match device.build_input_stream(
        &config_16k,
        move |data: &[f32], _| { dummy_clone.lock().unwrap().extend_from_slice(data); },
        |_| {},
        None,
    ) {
        Ok(_) => return Ok((config_16k, TARGET)),
        Err(e) => eprintln!("[yap] 16 kHz not supported ({e}), falling back to device default"),
    }

    // Fall back to device default config.
    let default_cfg = device
        .default_input_config()
        .map_err(|e| e.to_string())?;
    let rate = default_cfg.sample_rate().0;
    let fallback = cpal::StreamConfig {
        channels: 1,
        sample_rate: default_cfg.sample_rate(),
        buffer_size: cpal::BufferSize::Default,
    };
    Ok((fallback, rate))
}

/// Trim silence from the start and end of a recording.
fn trim_silence(samples: &[f32], threshold: f32, padding_ms: usize, rate: u32) -> &[f32] {
    let pad = (padding_ms * rate as usize / 1000).max(1);
    let start = samples
        .iter()
        .position(|&s| s.abs() > threshold)
        .unwrap_or(0)
        .saturating_sub(pad);
    let end = samples
        .iter()
        .rposition(|&s| s.abs() > threshold)
        .map(|i| (i + pad).min(samples.len() - 1))
        .unwrap_or(samples.len() - 1);
    if start > end { &[] } else { &samples[start..=end] }
}

/// Linear-interpolation resample from `from_rate` to 16 kHz. No-op if rates match.
fn resample_to_16k(samples: &[f32], from_rate: u32) -> Vec<f32> {
    const TARGET: u32 = 16_000;
    if from_rate == TARGET {
        return samples.to_vec();
    }
    let ratio = from_rate as f64 / TARGET as f64;
    let out_len = (samples.len() as f64 / ratio) as usize;
    (0..out_len)
        .map(|i| {
            let pos = i as f64 * ratio;
            let idx = pos as usize;
            let frac = (pos - idx as f64) as f32;
            let a = samples.get(idx).copied().unwrap_or(0.0);
            let b = samples.get(idx + 1).copied().unwrap_or(a);
            a + (b - a) * frac
        })
        .collect()
}

fn write_wav(samples: &[f32]) -> Result<std::path::PathBuf, String> {
    let path = std::env::temp_dir().join("yap_recording.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&path, spec).map_err(|e| e.to_string())?;
    for &s in samples {
        writer
            .write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
            .map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())?;
    Ok(path)
}

// --- Transcription -----------------------------------------------------------

async fn transcribe(
    client: &reqwest::Client,
    path: &std::path::Path,
    api_key: &str,
    language: Option<&str>,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("No API key — open settings (⚙) and enter your Groq API key".into());
    }

    let bytes = tokio::fs::read(path).await.map_err(|e| e.to_string())?;

    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;

    let mut form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", "whisper-large-v3-turbo")
        .text("response_format", "json");

    // Skipping language detection saves ~100 ms per request.
    if let Some(lang) = language.filter(|l| !l.is_empty()) {
        form = form.text("language", lang.to_string());
    }

    eprintln!("[yap] POST to Groq (language: {:?})", language);
    let res = client
        .post("https://api.groq.com/openai/v1/audio/transcriptions")
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = res.status();
    let json: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    eprintln!("[yap] Groq status: {status}");

    if !status.is_success() {
        return Err(format!(
            "Groq API error {status}: {}",
            json["error"]["message"]
                .as_str()
                .unwrap_or(&json.to_string())
        ));
    }

    json["text"]
        .as_str()
        .map(|s| s.trim().to_string())
        .ok_or_else(|| format!("Unexpected Groq response: {json}"))
}

// --- Paste implementation (platform-specific) --------------------------------

#[cfg(target_os = "linux")]
fn paste_impl() -> Result<(), String> {
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        std::process::Command::new("ydotool")
            .args(["key", "29:1", "47:1", "47:0", "29:0"])
            .status()
            .map_err(|e| format!("ydotool failed: {e} — install ydotool for Wayland"))?;
    } else {
        std::process::Command::new("xdotool")
            .args(["key", "--clearmodifiers", "ctrl+v"])
            .status()
            .map_err(|e| format!("xdotool failed: {e} — install xdotool: sudo apt install xdotool"))?;
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn paste_impl() -> Result<(), String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;

    #[cfg(target_os = "macos")]
    let modifier = Key::Meta; // Cmd+V on macOS
    #[cfg(not(target_os = "macos"))]
    let modifier = Key::Control;

    enigo
        .key(modifier, Direction::Press)
        .map_err(|e| e.to_string())?;
    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| e.to_string())?;
    enigo
        .key(modifier, Direction::Release)
        .map_err(|e| e.to_string())?;
    Ok(())
}

// --- Config helpers ----------------------------------------------------------

#[tauri::command]
pub fn get_hotkey(app: AppHandle) -> String {
    config_read(&app, "hotkey.txt").unwrap_or_else(|| "Ctrl+Shift+Space".to_string())
}

#[tauri::command]
pub fn save_hotkey(app: AppHandle, hotkey: String) -> Result<(), String> {
    config_write(&app, "hotkey.txt", &hotkey)
}

#[tauri::command]
pub fn get_language(app: AppHandle) -> String {
    config_read(&app, "language.txt").unwrap_or_default()
}

#[tauri::command]
pub fn save_language(app: AppHandle, language: String) -> Result<(), String> {
    config_write(&app, "language.txt", &language)
}

// --- Config helpers ----------------------------------------------------------

fn key_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("api_key.txt"))
}

fn config_read(app: &AppHandle, filename: &str) -> Option<String> {
    let path = app.path().app_config_dir().ok()?.join(filename);
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn config_write(app: &AppHandle, filename: &str, value: &str) -> Result<(), String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(filename), value.trim()).map_err(|e| e.to_string())
}
