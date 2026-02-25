// CLI binary — panicking on unrecoverable errors is standard for CLI tools.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::unreachable, clippy::indexing_slicing)]

use std::path::PathBuf;
use std::process;
use std::sync::atomic::AtomicU16;
use std::sync::Arc;

use parking_lot::Mutex;

use std::collections::HashMap;

use clap::{Parser, Subcommand};
use serde_json::Value;

use vibe_lights::dispatcher::CommandDispatcher;
use vibe_lights::model::Show;
use vibe_lights::registry::{self, Command, CommandOutput};
use vibe_lights::settings;
use vibe_lights::state::{AppState, CancellationRegistry, PlaybackState};

// ── CLI argument parsing ─────────────────────────────────────────

#[derive(Parser)]
#[command(name = "vibelights-cli", about = "VibeLights headless CLI", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Data directory override
    #[arg(long, global = true)]
    data_dir: Option<String>,

    /// Open this setup on startup
    #[arg(long, global = true)]
    setup: Option<String>,

    /// Open this sequence on startup (requires --setup)
    #[arg(long, global = true)]
    sequence: Option<String>,

    /// Output raw JSON instead of formatted text
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Setup management
    Setups {
        #[command(subcommand)]
        action: SetupAction,
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
    /// Human-readable description of the show and sequence
    Describe,
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
    /// Send a chat message (agent mode)
    Chat { message: String },
    /// Get agent chat history
    ChatHistory,
    /// Clear agent chat and start new conversation
    ChatClear,
    /// Get current settings
    Settings,
    /// Benchmark frame evaluation (direct, no HTTP)
    Bench {
        /// Data directory
        #[arg(long)]
        data_dir: String,
        /// Setup slug
        #[arg(long)]
        setup: String,
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
enum SetupAction {
    /// List all setups
    List,
    /// Create a new setup
    Create { name: String },
    /// Open/load a setup
    Open { slug: String },
    /// Delete a setup
    Delete { slug: String },
    /// Save the current setup
    Save,
}

#[derive(Subcommand)]
enum SequenceAction {
    /// List sequences in the current setup
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

// ── State initialization ─────────────────────────────────────────

fn dirs_config_dir() -> PathBuf {
    // Match Tauri's app config dir pattern: <config_dir>/com.vibelights.app
    let base = if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .map_or_else(|_| PathBuf::from("C:\\Users\\Default\\AppData\\Roaming"), PathBuf::from)
    } else if cfg!(target_os = "macos") {
        dirs_home().join("Library/Application Support")
    } else {
        std::env::var("XDG_CONFIG_HOME")
            .map_or_else(|_| dirs_home().join(".config"), PathBuf::from)
    };
    base.join(vibe_lights::paths::APP_ID)
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_or_else(|_| PathBuf::from("."), PathBuf::from)
}

fn initialize_state(
    data_dir_override: Option<&str>,
    setup_slug: Option<&str>,
    sequence_slug: Option<&str>,
) -> Arc<AppState> {
    let app_config_dir = dirs_config_dir();
    let mut loaded_settings = settings::load_settings(&app_config_dir);

    // Override data_dir if provided
    if let Some(dd) = data_dir_override {
        let data_path = PathBuf::from(dd);
        std::fs::create_dir_all(data_path.join("setups")).ok();
        let mut s = loaded_settings
            .clone()
            .unwrap_or_else(|| settings::AppSettings::new(data_path.clone()));
        s.data_dir = data_path;
        loaded_settings = Some(s);
    }

    // Load global libraries
    let global_libs = loaded_settings
        .as_ref()
        .and_then(|s| vibe_lights::setup::load_global_libraries(&s.data_dir).ok())
        .unwrap_or_default();

    let state = Arc::new(AppState {
        show: Mutex::new(Show::empty()),
        playback: Mutex::new(PlaybackState {
            playing: false,
            current_time: 0.0,
            sequence_index: 0,
            last_tick: None,
            region: None,
            looping: false,
        }),
        dispatcher: Mutex::new(CommandDispatcher::new()),
        app_config_dir,
        settings: Mutex::new(loaded_settings.clone()),
        current_setup: Mutex::new(None),
        current_sequence: Mutex::new(None),
        script_cache: Mutex::new(HashMap::new()),
        python_sidecar: Mutex::new(None),
        python_port: AtomicU16::new(0),
        analysis_cache: Mutex::new(indexmap::IndexMap::new()),
        agent_sidecar: Mutex::new(None),
        agent_port: AtomicU16::new(0),
        agent_session_id: Mutex::new(None),
        agent_display_messages: Mutex::new(Vec::new()),
        agent_chats: Mutex::new(vibe_lights::chat::AgentChatsData::default()),
        cancellation: CancellationRegistry::new(),
        global_libraries: Mutex::new(global_libs),
        api_port: AtomicU16::new(0),
    });

    // Load agent chat history
    vibe_lights::chat::load_agent_chats(&state);

    // Open setup if specified
    if let Some(setup_s) = setup_slug {
        if let Some(ref settings) = loaded_settings {
            match vibe_lights::setup::load_setup(&settings.data_dir, setup_s) {
                Ok(setup_data) => {
                    *state.current_setup.lock() = Some(setup_s.to_string());
                    eprintln!("[VibeLights] Setup: {} ({setup_s})", setup_data.name);

                    // Open sequence if specified
                    if let Some(seq_s) = sequence_slug {
                        match vibe_lights::setup::load_sequence(&settings.data_dir, setup_s, seq_s) {
                            Ok(seq_data) => {
                                let assembled =
                                    vibe_lights::setup::assemble_show(&setup_data, &seq_data);
                                *state.show.lock() = assembled;
                                *state.current_sequence.lock() = Some(seq_s.to_string());
                                eprintln!("[VibeLights] Sequence: {} ({seq_s})", seq_data.name);
                            }
                            Err(e) => {
                                eprintln!("[VibeLights] Failed to load sequence '{seq_s}': {e}");
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[VibeLights] Failed to load setup '{setup_s}': {e}");
                }
            }
        }
    }

    state
}

// ── Command building ─────────────────────────────────────────────

fn build_command(cmd: &Commands) -> Command {
    match cmd {
        Commands::Play => Command::Play,
        Commands::Pause => Command::Pause,
        Commands::Seek { time } => {
            let p: vibe_lights::registry::params::SeekParams =
                serde_json::from_value(serde_json::json!({ "time": time })).unwrap();
            Command::Seek(p)
        }
        Commands::Undo => Command::Undo,
        Commands::Redo => Command::Redo,
        Commands::Save => Command::SaveCurrentSequence,
        Commands::Show => Command::GetShow,
        Commands::Describe => Command::DescribeShow,
        Commands::Effects => Command::ListEffects,
        Commands::UndoState => Command::GetUndoState,
        Commands::Settings => Command::GetSettings,
        Commands::ChatHistory => Command::GetAgentChatHistory,
        Commands::Frame { time } => {
            let p: vibe_lights::registry::params::GetFrameParams =
                serde_json::from_value(serde_json::json!({ "time": time })).unwrap();
            Command::GetFrame(p)
        }
        Commands::EffectDetail { seq, track, idx } => {
            let p: vibe_lights::registry::params::GetEffectDetailParams =
                serde_json::from_value(serde_json::json!({
                    "sequence_index": seq,
                    "track_index": track,
                    "effect_index": idx,
                }))
                .unwrap();
            Command::GetEffectDetail(p)
        }
        Commands::AddEffect { track, kind, start, end } => {
            let p: vibe_lights::registry::params::AddEffectParams =
                serde_json::from_value(serde_json::json!({
                    "track_index": track,
                    "kind": kind,
                    "start": start,
                    "end": end,
                }))
                .unwrap();
            Command::AddEffect(p)
        }
        Commands::AddTrack { name, fixture } => {
            let p: vibe_lights::registry::params::AddTrackParams =
                serde_json::from_value(serde_json::json!({
                    "name": name,
                    "fixture_id": fixture,
                }))
                .unwrap();
            Command::AddTrack(p)
        }
        Commands::DeleteEffects { targets } => {
            let parsed: Vec<Value> = targets
                .split(',')
                .filter_map(|pair| {
                    let parts: Vec<&str> = pair.trim().split(':').collect();
                    if parts.len() == 2 {
                        let t: usize = parts[0].parse().ok()?;
                        let e: usize = parts[1].parse().ok()?;
                        Some(serde_json::json!({"track_index": t, "effect_index": e}))
                    } else {
                        None
                    }
                })
                .collect();
            let p: vibe_lights::registry::params::DeleteEffectsParams =
                serde_json::from_value(serde_json::json!({ "targets": parsed })).unwrap();
            Command::DeleteEffects(p)
        }
        Commands::UpdateParam { track, effect, key, value } => {
            let parsed_value: Value = serde_json::from_str(value)
                .unwrap_or(Value::String(value.clone()));
            let p: vibe_lights::registry::params::UpdateEffectParamParams =
                serde_json::from_value(serde_json::json!({
                    "track_index": track,
                    "effect_index": effect,
                    "key": key,
                    "value": parsed_value,
                }))
                .unwrap();
            Command::UpdateEffectParam(p)
        }
        Commands::UpdateTime { track, effect, start, end } => {
            let p: vibe_lights::registry::params::UpdateEffectTimeRangeParams =
                serde_json::from_value(serde_json::json!({
                    "track_index": track,
                    "effect_index": effect,
                    "start": start,
                    "end": end,
                }))
                .unwrap();
            Command::UpdateEffectTimeRange(p)
        }
        Commands::MoveEffect { from_track, effect, to_track } => {
            let p: vibe_lights::registry::params::MoveEffectToTrackParams =
                serde_json::from_value(serde_json::json!({
                    "from_track": from_track,
                    "effect_index": effect,
                    "to_track": to_track,
                }))
                .unwrap();
            Command::MoveEffectToTrack(p)
        }
        Commands::Setups { action } => match action {
            SetupAction::List => Command::ListSetups,
            SetupAction::Create { name } => {
                let p: vibe_lights::registry::params::CreateSetupParams =
                    serde_json::from_value(serde_json::json!({ "name": name })).unwrap();
                Command::CreateSetup(p)
            }
            SetupAction::Open { slug } => {
                let p: vibe_lights::registry::params::SlugParams =
                    serde_json::from_value(serde_json::json!({ "slug": slug })).unwrap();
                Command::OpenSetup(p)
            }
            SetupAction::Delete { slug } => {
                let p: vibe_lights::registry::params::SlugParams =
                    serde_json::from_value(serde_json::json!({ "slug": slug })).unwrap();
                Command::DeleteSetup(p)
            }
            SetupAction::Save => Command::SaveSetup,
        },
        Commands::Sequences { action } => match action {
            SequenceAction::List => Command::ListSequences,
            SequenceAction::Create { name } => {
                let p: vibe_lights::registry::params::CreateSequenceParams =
                    serde_json::from_value(serde_json::json!({ "name": name })).unwrap();
                Command::CreateSequence(p)
            }
            SequenceAction::Open { slug } => {
                let p: vibe_lights::registry::params::SlugParams =
                    serde_json::from_value(serde_json::json!({ "slug": slug })).unwrap();
                Command::OpenSequence(p)
            }
            SequenceAction::Delete { slug } => {
                let p: vibe_lights::registry::params::SlugParams =
                    serde_json::from_value(serde_json::json!({ "slug": slug })).unwrap();
                Command::DeleteSequence(p)
            }
            SequenceAction::Save => Command::SaveCurrentSequence,
        },
        Commands::Chat { .. } => {
            eprintln!("Error: Agent chat requires the VibeLights GUI.");
            eprintln!("Use `pnpm tauri dev` to run the full application.");
            process::exit(1);
        }
        Commands::ChatClear => Command::NewAgentConversation,
        // Bench is handled separately before this function is called
        Commands::Bench { .. } => unreachable!(),
    }
}

// ── Output formatting ────────────────────────────────────────────

fn print_output(output: &CommandOutput, raw_json: bool) {
    if raw_json {
        let json = serde_json::json!({
            "message": output.message,
            "result": output.result,
        });
        println!("{}", serde_json::to_string_pretty(&json).unwrap_or_default());
        return;
    }

    println!("{}", output.message);

    // For data commands, also print the data
    let result_json = serde_json::to_value(&output.result).unwrap_or(Value::Null);
    if let Some(data) = result_json.get("data") {
        if !data.is_null() {
            if data.is_string() {
                println!("{}", data.as_str().unwrap());
            } else if data.is_array() || data.is_object() {
                println!("{}", serde_json::to_string_pretty(data).unwrap_or_default());
            } else {
                println!("{data}");
            }
        }
    }
}

// ── Bench mode ──────────────────────────────────────────────────

#[allow(clippy::cast_precision_loss)]
fn run_bench(data_dir: &str, setup_slug: &str, sequence_slug: &str, time: f64, iterations: usize) {
    use std::time::Instant;
    use vibe_lights::engine;
    use vibe_lights::model::fixture::EffectTarget;
    use vibe_lights::setup;

    let data_path = PathBuf::from(data_dir);

    // Load setup
    let setup_data = setup::load_setup(&data_path, setup_slug).unwrap_or_else(|e| {
        eprintln!("Failed to load setup '{setup_slug}': {e}");
        process::exit(1);
    });

    // Load sequence
    let seq = setup::load_sequence(&data_path, setup_slug, sequence_slug).unwrap_or_else(|e| {
        eprintln!("Failed to load sequence '{sequence_slug}': {e}");
        process::exit(1);
    });

    // Assemble show
    let show = setup::assemble_show(&setup_data, &seq);

    eprintln!("Show: {} fixtures, {} groups", show.fixtures.len(), show.groups.len());
    let total_pixels: u32 = show.fixtures.iter().map(|f| f.pixel_count).sum();
    eprintln!("Total pixels: {total_pixels}");
    if let Some(s) = show.sequences.first() {
        eprintln!("Sequence: {} tracks, duration {:.1}s", s.tracks.len(), s.duration);
        let total_effects: usize = s.tracks.iter().map(|t| t.effects.len()).sum();
        eprintln!("Total effects: {total_effects}");

        // Count active effects at benchmark time
        let active_effects: usize = s.tracks.iter().map(|t| {
            t.effects.iter().filter(|e| e.time_range.contains(time)).count()
        }).sum();
        eprintln!("Active effects at t={time}: {active_effects}");

        // Count target stats
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
        eprintln!("Track targets: {all_count} All, {group_count} Group, {fixture_count} Fixtures");
    }

    // Measure serialization overhead
    {
        let frame = engine::evaluate(&show, 0, time, None, None, &HashMap::new(), &HashMap::new());
        let json = serde_json::to_string(&frame).unwrap();
        eprintln!("Frame JSON size: {} bytes ({:.1} KB)", json.len(), json.len() as f64 / 1024.0);
        eprintln!("Frame fixture count: {}", frame.fixtures.len());

        eprintln!("Non-black fixtures in frame: {}", frame.fixtures.len());

        let start = std::time::Instant::now();
        for _ in 0..20 {
            let f = engine::evaluate(&show, 0, time, None, None, &HashMap::new(), &HashMap::new());
            let j = serde_json::to_string(&f).unwrap();
            std::hint::black_box(&j);
        }
        let ser_time = start.elapsed() / 20;
        eprintln!("Eval + serialize: {ser_time:?}");

        // Measure serialize alone
        let frame2 = engine::evaluate(&show, 0, time, None, None, &HashMap::new(), &HashMap::new());
        let start2 = std::time::Instant::now();
        for _ in 0..20 {
            let j = serde_json::to_string(&frame2).unwrap();
            std::hint::black_box(&j);
        }
        let ser_only = start2.elapsed() / 20;
        eprintln!("Serialize only: {ser_only:?}");
    }

    // Warmup
    eprintln!("\nWarmup...");
    let _ = engine::evaluate(&show, 0, time, None, None, &HashMap::new(), &HashMap::new());

    // Benchmark (evaluate only)
    eprintln!("Benchmarking {iterations} iterations at t={time}...\n");
    let mut times = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        let frame = engine::evaluate(&show, 0, time, None, None, &HashMap::new(), &HashMap::new());
        let elapsed = start.elapsed();
        times.push(elapsed);
        std::hint::black_box(&frame);
    }

    times.sort();
    let total: std::time::Duration = times.iter().sum();
    #[allow(clippy::cast_possible_truncation)]
    let avg = total / iterations as u32;
    let median = times[iterations / 2];
    let min = times[0];
    let max = times[iterations - 1];
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let p95 = times[(iterations as f64 * 0.95) as usize];

    eprintln!("Results ({iterations} iterations):");
    eprintln!("  avg:    {avg:>8.2?}");
    eprintln!("  median: {median:>8.2?}");
    eprintln!("  min:    {min:>8.2?}");
    eprintln!("  max:    {max:>8.2?}");
    eprintln!("  p95:    {p95:>8.2?}");
    eprintln!("  fps:    {:.1}", 1.0 / avg.as_secs_f64());
}

// ── Main ─────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Handle bench separately (doesn't need full state)
    if let Commands::Bench {
        data_dir,
        setup,
        sequence,
        time,
        iterations,
    } = &cli.command
    {
        run_bench(data_dir, setup, sequence, *time, *iterations);
        return;
    }

    // Initialize state from disk
    let state = initialize_state(
        cli.data_dir.as_deref(),
        cli.setup.as_deref(),
        cli.sequence.as_deref(),
    );

    // Build the command
    let cmd = build_command(&cli.command);
    let raw = cli.json;

    // CLI only supports sync commands. Async commands (agent chat, audio analysis)
    // require the Tauri GUI runtime.
    if cmd.is_async() {
        eprintln!("Error: This command requires the VibeLights GUI (async dispatch).");
        eprintln!("Use `pnpm tauri dev` to run the full application.");
        process::exit(1);
    }

    match registry::execute::execute(&state, cmd) {
        Ok(output) => print_output(&output, raw),
        Err(e) => {
            eprintln!("Error: {e}");
            process::exit(1);
        }
    }
}
