// CLI binary — panicking on unrecoverable errors is standard for CLI tools.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::unreachable)]

use std::path::PathBuf;
use std::process;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;

use clap::{Parser, Subcommand};
use serde_json::Value;

// ── CLI argument parsing ─────────────────────────────────────────

#[derive(Parser)]
#[command(name = "vibelights-cli", about = "VibeLights headless CLI", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Connect to a specific port (default: auto-discover from .vibelights-port)
    #[arg(long, global = true)]
    port: Option<u16>,

    /// Output raw JSON instead of formatted text
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the VibeLights API server (long-running)
    Serve {
        /// Data directory (overrides settings.json)
        #[arg(long)]
        data_dir: Option<String>,
        /// Open this profile on startup
        #[arg(long)]
        profile: Option<String>,
        /// Open this sequence on startup (requires --profile)
        #[arg(long)]
        sequence: Option<String>,
        /// Bind to specific port (default: OS-assigned)
        #[arg(long, default_value = "0")]
        port: u16,
        /// Set Claude API key
        #[arg(long)]
        api_key: Option<String>,
    },
    /// Profile management
    Profiles {
        #[command(subcommand)]
        action: ProfileAction,
    },
    /// Sequence management
    Sequences {
        #[command(subcommand)]
        action: SequenceAction,
    },
    /// Add an effect to a track
    AddEffect {
        #[arg(long)]
        track: usize,
        #[arg(long)]
        kind: String,
        #[arg(long)]
        start: f64,
        #[arg(long)]
        end: f64,
    },
    /// Add a new track
    AddTrack {
        #[arg(long)]
        name: String,
        #[arg(long)]
        fixture: u32,
    },
    /// Delete effects by track:effect pairs (e.g. "0:2,0:3")
    DeleteEffects {
        #[arg(long)]
        targets: String,
    },
    /// Update an effect parameter
    UpdateParam {
        #[arg(long)]
        track: usize,
        #[arg(long)]
        effect: usize,
        #[arg(long)]
        key: String,
        #[arg(long)]
        value: String,
    },
    /// Update an effect's time range
    UpdateTime {
        #[arg(long)]
        track: usize,
        #[arg(long)]
        effect: usize,
        #[arg(long)]
        start: f64,
        #[arg(long)]
        end: f64,
    },
    /// Move an effect to another track
    MoveEffect {
        #[arg(long)]
        from_track: usize,
        #[arg(long)]
        effect: usize,
        #[arg(long)]
        to_track: usize,
    },
    /// Start playback
    Play,
    /// Pause playback
    Pause,
    /// Seek to a time in seconds
    Seek { time: f64 },
    /// Get the full show JSON
    Show,
    /// Human-readable description of the show
    Describe {
        /// Include frame state at this time
        #[arg(long)]
        frame_time: Option<f64>,
    },
    /// Get frame data at a specific time
    Frame { time: f64 },
    /// List available effect types with parameter schemas
    Effects,
    /// Get detail for a specific effect
    EffectDetail {
        seq: usize,
        track: usize,
        idx: usize,
    },
    /// Undo the last action
    Undo,
    /// Redo the last undone action
    Redo,
    /// Get undo/redo state
    UndoState,
    /// Save the current sequence
    Save,
    /// Send a chat message
    Chat { message: String },
    /// Get chat history
    ChatHistory,
    /// Clear chat history
    ChatClear,
    /// Scan a Vixen 3 directory
    VixenScan { vixen_dir: String },
    /// Execute a Vixen import (pass config JSON)
    VixenImport { config_json: String },
    /// Benchmark frame evaluation (direct, no HTTP)
    Bench {
        /// Data directory
        #[arg(long)]
        data_dir: String,
        /// Profile slug
        #[arg(long)]
        profile: String,
        /// Sequence slug
        #[arg(long)]
        sequence: String,
        /// Time to evaluate at
        #[arg(long, default_value = "10.0")]
        time: f64,
        /// Number of iterations
        #[arg(long, default_value = "20")]
        iterations: usize,
    },
}

