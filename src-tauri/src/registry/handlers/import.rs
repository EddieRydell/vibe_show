#![allow(clippy::needless_pass_by_value, clippy::too_many_lines)]

use std::sync::Arc;

use crate::error::AppError;
use crate::setup::{self, MEDIA_EXTENSIONS};
use crate::registry::params::{
    CheckVixenPreviewFileParams, ImportVixenParams, ImportVixenSetupParams,
    ImportVixenSequenceParams, ScanVixenDirectoryParams,
};
use crate::registry::{CommandOutput, CommandResult};
use crate::state::{get_data_dir, AppState};

pub fn import_vixen(
    state: &Arc<AppState>,
    p: ImportVixenParams,
) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;

    let mut importer = crate::import::vixen::VixenImporter::new();
    importer
        .parse_system_config(std::path::Path::new(&p.system_config_path))
        .map_err(|e| AppError::ImportError {
            message: e.to_string(),
        })?;
    for seq_path in &p.sequence_paths {
        importer
            .parse_sequence(std::path::Path::new(seq_path), None)
            .map_err(|e| AppError::ImportError {
                message: e.to_string(),
            })?;
    }

    let guid_map = importer.guid_map().clone();
    let show = importer.into_show();

    let setup_name = if show.name.is_empty() {
        "Vixen Import".to_string()
    } else {
        show.name.clone()
    };
    let summary = setup::create_setup(&data_dir, &setup_name).map_err(AppError::from)?;

    let s = setup::Setup {
        name: setup_name,
        slug: summary.slug.clone(),
        fixtures: show.fixtures.clone(),
        groups: show.groups.clone(),
        controllers: show.controllers.clone(),
        patches: show.patches.clone(),
        layout: show.layout.clone(),
    };
    setup::save_setup(&data_dir, &summary.slug, &s).map_err(AppError::from)?;
    setup::save_vixen_guid_map(&data_dir, &summary.slug, &guid_map)
        .map_err(AppError::from)?;

    for seq in &show.sequences {
        setup::create_sequence(&data_dir, &summary.slug, &seq.name)
            .map_err(AppError::from)?;
        let seq_slug = crate::project::slugify(&seq.name);
        setup::save_sequence(&data_dir, &summary.slug, &seq_slug, seq)
            .map_err(AppError::from)?;
    }

    let setups = setup::list_setups(&data_dir).map_err(AppError::from)?;
    let updated_summary = setups
        .into_iter()
        .find(|s| s.slug == summary.slug)
        .unwrap_or(summary);

    Ok(CommandOutput::new("Vixen import complete.", CommandResult::ImportVixen(updated_summary)))
}

pub fn import_vixen_setup(
    state: &Arc<AppState>,
    p: ImportVixenSetupParams,
) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;

    let mut importer = crate::import::vixen::VixenImporter::new();
    importer
        .parse_system_config(std::path::Path::new(&p.system_config_path))
        .map_err(|e| AppError::ImportError {
            message: e.to_string(),
        })?;

    let guid_map = importer.guid_map().clone();
    let show = importer.into_show();

    let setup_name = "Vixen Import".to_string();
    let summary = setup::create_setup(&data_dir, &setup_name).map_err(AppError::from)?;

    let s = setup::Setup {
        name: setup_name,
        slug: summary.slug.clone(),
        fixtures: show.fixtures.clone(),
        groups: show.groups.clone(),
        controllers: show.controllers.clone(),
        patches: show.patches.clone(),
        layout: show.layout.clone(),
    };
    setup::save_setup(&data_dir, &summary.slug, &s).map_err(AppError::from)?;
    setup::save_vixen_guid_map(&data_dir, &summary.slug, &guid_map)
        .map_err(AppError::from)?;

    let setups = setup::list_setups(&data_dir).map_err(AppError::from)?;
    let updated_summary = setups
        .into_iter()
        .find(|s| s.slug == summary.slug)
        .unwrap_or(summary);

    Ok(CommandOutput::new(
        "Vixen setup import complete.",
        CommandResult::ImportVixenSetup(updated_summary),
    ))
}

