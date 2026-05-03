use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

// --- Recording state ---------------------------------------------------------

pub struct RecordingState {
    is_recording: Arc<Mutex<bool>>,
    samples: Arc<Mutex<Vec<f32>>>,
    stream: Mutex<Option<cpal::Stream>>,
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
pub struct HttpClient(pub reqwest::Client);

// --- Local Whisper state -----------------------------------------------------

pub struct LocalWhisperState {
    /// Cached (model_path, context) — reloaded only when model changes.
    cached: Mutex<Option<(String, Arc<WhisperContext>)>>,
    cancel: Arc<AtomicBool>,
    downloading: Arc<AtomicBool>,
}

impl Default for LocalWhisperState {
    fn default() -> Self {
        Self {
            cached: Mutex::new(None),
            cancel: Arc::new(AtomicBool::new(false)),
            downloading: Arc::new(AtomicBool::new(false)),
        }
    }
}

unsafe impl Send for LocalWhisperState {}
unsafe impl Sync for LocalWhisperState {}

// --- Model catalogue ---------------------------------------------------------

#[derive(Serialize, Clone)]
pub struct ModelEntry {
    pub id: &'static str,
    pub label: &'static str,
    pub filename: &'static str,
    /// Approximate download size in bytes.
    pub size_bytes: u64,
    pub recommended: bool,
}

pub const MODELS: &[ModelEntry] = &[
    ModelEntry {
        id: "ggml-tiny-q5_1",
        label: "Tiny (Q5) — 31 MB",
        filename: "ggml-tiny-q5_1.bin",
        size_bytes: 31_953_664,
        recommended: false,
    },
    ModelEntry {
        id: "ggml-base",
        label: "Base — 142 MB",
        filename: "ggml-base.bin",
        size_bytes: 147_951_465,
        recommended: false,
    },
    ModelEntry {
        id: "ggml-small-q5_1",
        label: "Small (Q5) — 181 MB",
        filename: "ggml-small-q5_1.bin",
        size_bytes: 189_892_584,
        recommended: false,
    },
    ModelEntry {
        id: "ggml-small",
        label: "Small — 466 MB",
        filename: "ggml-small.bin",
        size_bytes: 487_601_465,
        recommended: false,
    },
    ModelEntry {
        id: "ggml-large-v3-turbo-q5_0",
        label: "Large Turbo (Q5) — 547 MB",
        filename: "ggml-large-v3-turbo-q5_0.bin",
        size_bytes: 573_645_800,
        recommended: true,
    },
    ModelEntry {
        id: "ggml-large-v3-turbo",
        label: "Large Turbo — 874 MB",
        filename: "ggml-large-v3-turbo.bin",
        size_bytes: 916_716_544,
        recommended: false,
    },
];

const HF_BASE: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

// --- Event payloads ----------------------------------------------------------

#[derive(Serialize, Clone)]
struct DownloadProgress {
    model_id: String,
    downloaded: u64,
    total: u64,
}

#[derive(Serialize, Clone)]
struct DownloadDone {
    model_id: String,
}

#[derive(Serialize, Clone)]
struct DownloadError {
    model_id: String,
    error: String,
}

// --- Model status (list_models response) -------------------------------------

#[derive(Serialize, Clone)]
pub struct ModelStatus {
    pub id: &'static str,
    pub label: &'static str,
    pub filename: &'static str,
    pub size_bytes: u64,
    pub recommended: bool,
    pub downloaded: bool,
}

// --- Commands ----------------------------------------------------------------

#[tauri::command]
pub fn start_recording(state: State<'_, RecordingState>) -> Result<(), String> {
    eprintln!("[yap] start_recording called");
    *state.is_recording.lock().unwrap() = true;
    state.samples.lock().unwrap().clear();

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or("No input device available")?;
    eprintln!("[yap] device: {}", device.name().unwrap_or_default());

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
    whisper: State<'_, LocalWhisperState>,
    app: AppHandle,
    api_key: String,
    language: Option<String>,
    backend: String,
    model_id: Option<String>,
) -> Result<String, String> {
    eprintln!("[yap] stop_recording called (backend={backend})");
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

    let trimmed = trim_silence(&raw, VAD_THRESHOLD, VAD_PADDING_MS, capture_rate);
    eprintln!("[yap] after VAD trim: {} samples", trimmed.len());

    if trimmed.is_empty() {
        return Err("Only silence detected — speak closer to the microphone".into());
    }

    let resampled = resample_to_16k(trimmed, capture_rate);
    eprintln!("[yap] resampled: {} samples @ 16kHz", resampled.len());

    if backend == "local" {
        return transcribe_local(
            &app,
            &whisper,
            &resampled,
            language.as_deref(),
            model_id.as_deref(),
        )
        .await;
    }

    // Groq path
    let wav_path = write_wav(&resampled)?;
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

#[tauri::command]
pub fn paste_text() -> Result<(), String> {
    paste_impl()
}

#[tauri::command]
pub fn get_hotkey(app: AppHandle) -> String {
    config_read(&app, "hotkey.txt").unwrap_or_else(|| "Ctrl+Shift+Space".to_string())
}

#[tauri::command]
pub fn save_hotkey(app: AppHandle, hotkey: String) -> Result<(), String> {
    config_write(&app, "hotkey.txt", &hotkey)
}

#[tauri::command]
pub fn get_hotkey_mode(app: AppHandle) -> String {
    let v = config_read(&app, "hotkey_mode.txt").unwrap_or_else(|| "toggle".to_string());
    if v == "hold" { "hold".into() } else { "toggle".into() }
}

#[tauri::command]
pub fn save_hotkey_mode(app: AppHandle, mode: String) -> Result<(), String> {
    let normalized = if mode == "hold" { "hold" } else { "toggle" };
    config_write(&app, "hotkey_mode.txt", normalized)
}

#[tauri::command]
pub fn get_language(app: AppHandle) -> String {
    config_read(&app, "language.txt").unwrap_or_default()
}

#[tauri::command]
pub fn save_language(app: AppHandle, language: String) -> Result<(), String> {
    config_write(&app, "language.txt", &language)
}

#[tauri::command]
pub fn get_backend(app: AppHandle) -> String {
    config_read(&app, "backend.txt").unwrap_or_else(|| "groq".to_string())
}

#[tauri::command]
pub fn save_backend(app: AppHandle, backend: String) -> Result<(), String> {
    config_write(&app, "backend.txt", &backend)
}

#[tauri::command]
pub fn get_active_model(app: AppHandle) -> String {
    config_read(&app, "active_model.txt")
        .unwrap_or_else(|| "ggml-large-v3-turbo-q5_0".to_string())
}

#[tauri::command]
pub fn save_active_model(app: AppHandle, model_id: String) -> Result<(), String> {
    config_write(&app, "active_model.txt", &model_id)
}

#[tauri::command]
pub fn list_models(app: AppHandle) -> Vec<ModelStatus> {
    let dir = model_dir(&app);
    MODELS
        .iter()
        .map(|m| {
            let downloaded = dir
                .as_ref()
                .map(|d| d.join(m.filename).exists())
                .unwrap_or(false);
            ModelStatus {
                id: m.id,
                label: m.label,
                filename: m.filename,
                size_bytes: m.size_bytes,
                recommended: m.recommended,
                downloaded,
            }
        })
        .collect()
}

#[tauri::command]
pub async fn download_model(
    app: AppHandle,
    http: State<'_, HttpClient>,
    whisper: State<'_, LocalWhisperState>,
    model_id: String,
) -> Result<(), String> {
    if whisper.downloading.load(Ordering::SeqCst) {
        return Err("A download is already in progress".into());
    }

    let model = MODELS
        .iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| format!("Unknown model: {model_id}"))?;

    let dir = model_dir(&app).ok_or("Could not resolve app data dir")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let final_path = dir.join(model.filename);
    let tmp_path = dir.join(format!("{}.tmp", model.filename));

    whisper.cancel.store(false, Ordering::SeqCst);
    whisper.downloading.store(true, Ordering::SeqCst);

    let url = format!("{HF_BASE}/{}", model.filename);
    let cancel = Arc::clone(&whisper.cancel);
    let downloading = Arc::clone(&whisper.downloading);
    let mid = model_id.clone();

    let result: Result<(), String> = async {
        let res = http
            .0
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            return Err(format!("Download failed: HTTP {}", res.status()));
        }

        let total = res.content_length().unwrap_or(0);
        let mut downloaded_bytes = 0u64;

        let mut file = tokio::fs::File::create(&tmp_path)
            .await
            .map_err(|e| e.to_string())?;

        let mut stream = res;
        use tokio::io::AsyncWriteExt;

        while let Some(chunk) = stream.chunk().await.map_err(|e| e.to_string())? {
            if cancel.load(Ordering::SeqCst) {
                drop(file);
                let _ = tokio::fs::remove_file(&tmp_path).await;
                return Err("Cancelled".into());
            }
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
            downloaded_bytes += chunk.len() as u64;
            let _ = app.emit(
                "download_progress",
                DownloadProgress {
                    model_id: mid.clone(),
                    downloaded: downloaded_bytes,
                    total,
                },
            );
        }

        file.flush().await.map_err(|e| e.to_string())?;
        tokio::fs::rename(&tmp_path, &final_path)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    .await;

    downloading.store(false, Ordering::SeqCst);

    match &result {
        Ok(_) => {
            let _ = app.emit("download_done", DownloadDone { model_id });
        }
        Err(e) => {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            let _ = app.emit(
                "download_error",
                DownloadError {
                    model_id,
                    error: e.clone(),
                },
            );
        }
    }

    result
}

#[tauri::command]
pub fn cancel_download(whisper: State<'_, LocalWhisperState>) {
    whisper.cancel.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub fn delete_model(
    app: AppHandle,
    whisper: State<'_, LocalWhisperState>,
    model_id: String,
) -> Result<(), String> {
    let model = MODELS
        .iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| format!("Unknown model: {model_id}"))?;

    let dir = model_dir(&app).ok_or("Could not resolve app data dir")?;
    let path = dir.join(model.filename);

    if path.exists() {
        // Evict cached context if it belongs to this model.
        let mut cache = whisper.cached.lock().unwrap();
        if let Some((ref p, _)) = *cache {
            if p == path.to_str().unwrap_or("") {
                *cache = None;
            }
        }
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// --- Audio helpers -----------------------------------------------------------

const NOISE_GATE_THRESHOLD: f32 = 0.005;
const VAD_THRESHOLD: f32 = 0.01;
const VAD_PADDING_MS: usize = 150;

fn preferred_config(device: &cpal::Device) -> Result<(cpal::StreamConfig, u32), String> {
    const TARGET: u32 = 16_000;
    let config_16k = cpal::StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(TARGET),
        buffer_size: cpal::BufferSize::Default,
    };
    let dummy: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let dummy_c = Arc::clone(&dummy);
    match device.build_input_stream(
        &config_16k,
        move |data: &[f32], _| {
            dummy_c.lock().unwrap().extend_from_slice(data);
        },
        |_| {},
        None,
    ) {
        Ok(_) => return Ok((config_16k, TARGET)),
        Err(e) => eprintln!("[yap] 16 kHz not supported ({e}), falling back"),
    }

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

// --- Groq transcription ------------------------------------------------------

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

// --- Local Whisper transcription ---------------------------------------------

async fn transcribe_local(
    app: &AppHandle,
    whisper: &LocalWhisperState,
    samples: &[f32],
    language: Option<&str>,
    model_id: Option<&str>,
) -> Result<String, String> {
    let model_id = model_id.unwrap_or("ggml-large-v3-turbo-q5_0");

    let model = MODELS
        .iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| format!("Unknown model: {model_id}"))?;

    let dir = model_dir(app).ok_or("Could not resolve model directory")?;
    let model_path = dir.join(model.filename);

    if !model_path.exists() {
        return Err(format!(
            "Model '{}' is not downloaded. Open Settings → Local Whisper to download it.",
            model.label
        ));
    }

    let model_path_str = model_path.to_string_lossy().to_string();

    // Load or reuse cached context (loading is expensive for large models).
    let ctx = {
        let mut cache = whisper.cached.lock().unwrap();
        let needs_reload = cache
            .as_ref()
            .map(|(p, _)| p.as_str() != model_path_str)
            .unwrap_or(true);

        if needs_reload {
            eprintln!("[yap] loading whisper model: {model_path_str}");
            let new_ctx =
                WhisperContext::new_with_params(&model_path_str, WhisperContextParameters::default())
                    .map_err(|e| format!("Failed to load model: {e}"))?;
            *cache = Some((model_path_str.clone(), Arc::new(new_ctx)));
        }
        Arc::clone(&cache.as_ref().unwrap().1)
    };

    let samples_owned = samples.to_vec();
    let lang_owned = language
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string());

    tokio::task::spawn_blocking(move || {
        let mut wstate = ctx.create_state().map_err(|e| e.to_string())?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        let lang_ref = lang_owned.as_deref();
        params.set_language(lang_ref);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_single_segment(false);

        wstate
            .full(params, &samples_owned)
            .map_err(|e| e.to_string())?;

        let n = wstate.full_n_segments().map_err(|e| e.to_string())?;
        let mut text = String::new();
        for i in 0..n {
            text.push_str(
                &wstate
                    .full_get_segment_text(i)
                    .map_err(|e| e.to_string())?,
            );
        }
        Ok::<String, String>(text.trim().to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// --- Paste implementation ----------------------------------------------------

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
    let modifier = Key::Meta;
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

// --- Config / path helpers ---------------------------------------------------

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

fn model_dir(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|d| d.join("models"))
}
