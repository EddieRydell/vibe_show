#![allow(clippy::needless_pass_by_value, clippy::too_many_lines)]

use std::sync::Arc;

use crate::error::AppError;
use crate::profile::{self, MEDIA_EXTENSIONS};
use crate::registry::params::{
    CheckVixenPreviewFileParams, ImportVixenParams, ImportVixenProfileParams,
    ImportVixenSequenceParams, ScanVixenDirectoryParams,
};
use crate::registry::CommandOutput;
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

    let profile_name = if show.name.is_empty() {
        "Vixen Import".to_string()
    } else {
        show.name.clone()
    };
    let summary = profile::create_profile(&data_dir, &profile_name).map_err(AppError::from)?;

    let prof = profile::Profile {
        name: profile_name,
        slug: summary.slug.clone(),
        fixtures: show.fixtures.clone(),
        groups: show.groups.clone(),
        controllers: show.controllers.clone(),
        patches: show.patches.clone(),
        layout: show.layout.clone(),
    };
    profile::save_profile(&data_dir, &summary.slug, &prof).map_err(AppError::from)?;
    profile::save_vixen_guid_map(&data_dir, &summary.slug, &guid_map)
        .map_err(AppError::from)?;

    for seq in &show.sequences {
        profile::create_sequence(&data_dir, &summary.slug, &seq.name)
            .map_err(AppError::from)?;
        let seq_slug = crate::project::slugify(&seq.name);
        profile::save_sequence(&data_dir, &summary.slug, &seq_slug, seq)
            .map_err(AppError::from)?;
    }

    let profiles = profile::list_profiles(&data_dir).map_err(AppError::from)?;
    let updated_summary = profiles
        .into_iter()
        .find(|p| p.slug == summary.slug)
        .unwrap_or(summary);

    Ok(CommandOutput::json("Vixen import complete.", &updated_summary))
}

pub fn import_vixen_profile(
    state: &Arc<AppState>,
    p: ImportVixenProfileParams,
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

    let profile_name = "Vixen Import".to_string();
    let summary = profile::create_profile(&data_dir, &profile_name).map_err(AppError::from)?;

    let prof = profile::Profile {
        name: profile_name,
        slug: summary.slug.clone(),
        fixtures: show.fixtures.clone(),
        groups: show.groups.clone(),
        controllers: show.controllers.clone(),
        patches: show.patches.clone(),
        layout: show.layout.clone(),
    };
    profile::save_profile(&data_dir, &summary.slug, &prof).map_err(AppError::from)?;
    profile::save_vixen_guid_map(&data_dir, &summary.slug, &guid_map)
        .map_err(AppError::from)?;

    let profiles = profile::list_profiles(&data_dir).map_err(AppError::from)?;
    let updated_summary = profiles
        .into_iter()
        .find(|p| p.slug == summary.slug)
        .unwrap_or(summary);

    Ok(CommandOutput::json(
        "Vixen profile import complete.",
        &updated_summary,
    ))
}

pub fn import_vixen_sequence(
    state: &Arc<AppState>,
    p: ImportVixenSequenceParams,
) -> Result<CommandOutput, AppError> {
    let data_dir = get_data_dir(state).map_err(|_| AppError::NoSettings)?;

    let prof =
        profile::load_profile(&data_dir, &p.profile_slug).map_err(AppError::from)?;
    let guid_map =
        profile::load_vixen_guid_map(&data_dir, &p.profile_slug).map_err(AppError::from)?;

    if guid_map.is_empty() {
        return Err(AppError::ImportError {
            message: "No Vixen GUID map found for this profile. Import the profile from Vixen first.".into(),
        });
    }

    let mut importer = crate::import::vixen::VixenImporter::from_profile(
        prof.fixtures,
        prof.groups,
        prof.controllers,
        prof.patches,
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
    if let Err(e) = profile::create_sequence(&data_dir, &p.profile_slug, &seq.name) {
        eprintln!("[VibeLights] Failed to create sequence entry: {e}");
    }
    profile::save_sequence(&data_dir, &p.profile_slug, &seq_slug, &seq)
        .map_err(AppError::from)?;

    let summary = profile::SequenceSummary {
        name: seq.name,
        slug: seq_slug,
    };
    Ok(CommandOutput::json(
        "Vixen sequence imported.",
        &summary,
    ))
}

pub fn scan_vixen_directory(p: ScanVixenDirectoryParams) -> Result<CommandOutput, AppError> {
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
    Ok(CommandOutput::json("Vixen directory scanned.", &discovery))
}

pub fn check_vixen_preview_file(
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
    Ok(CommandOutput::json(
        format!("{count} display items found."),
        &count,
    ))
}