#[derive(Subcommand)]
enum ProfileAction {
    /// List all profiles
    List,
    /// Create a new profile
    Create { name: String },
    /// Open/load a profile
    Open { slug: String },
    /// Delete a profile
    Delete { slug: String },
    /// Save the current profile
    Save,
}

#[derive(Subcommand)]
enum SequenceAction {
    /// List sequences in the current profile
    List,
    /// Create a new sequence
    Create { name: String },
    /// Open/load a sequence
    Open { slug: String },
    /// Delete a sequence
    Delete { slug: String },
    /// Save the current sequence
    Save,
}

// ── Server mode ──────────────────────────────────────────────────

async fn run_serve(
    data_dir: Option<String>,
    profile: Option<String>,
    sequence: Option<String>,
    port: u16,
    api_key: Option<String>,
) {
    use vibe_lights::api;
    use vibe_lights::chat::ChatManager;
    use vibe_lights::dispatcher::CommandDispatcher;
    use vibe_lights::model::Show;
    use vibe_lights::settings;
    use vibe_lights::state::{AppState, PlaybackState};

    // Determine app config dir
    let app_config_dir = dirs_config_dir();

    // Load or create settings
    let mut loaded_settings = settings::load_settings(&app_config_dir);

    // Override data_dir if provided
    if let Some(ref dd) = data_dir {
        let data_path = PathBuf::from(dd);
        std::fs::create_dir_all(data_path.join("profiles")).ok();
        let mut s = loaded_settings
            .clone()
            .unwrap_or_else(|| settings::AppSettings::new(data_path.clone()));
        s.data_dir = data_path;
        settings::save_settings(&app_config_dir, &s).ok();
        loaded_settings = Some(s);
    }

    // Override API key if provided
    if let Some(ref key) = api_key {
        if let Some(ref mut s) = loaded_settings {
            s.claude_api_key = Some(key.clone());
            settings::save_settings(&app_config_dir, s).ok();
        }
    }

    let state = Arc::new(AppState {
        show: Mutex::new(Show::empty()),
        playback: Mutex::new(PlaybackState {
            playing: false,
            current_time: 0.0,
            sequence_index: 0,
            last_tick: None,
        }),
        dispatcher: Mutex::new(CommandDispatcher::new()),
        chat: Mutex::new(ChatManager::new()),
        api_port: AtomicU16::new(0),
        app_config_dir: app_config_dir.clone(),
        settings: Mutex::new(loaded_settings.clone()),
        current_profile: Mutex::new(None),
        current_sequence: Mutex::new(None),
    });

    // Open profile and sequence if specified
    if let Some(ref profile_slug) = profile {
        if let Some(ref settings) = loaded_settings {
            match vibe_lights::profile::load_profile(&settings.data_dir, profile_slug) {
                Ok(profile_data) => {
                    *state.current_profile.lock() = Some(profile_slug.clone());
                    eprintln!(
                        "[VibeLights] Profile: {} ({})",
                        profile_data.name, profile_slug
                    );

                    if let Some(ref seq_slug) = sequence {
                        match vibe_lights::profile::load_sequence(
                            &settings.data_dir,
                            profile_slug,
                            seq_slug,
                        ) {
                            Ok(seq_data) => {
                                let assembled =
                                    vibe_lights::profile::assemble_show(&profile_data, &seq_data);
                                *state.show.lock() = assembled;
                                *state.current_sequence.lock() =
                                    Some(seq_slug.clone());
                                eprintln!(
                                    "[VibeLights] Sequence: {} ({})",
                                    seq_data.name, seq_slug
                                );
                            }
                            Err(e) => {
                                eprintln!("[VibeLights] Failed to load sequence '{}': {}", seq_slug, e);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[VibeLights] Failed to load profile '{}': {}",
                        profile_slug, e
                    );
                }
            }
        }
    }

    // Determine port
    let bind_port = if port == 0 {
        // Use OS-assigned port — start server, then print
        let actual_port = api::start_api_server(state.clone()).await;
        state.api_port.store(actual_port, Ordering::Relaxed);
        actual_port
    } else {
        state.api_port.store(port, Ordering::Relaxed);
        port
    };

    // Write port file
    let port_file = app_config_dir.join(".vibelights-port");
    let _ = std::fs::write(&port_file, bind_port.to_string());

    eprintln!("[VibeLights] API server: http://127.0.0.1:{}", bind_port);
    eprintln!("[VibeLights] Ready. Press Ctrl+C to stop.");

    if port != 0 {
        // If a specific port was requested, we need to start the server
        // on that port and block until it exits.
        api::start_api_server_on_port(state, port).await;
    } else {
        // Server is already running via start_api_server (spawned).
        // Block until Ctrl+C.
        tokio::signal::ctrl_c().await.ok();
        eprintln!("\n[VibeLights] Shutting down.");
    }

    // Clean up port file
    let _ = std::fs::remove_file(&port_file);
}

// ── Bench mode ──────────────────────────────────────────────────

fn run_bench(data_dir: &str, profile_slug: &str, sequence_slug: &str, time: f64, iterations: usize) {
    use std::time::Instant;
    use vibe_lights::engine;
    use vibe_lights::profile;

    let data_path = PathBuf::from(data_dir);

    // Load profile
    let prof = profile::load_profile(&data_path, profile_slug).unwrap_or_else(|e| {
        eprintln!("Failed to load profile '{}': {}", profile_slug, e);
        process::exit(1);
    });

    // Load sequence
    let seq = profile::load_sequence(&data_path, profile_slug, sequence_slug).unwrap_or_else(|e| {
        eprintln!("Failed to load sequence '{}': {}", sequence_slug, e);
        process::exit(1);
    });

    // Assemble show
    let show = profile::assemble_show(&prof, &seq);

    eprintln!("Show: {} fixtures, {} groups", show.fixtures.len(), show.groups.len());
    let total_pixels: u32 = show.fixtures.iter().map(|f| f.pixel_count).sum();
    eprintln!("Total pixels: {}", total_pixels);
    if let Some(s) = show.sequences.first() {
        eprintln!("Sequence: {} tracks, duration {:.1}s", s.tracks.len(), s.duration);
        let total_effects: usize = s.tracks.iter().map(|t| t.effects.len()).sum();
        eprintln!("Total effects: {}", total_effects);

        // Count active effects at benchmark time
        let active_effects: usize = s.tracks.iter().map(|t| {
            t.effects.iter().filter(|e| e.time_range.contains(time)).count()
        }).sum();
        eprintln!("Active effects at t={}: {}", time, active_effects);

        // Count target stats
        use vibe_lights::model::fixture::EffectTarget;
        let mut all_count = 0usize;
        let mut group_count = 0usize;
        let mut fixture_count = 0usize;
        for track in &s.tracks {
            match &track.target {
                EffectTarget::All => all_count += 1,
                EffectTarget::Group(_) => group_count += 1,
                EffectTarget::Fixtures(_) => fixture_count += 1,
            }
        }
        eprintln!("Track targets: {} All, {} Group, {} Fixtures", all_count, group_count, fixture_count);
    }

    // Measure serialization overhead
    {
        let frame = engine::evaluate(&show, 0, time, None);
        let json = serde_json::to_string(&frame).unwrap();
        eprintln!("Frame JSON size: {} bytes ({:.1} KB)", json.len(), json.len() as f64 / 1024.0);
        eprintln!("Frame fixture count: {}", frame.fixtures.len());

        eprintln!("Non-black fixtures in frame: {}", frame.fixtures.len());

        let start = std::time::Instant::now();
        for _ in 0..20 {
            let f = engine::evaluate(&show, 0, time, None);
            let j = serde_json::to_string(&f).unwrap();
            std::hint::black_box(&j);
        }
        let ser_time = start.elapsed() / 20;
        eprintln!("Eval + serialize: {:?}", ser_time);

        // Measure serialize alone
        let frame2 = engine::evaluate(&show, 0, time, None);
        let start2 = std::time::Instant::now();
        for _ in 0..20 {
            let j = serde_json::to_string(&frame2).unwrap();
            std::hint::black_box(&j);
        }
        let ser_only = start2.elapsed() / 20;
        eprintln!("Serialize only: {:?}", ser_only);
    }

    // Warmup
    eprintln!("\nWarmup...");
    let _ = engine::evaluate(&show, 0, time, None);

    // Benchmark (evaluate only)
    eprintln!("Benchmarking {} iterations at t={}...\n", iterations, time);
    let mut times = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        let frame = engine::evaluate(&show, 0, time, None);
        let elapsed = start.elapsed();
        times.push(elapsed);
        std::hint::black_box(&frame);
    }

    times.sort();
    let total: std::time::Duration = times.iter().sum();
    let avg = total / iterations as u32;
    let median = times[iterations / 2];
    let min = times[0];
    let max = times[iterations - 1];
    let p95 = times[(iterations as f64 * 0.95) as usize];

    eprintln!("Results ({} iterations):", iterations);
    eprintln!("  avg:    {:>8.2?}", avg);
    eprintln!("  median: {:>8.2?}", median);
    eprintln!("  min:    {:>8.2?}", min);
    eprintln!("  max:    {:>8.2?}", max);
    eprintln!("  p95:    {:>8.2?}", p95);
    eprintln!("  fps:    {:.1}", 1.0 / avg.as_secs_f64());
}

// ── Client mode helpers ──────────────────────────────────────────

fn discover_port(cli_port: Option<u16>) -> u16 {
    if let Some(p) = cli_port {
        return p;
    }

    // Try .vibelights-port in config dir
    let config_dir = dirs_config_dir();
    let port_file = config_dir.join(".vibelights-port");
    if let Ok(contents) = std::fs::read_to_string(&port_file) {
        if let Ok(p) = contents.trim().parse::<u16>() {
            return p;
        }
    }

    eprintln!("Error: Could not discover VibeLights server port.");
    eprintln!("Either start 'vibelights-cli serve' or use --port <PORT>.");
    process::exit(1);
}

fn base_url(port: u16) -> String {
    format!("http://127.0.0.1:{}", port)
}

fn dirs_config_dir() -> PathBuf {
    // Match Tauri's app config dir pattern: <config_dir>/com.vibelights.app
    let base = if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("C:\\Users\\Default\\AppData\\Roaming"))
    } else if cfg!(target_os = "macos") {
        dirs_home().join("Library/Application Support")
    } else {
        std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| dirs_home().join(".config"))
    };
    base.join("com.vibelights.app")
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

