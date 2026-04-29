use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tauri::{AppHandle, Manager, State};

// --- State -------------------------------------------------------------------

pub struct RecordingState {
    is_recording: Arc<Mutex<bool>>,
    samples: Arc<Mutex<Vec<f32>>>,
    stream: Mutex<Option<cpal::Stream>>,
}

impl Default for RecordingState {
    fn default() -> Self {
        Self {
            is_recording: Arc::new(Mutex::new(false)),
            samples: Arc::new(Mutex::new(Vec::new())),
            stream: Mutex::new(None),
        }
    }
}

// cpal::Stream is !Send, but access is serialised by the Mutex.
unsafe impl Send for RecordingState {}
unsafe impl Sync for RecordingState {}

// --- Commands ----------------------------------------------------------------

#[tauri::command]
pub fn start_recording(state: State<'_, RecordingState>) -> Result<(), String> {
    eprintln!("[yap] start_recording called");
    *state.is_recording.lock().unwrap() = true;
    state.samples.lock().unwrap().clear();

    let host = cpal::default_host();
    eprintln!("[yap] default host: {}", host.id().name());

    let device = host
        .default_input_device()
        .ok_or("No input device available")?;
    eprintln!("[yap] input device: {}", device.name().unwrap_or_default());

    let config = cpal::StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(16_000),
        buffer_size: cpal::BufferSize::Default,
    };

    let samples_w = Arc::clone(&state.samples);
    let is_rec_w = Arc::clone(&state.is_recording);

    let stream = device
        .build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if *is_rec_w.lock().unwrap() {
                    samples_w.lock().unwrap().extend_from_slice(data);
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
    api_key: String,
) -> Result<String, String> {
    eprintln!("[yap] stop_recording called");
    *state.is_recording.lock().unwrap() = false;
    *state.stream.lock().unwrap() = None;

    let samples = state.samples.lock().unwrap().clone();
    eprintln!("[yap] captured {} samples ({:.1}s)", samples.len(), samples.len() as f32 / 16000.0);

    if samples.is_empty() {
        return Err("No audio captured — try holding the key a bit longer".into());
    }

    let wav_path = write_wav(&samples)?;
    eprintln!("[yap] WAV written to {:?}", wav_path);
    eprintln!("[yap] sending to Groq (api_key set: {})", !api_key.is_empty());
    transcribe(&wav_path, &api_key).await
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

/// Simulate Ctrl+V in the currently-focused window.
#[tauri::command]
pub fn paste_text() -> Result<(), String> {
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        std::process::Command::new("ydotool")
            .args(["key", "29:1", "47:1", "47:0", "29:0"])
            .status()
            .map_err(|e| format!("ydotool failed: {e} — install ydotool for Wayland"))?;
    } else {
        std::process::Command::new("xdotool")
            .args(["key", "--clearmodifiers", "ctrl+v"])
            .status()
            .map_err(|e| format!("xdotool failed: {e} — install xdotool for X11"))?;
    }
    Ok(())
}

// --- Helpers -----------------------------------------------------------------

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
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer.write_sample(v).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())?;
    Ok(path)
}

async fn transcribe(path: &std::path::Path, api_key: &str) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("No API key — open settings (⚙) and enter your Groq API key".into());
    }

    let bytes = tokio::fs::read(path).await.map_err(|e| e.to_string())?;

    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;

    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", "whisper-large-v3-turbo");

    let client = reqwest::Client::new();
    let res = client
        .post("https://api.groq.com/openai/v1/audio/transcriptions")
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = res.status();
    let json: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;

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

fn key_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("api_key.txt"))
}