pub fn import_vixen_sequence(
    state: &Arc<AppState>,
    p: ImportVixenSequenceParams,
) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;

    let setup_data =
        setup::load_setup(&data_dir, &p.setup_slug).map_err(AppError::from)?;
    let guid_map =
        setup::load_vixen_guid_map(&data_dir, &p.setup_slug).map_err(AppError::from)?;

    if guid_map.is_empty() {
        return Err(AppError::ImportError {
            message: "No Vixen GUID map found for this setup. Import the setup from Vixen first.".into(),
        });
    }

    let mut importer = crate::import::vixen::VixenImporter::from_setup(
        setup_data.fixtures,
        setup_data.groups,
        setup_data.controllers,
        setup_data.patches,
        guid_map,
    );

    importer
        .parse_sequence(std::path::Path::new(&p.tim_path), None)
        .map_err(|e| AppError::ImportError {
            message: e.to_string(),
        })?;

    let sequences = importer.into_sequences();
    let seq = sequences.into_iter().next().ok_or(AppError::ImportError {
        message: "No sequence parsed from file".into(),
    })?;

    let seq_slug = crate::project::slugify(&seq.name);
    if let Err(e) = setup::create_sequence(&data_dir, &p.setup_slug, &seq.name) {
        eprintln!("[VibeLights] Failed to create sequence entry: {e}");
    }
    setup::save_sequence(&data_dir, &p.setup_slug, &seq_slug, &seq)
        .map_err(AppError::from)?;

    let summary = setup::SequenceSummary {
        name: seq.name,
        slug: seq_slug,
    };
    Ok(CommandOutput::new(
        "Vixen sequence imported.",
        CommandResult::ImportVixenSequence(summary),
    ))
}

pub fn scan_vixen_directory(_state: &Arc<AppState>, p: ScanVixenDirectoryParams) -> Result<CommandOutput, AppError> {
    use crate::import::vixen_preview;

    let vixen_path = std::path::Path::new(&p.vixen_dir);
    let config_path = vixen_path.join(crate::import::VIXEN_SYSTEM_DATA_DIR).join(crate::import::VIXEN_SYSTEM_CONFIG_FILE);
    if !config_path.exists() {
        return Err(AppError::ImportError {
            message: format!(
                "Not a valid Vixen 3 directory: SystemData/SystemConfig.xml not found in {}",
                p.vixen_dir
            ),
        });
    }

    let mut importer = crate::import::vixen::VixenImporter::new();
    importer
        .parse_system_config(&config_path)
        .map_err(|e| AppError::ImportError {
            message: e.to_string(),
        })?;

    let fixtures_found = importer.fixture_count();
    let groups_found = importer.group_count();
    let controllers_found = importer.controller_count();

    let preview_file = vixen_preview::find_preview_file(vixen_path);
    let (preview_available, preview_item_count) = if let Some(ref pf) = preview_file {
        match vixen_preview::parse_preview_file(pf) {
            Ok(data) => (!data.display_items.is_empty(), data.display_items.len()),
            Err(_) => (false, 0),
        }
    } else {
        (false, 0)
    };
    let preview_file_path = preview_file.map(|p| p.to_string_lossy().to_string());

    let mut sequences = Vec::new();
    let seq_dir = vixen_path.join("Sequence");
    if seq_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&seq_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if ext == crate::import::VIXEN_SEQUENCE_EXT {
                        let filename = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                        sequences.push(crate::import::vixen::VixenSequenceInfo {
                            filename,
                            path: path.to_string_lossy().to_string(),
                            size_bytes,
                        });
                    }
                }
            }
        }
    }
    sequences.sort_by(|a, b| a.filename.cmp(&b.filename));

    let mut media_files = Vec::new();
    let media_dir = vixen_path.join("Media");
    if media_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&media_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if MEDIA_EXTENSIONS.contains(&ext.as_str()) {
                        let filename = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                        media_files.push(crate::import::vixen::VixenMediaInfo {
                            filename,
                            path: path.to_string_lossy().to_string(),
                            size_bytes,
                        });
                    }
                }
            }
        }
    }
    media_files.sort_by(|a, b| a.filename.cmp(&b.filename));

    let discovery = crate::import::vixen::VixenDiscovery {
        vixen_dir: p.vixen_dir,
        fixtures_found,
        groups_found,
        controllers_found,
        preview_available,
        preview_item_count,
        preview_file_path,
        sequences,
        media_files,
    };
    Ok(CommandOutput::new("Vixen directory scanned.", CommandResult::ScanVixenDirectory(Box::new(discovery))))
}

// ── Async handler ────────────────────────────────────────────────