// ── HTTP client helpers ──────────────────────────────────────────

async fn http_get(url: &str) -> Result<Value, String> {
    let resp = reqwest::get(url)
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    resp.json()
        .await
        .map_err(|e| format!("Parse failed: {}", e))
}

async fn http_post(url: &str, body: Value) -> Result<Value, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    resp.json()
        .await
        .map_err(|e| format!("Parse failed: {}", e))
}

async fn http_delete(url: &str) -> Result<Value, String> {
    let client = reqwest::Client::new();
    let resp = client
        .delete(url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    resp.json()
        .await
        .map_err(|e| format!("Parse failed: {}", e))
}

// http_put available for future use (e.g., fixtures/setup endpoints)
#[allow(dead_code)]
async fn http_put(url: &str, body: Value) -> Result<Value, String> {
    let client = reqwest::Client::new();
    let resp = client
        .put(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    resp.json()
        .await
        .map_err(|e| format!("Parse failed: {}", e))
}

fn check_response(json: &Value) -> Result<(), String> {
    if json["ok"].as_bool() != Some(true) {
        let err = json["error"].as_str().unwrap_or("Unknown error");
        return Err(err.to_string());
    }
    Ok(())
}

fn print_result(json: &Value, raw_json: bool) {
    if raw_json {
        println!("{}", serde_json::to_string_pretty(json).unwrap_or_default());
        return;
    }

    if let Err(e) = check_response(json) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }

    if let Some(desc) = json["description"].as_str() {
        println!("{}", desc);
    }

    let data = &json["data"];
    if data.is_null() {
        if json["description"].is_null() {
            println!("OK");
        }
    } else if data.is_string() {
        println!("{}", data.as_str().unwrap());
    } else if data.is_array() || data.is_object() {
        println!("{}", serde_json::to_string_pretty(data).unwrap_or_default());
    } else {
        println!("{}", data);
    }
}

// ── Main ─────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve {
            data_dir,
            profile,
            sequence,
            port,
            api_key,
        } => {
            run_serve(data_dir, profile, sequence, port, api_key).await;
        }
        Commands::Bench {
            data_dir,
            profile,
            sequence,
            time,
            iterations,
        } => {
            run_bench(&data_dir, &profile, &sequence, time, iterations);
        }
        cmd => {
            let port = discover_port(cli.port);
            let base = base_url(port);
            let raw = cli.json;

            let result = match cmd {
                Commands::Serve { .. } | Commands::Bench { .. } => unreachable!(),

                Commands::Profiles { action } => match action {
                    ProfileAction::List => http_get(&format!("{}/api/profiles", base)).await,
                    ProfileAction::Create { name } => {
                        http_post(&format!("{}/api/profiles", base), serde_json::json!({ "name": name })).await
                    }
                    ProfileAction::Open { slug } => {
                        http_get(&format!("{}/api/profiles/{}", base, slug)).await
                    }
                    ProfileAction::Delete { slug } => {
                        http_delete(&format!("{}/api/profiles/{}", base, slug)).await
                    }
                    ProfileAction::Save => {
                        http_post(&format!("{}/api/save", base), serde_json::json!({})).await
                    }
                },

                Commands::Sequences { action } => {
                    // We need the current profile slug for sequence operations.
                    // First try to get it from the server's settings.
                    let settings_json = http_get(&format!("{}/api/settings", base)).await;
                    let current_profile = settings_json
                        .as_ref()
                        .ok()
                        .and_then(|j| j["data"]["last_profile"].as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_default();

                    if current_profile.is_empty() {
                        eprintln!("Error: No profile is currently open. Open a profile first.");
                        process::exit(1);
                    }

                    match action {
                        SequenceAction::List => {
                            http_get(&format!("{}/api/profiles/{}/sequences", base, current_profile)).await
                        }
                        SequenceAction::Create { name } => {
                            http_post(
                                &format!("{}/api/profiles/{}/sequences", base, current_profile),
                                serde_json::json!({ "name": name }),
                            ).await
                        }
                        SequenceAction::Open { slug } => {
                            http_get(&format!(
                                "{}/api/profiles/{}/sequences/{}",
                                base, current_profile, slug
                            )).await
                        }
                        SequenceAction::Delete { slug } => {
                            http_delete(&format!(
                                "{}/api/profiles/{}/sequences/{}",
                                base, current_profile, slug
                            )).await
                        }
                        SequenceAction::Save => {
                            http_post(&format!("{}/api/save", base), serde_json::json!({})).await
                        }
                    }
                }

                Commands::AddEffect { track, kind, start, end } => {
                    http_post(
                        &format!("{}/api/command", base),
                        serde_json::json!({
                            "AddEffect": {
                                "sequence_index": 0,
                                "track_index": track,
                                "kind": kind,
                                "start": start,
                                "end": end
                            }
                        }),
                    ).await
                }

                Commands::AddTrack { name, fixture } => {
                    http_post(
                        &format!("{}/api/command", base),
                        serde_json::json!({
                            "AddTrack": {
                                "sequence_index": 0,
                                "name": name,
                                "target": { "Fixtures": [fixture] }
                            }
                        }),
                    ).await
                }

                Commands::DeleteEffects { targets } => {
                    let parsed: Vec<(usize, usize)> = targets
                        .split(',')
                        .filter_map(|pair| {
                            let parts: Vec<&str> = pair.trim().split(':').collect();
                            if parts.len() == 2 {
                                Some((parts[0].parse().ok()?, parts[1].parse().ok()?))
                            } else {
                                None
                            }
                        })
                        .collect();
                    http_post(
                        &format!("{}/api/command", base),
                        serde_json::json!({
                            "DeleteEffects": {
                                "sequence_index": 0,
                                "targets": parsed
                            }
                        }),
                    ).await
                }

                Commands::UpdateParam { track, effect, key, value } => {
                    let parsed_value: Value = serde_json::from_str(&value)
                        .unwrap_or(Value::String(value));
                    http_post(
                        &format!("{}/api/command", base),
                        serde_json::json!({
                            "UpdateEffectParam": {
                                "sequence_index": 0,
                                "track_index": track,
                                "effect_index": effect,
                                "key": key,
                                "value": parsed_value
                            }
                        }),
                    ).await
                }

                Commands::UpdateTime { track, effect, start, end } => {
                    http_post(
                        &format!("{}/api/command", base),
                        serde_json::json!({
                            "UpdateEffectTimeRange": {
                                "sequence_index": 0,
                                "track_index": track,
                                "effect_index": effect,
                                "start": start,
                                "end": end
                            }
                        }),
                    ).await
                }

                Commands::MoveEffect { from_track, effect, to_track } => {
                    http_post(
                        &format!("{}/api/command", base),
                        serde_json::json!({
                            "MoveEffectToTrack": {
                                "sequence_index": 0,
                                "from_track": from_track,
                                "effect_index": effect,
                                "to_track": to_track
                            }
                        }),
                    ).await
                }

                Commands::Play => http_post(&format!("{}/api/play", base), serde_json::json!({})).await,
                Commands::Pause => http_post(&format!("{}/api/pause", base), serde_json::json!({})).await,
                Commands::Seek { time } => {
                    http_post(&format!("{}/api/seek", base), serde_json::json!({ "time": time })).await
                }

                Commands::Show => http_get(&format!("{}/api/show", base)).await,
                Commands::Describe { frame_time } => {
                    let url = if let Some(t) = frame_time {
                        format!("{}/api/describe?frame_time={}", base, t)
                    } else {
                        format!("{}/api/describe", base)
                    };
                    http_get(&url).await
                }
                Commands::Frame { time } => {
                    http_get(&format!("{}/api/frame?time={}", base, time)).await
                }
                Commands::Effects => http_get(&format!("{}/api/effects", base)).await,
                Commands::EffectDetail { seq, track, idx } => {
                    http_get(&format!("{}/api/effect/{}/{}/{}", base, seq, track, idx)).await
                }

                Commands::Undo => http_post(&format!("{}/api/undo", base), serde_json::json!({})).await,
                Commands::Redo => http_post(&format!("{}/api/redo", base), serde_json::json!({})).await,
                Commands::UndoState => http_get(&format!("{}/api/undo-state", base)).await,
                Commands::Save => http_post(&format!("{}/api/save", base), serde_json::json!({})).await,

                Commands::Chat { message } => {
                    http_post(&format!("{}/api/chat", base), serde_json::json!({ "message": message })).await
                }
                Commands::ChatHistory => http_get(&format!("{}/api/chat/history", base)).await,
                Commands::ChatClear => {
                    http_post(&format!("{}/api/chat/clear", base), serde_json::json!({})).await
                }

                Commands::VixenScan { vixen_dir } => {
                    http_post(
                        &format!("{}/api/vixen/scan", base),
                        serde_json::json!({ "vixen_dir": vixen_dir }),
                    ).await
                }
                Commands::VixenImport { config_json } => {
                    let config: Value = serde_json::from_str(&config_json).unwrap_or_else(|e| {
                        eprintln!("Error: Invalid JSON config: {}", e);
                        process::exit(1);
                    });
                    http_post(&format!("{}/api/vixen/execute", base), config).await
                }
            };

            match result {
                Ok(json) => print_result(&json, raw),
                Err(e) => {
                    eprintln!("Error: {}", e);
                    process::exit(1);
                }
            }
        }
    }
}