#[cfg(feature = "tauri-app")]
#[allow(clippy::too_many_lines)]
pub async fn execute_vixen_import(
    state: Arc<AppState>,
    app: Option<tauri::AppHandle>,
    config: crate::import::vixen::VixenImportConfig,
) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(&state).map_err(|_| AppError::NoSettings)?;
    let cancel_flag = state.cancellation.register("import");

    let app_ref = app.clone();
    let emit = move |step: &str, msg: &str, pct: f64, detail: Option<&str>| {
        if let Some(ref a) = app_ref {
            crate::progress::emit_progress(a, step, msg, pct, detail);
        }
    };

    let emit2 = emit.clone();
    let result = tokio::time::timeout(
        tokio::time::Duration::from_secs(600),
        tokio::task::spawn_blocking(move || {
            let vixen_path = std::path::Path::new(&config.vixen_dir);
            let config_path = vixen_path
                .join(crate::import::VIXEN_SYSTEM_DATA_DIR)
                .join(crate::import::VIXEN_SYSTEM_CONFIG_FILE);

            emit2("import", "Parsing system config...", 0.05, None);
            crate::state::check_cancelled(&cancel_flag, "import")?;
            let mut importer = crate::import::vixen::VixenImporter::new();
            importer
                .parse_system_config(&config_path)
                .map_err(|e| AppError::ImportError { message: e.to_string() })?;

            crate::state::check_cancelled(&cancel_flag, "import")?;
            let layout_items = if config.import_layout {
                emit2("import", "Parsing layout...", 0.1, None);
                let override_path = config
                    .preview_file_override
                    .as_deref()
                    .map(std::path::Path::new);
                match importer.parse_preview(vixen_path, override_path) {
                    Ok(layouts) => layouts,
                    Err(e) => {
                        eprintln!("[VibeLights] Preview import warning: {e}");
                        Vec::new()
                    }
                }
            } else {
                Vec::new()
            };

            crate::state::check_cancelled(&cancel_flag, "import")?;
            let total_seqs = config.sequence_paths.len();
            let mut sequences_imported = 0usize;
            for (i, seq_path) in config.sequence_paths.iter().enumerate() {
                crate::state::check_cancelled(&cancel_flag, "import")?;
                #[allow(clippy::cast_precision_loss)]
                let base_progress = 0.15 + 0.45 * (i as f64 / total_seqs.max(1) as f64);
                #[allow(clippy::cast_precision_loss)]
                let next_progress = 0.15 + 0.45 * ((i + 1) as f64 / total_seqs.max(1) as f64);
                let detail = format!("Sequence {} of {}", i + 1, total_seqs);
                emit2("import", "Parsing sequences...", base_progress, Some(&detail));
                let emit3 = emit2.clone();
                let detail2 = detail.clone();
                let progress_cb = move |frac: f64| {
                    let p = base_progress + (next_progress - base_progress) * frac;
                    emit3("import", "Parsing sequences...", p, Some(&detail2));
                };
                match importer.parse_sequence(std::path::Path::new(seq_path), Some(&progress_cb)) {
                    Ok(()) => sequences_imported += 1,
                    Err(e) => {
                        eprintln!("[VibeLights] Sequence import warning: {e}");
                    }
                }
            }

            let guid_map = importer.guid_map().clone();
            let warnings: Vec<String> = importer.warnings().to_vec();
            let show = importer.into_show();

            let fixtures_imported = show.fixtures.len();
            let groups_imported = show.groups.len();
            let controllers_imported = if config.import_controllers {
                show.controllers.len()
            } else {
                0
            };
            let layout_items_imported = layout_items.len();

            crate::state::check_cancelled(&cancel_flag, "import")?;
            emit2("import", "Saving setup...", 0.65, None);
            let setup_name = if config.setup_name.trim().is_empty() {
                "Vixen Import".to_string()
            } else {
                config.setup_name.trim().to_string()
            };
            let summary =
                setup::create_setup(&data_dir, &setup_name).map_err(AppError::from)?;

            let layout = if layout_items.is_empty() {
                show.layout.clone()
            } else {
                crate::model::show::Layout {
                    fixtures: layout_items,
                }
            };

            let prof = setup::Setup {
                name: setup_name,
                slug: summary.slug.clone(),
                fixtures: show.fixtures.clone(),
                groups: show.groups.clone(),
                controllers: if config.import_controllers {
                    show.controllers.clone()
                } else {
                    Vec::new()
                },
                patches: if config.import_controllers {
                    show.patches.clone()
                } else {
                    Vec::new()
                },
                layout,
            };
            setup::save_setup(&data_dir, &summary.slug, &prof).map_err(AppError::from)?;
            setup::save_vixen_guid_map(&data_dir, &summary.slug, &guid_map)
                .map_err(AppError::from)?;

            crate::state::check_cancelled(&cancel_flag, "import")?;
            let mut media_imported = 0usize;
            if !config.media_filenames.is_empty() {
                emit2("import", "Copying media files...", 0.70, None);
                for media_filename in &config.media_filenames {
                    crate::state::check_cancelled(&cancel_flag, "import")?;
                    let source = vixen_path.join("Media").join(media_filename);
                    if source.exists() {
                        match setup::import_media(&data_dir, &summary.slug, &source) {
                            Ok(_) => media_imported += 1,
                            Err(e) => {
                                eprintln!("[VibeLights] Media import warning: {e}");
                            }
                        }
                    }
                }
            }

            crate::state::check_cancelled(&cancel_flag, "import")?;
            emit2("import", "Saving sequences...", 0.75, None);
            for (i, seq) in show.sequences.iter().enumerate() {
                crate::state::check_cancelled(&cancel_flag, "import")?;
                #[allow(clippy::cast_precision_loss)]
                let progress = 0.75 + 0.15 * (i as f64 / show.sequences.len().max(1) as f64);
                emit2(
                    "import",
                    "Saving sequences...",
                    progress,
                    Some(&format!("Sequence {} of {}", i + 1, show.sequences.len())),
                );
                let mut seq = seq.clone();
                if let Some(ref audio_path) = seq.audio_file {
                    let audio_basename = std::path::Path::new(audio_path)
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string());
                    if let Some(ref basename) = audio_basename {
                        if config.media_filenames.iter().any(|m| m == basename) {
                            seq.audio_file = Some(basename.clone());
                        } else {
                            seq.audio_file = None;
                        }
                    }
                }
                if let Err(e) = setup::create_sequence(&data_dir, &summary.slug, &seq.name) {
                    eprintln!("[VibeLights] Failed to create sequence entry: {e}");
                }
                let seq_slug = crate::project::slugify(&seq.name);
                setup::save_sequence(&data_dir, &summary.slug, &seq_slug, &seq)
                    .map_err(AppError::from)?;
            }

            emit2("import", "Import complete", 1.0, None);

            Ok::<_, AppError>(crate::import::vixen::VixenImportResult {
                setup_slug: summary.slug,
                fixtures_imported,
                groups_imported,
                controllers_imported,
                layout_items_imported,
                sequences_imported,
                media_imported,
                warnings,
            })
        }),
    )
    .await;

    state.cancellation.unregister("import");

    match result {
        Ok(join_result) => {
            let import_result = join_result.map_err(|e| AppError::ApiError { message: e.to_string() })??;
            Ok(CommandOutput::new(
                "Vixen import complete.",
                CommandResult::ExecuteVixenImport(import_result),
            ))
        }
        Err(_elapsed) => {
            if let Some(ref a) = app {
                crate::progress::emit_progress(a, "import", "Import timed out", 1.0, None);
            }
            Err(AppError::ImportError {
                message: "Import timed out after 10 minutes".into(),
            })
        }
    }
}

pub fn check_vixen_preview_file(
    _state: &Arc<AppState>,
    p: CheckVixenPreviewFileParams,
) -> Result<CommandOutput, AppError> {
    use crate::import::vixen_preview;

    let path = std::path::Path::new(&p.file_path);
    if !path.exists() {
        return Err(AppError::NotFound {
            what: "Preview file".into(),
        });
    }

    let data = vixen_preview::parse_preview_file(path).map_err(|e| AppError::ImportError {
        message: e.to_string(),
    })?;
    if data.display_items.is_empty() {
        return Err(AppError::ImportError {
            message: "File was parsed but no display items were found. \
                      The file may not contain Vixen preview/layout data."
                .into(),
        });
    }

    let count = data.display_items.len();
    Ok(CommandOutput::new(
        format!("{count} display items found."),
        CommandResult::CheckVixenPreviewFile(count),
    ))
}
