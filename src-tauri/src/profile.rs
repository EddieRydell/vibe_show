use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::fixture::{Controller, FixtureDef, FixtureGroup, Patch};
use crate::model::show::{Layout, Show};
use crate::model::timeline::Sequence;
use crate::project::{read_json, slugify, write_json, ProjectError};

// ── Profile types ──────────────────────────────────────────────────

const PROFILE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMeta {
    pub version: u32,
    pub name: String,
    pub created_at: String,
}

/// Summary info for listing profiles (cheap to compute).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProfileSummary {
    pub name: String,
    pub slug: String,
    pub created_at: String,
    pub sequence_count: usize,
    pub fixture_count: usize,
}

/// Full profile data loaded into memory.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Profile {
    pub name: String,
    pub slug: String,
    pub fixtures: Vec<FixtureDef>,
    pub groups: Vec<FixtureGroup>,
    pub controllers: Vec<Controller>,
    pub patches: Vec<Patch>,
    pub layout: Layout,
}

// ── Sequence types ─────────────────────────────────────────────────

/// Summary info for listing sequences.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SequenceSummary {
    pub name: String,
    pub slug: String,
}

// ── Media types ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MediaFile {
    pub filename: String,
    #[ts(type = "number")]
    pub size_bytes: u64,
}

// ── Envelope types for disk files ──────────────────────────────────

#[derive(Serialize, Deserialize)]
struct FixturesFile {
    fixtures: Vec<FixtureDef>,
    groups: Vec<FixtureGroup>,
    /// Mapping from Vixen GUIDs to VibeLights fixture/group IDs.
    /// Persisted so that sequence imports can resolve effect targets.
    #[serde(default)]
    vixen_guid_map: std::collections::HashMap<String, u32>,
}

#[derive(Serialize, Deserialize)]
struct SetupFile {
    controllers: Vec<Controller>,
    patches: Vec<Patch>,
}

// ── Profile operations ─────────────────────────────────────────────

fn profiles_dir(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("profiles")
}

fn profile_dir(data_dir: &Path, slug: &str) -> std::path::PathBuf {
    profiles_dir(data_dir).join(slug)
}

/// List all profiles in the data directory.
pub fn list_profiles(data_dir: &Path) -> Result<Vec<ProfileSummary>, ProjectError> {
    let dir = profiles_dir(data_dir);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut profiles = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let meta_path = entry.path().join("profile.json");
        if !meta_path.exists() {
            continue;
        }
        let meta: ProfileMeta = match read_json(&meta_path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let slug = entry
            .file_name()
            .to_string_lossy()
            .to_string();

        // Count fixtures
        let fixture_count = read_json::<FixturesFile>(&entry.path().join("fixtures.json"))
            .map(|f| f.fixtures.len())
            .unwrap_or(0);

        // Count sequences
        let seq_dir = entry.path().join("sequences");
        let sequence_count = if seq_dir.exists() {
            fs::read_dir(&seq_dir)
                .map(|rd| {
                    rd.filter_map(|e| e.ok())
                        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
                        .count()
                })
                .unwrap_or(0)
        } else {
            0
        };

        profiles.push(ProfileSummary {
            name: meta.name,
            slug,
            created_at: meta.created_at,
            sequence_count,
            fixture_count,
        });
    }

    Ok(profiles)
}

/// Create a new empty profile.
pub fn create_profile(data_dir: &Path, name: &str) -> Result<ProfileSummary, ProjectError> {
    let slug = slugify(name);
    let dir = profile_dir(data_dir, &slug);

    if dir.exists() {
        return Err(ProjectError::InvalidProject(format!(
            "Profile '{}' already exists",
            slug
        )));
    }

    fs::create_dir_all(&dir)?;
    fs::create_dir_all(dir.join("sequences"))?;
    fs::create_dir_all(dir.join("media"))?;

    let now = chrono_now();

    write_json(
        &dir.join("profile.json"),
        &ProfileMeta {
            version: PROFILE_VERSION,
            name: name.to_string(),
            created_at: now.clone(),
        },
    )?;

    write_json(
        &dir.join("fixtures.json"),
        &FixturesFile {
            fixtures: Vec::new(),
            groups: Vec::new(),
            vixen_guid_map: std::collections::HashMap::new(),
        },
    )?;

    write_json(
        &dir.join("setup.json"),
        &SetupFile {
            controllers: Vec::new(),
            patches: Vec::new(),
        },
    )?;

    write_json(
        &dir.join("layout.json"),
        &Layout {
            fixtures: Vec::new(),
        },
    )?;

    Ok(ProfileSummary {
        name: name.to_string(),
        slug,
        created_at: now,
        sequence_count: 0,
        fixture_count: 0,
    })
}

/// Load full profile data.
pub fn load_profile(data_dir: &Path, slug: &str) -> Result<Profile, ProjectError> {
    let dir = profile_dir(data_dir, slug);
    if !dir.exists() {
        return Err(ProjectError::InvalidProject(format!(
            "Profile '{}' not found",
            slug
        )));
    }

    let meta: ProfileMeta = read_json(&dir.join("profile.json"))?;
    let fixtures_file: FixturesFile = read_json(&dir.join("fixtures.json"))?;
    let setup_file: SetupFile = read_json(&dir.join("setup.json"))?;
    let layout: Layout = read_json(&dir.join("layout.json"))?;

    Ok(Profile {
        name: meta.name,
        slug: slug.to_string(),
        fixtures: fixtures_file.fixtures,
        groups: fixtures_file.groups,
        controllers: setup_file.controllers,
        patches: setup_file.patches,
        layout,
    })
}

/// Save profile house data (fixtures, groups, controllers, patches, layout).
pub fn save_profile(data_dir: &Path, slug: &str, profile: &Profile) -> Result<(), ProjectError> {
    let dir = profile_dir(data_dir, slug);

    // Preserve existing vixen_guid_map if present
    let existing_map = read_json::<FixturesFile>(&dir.join("fixtures.json"))
        .map(|f| f.vixen_guid_map)
        .unwrap_or_default();

    write_json(
        &dir.join("fixtures.json"),
        &FixturesFile {
            fixtures: profile.fixtures.clone(),
            groups: profile.groups.clone(),
            vixen_guid_map: existing_map,
        },
    )?;

    write_json(
        &dir.join("setup.json"),
        &SetupFile {
            controllers: profile.controllers.clone(),
            patches: profile.patches.clone(),
        },
    )?;

    write_json(&dir.join("layout.json"), &profile.layout)?;

    Ok(())
}

/// Delete a profile and all its data.
pub fn delete_profile(data_dir: &Path, slug: &str) -> Result<(), ProjectError> {
    let dir = profile_dir(data_dir, slug);
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

// ── Sequence operations ────────────────────────────────────────────

fn sequences_dir(data_dir: &Path, profile_slug: &str) -> std::path::PathBuf {
    profile_dir(data_dir, profile_slug).join("sequences")
}

/// List all sequences in a profile.
pub fn list_sequences(
    data_dir: &Path,
    profile_slug: &str,
) -> Result<Vec<SequenceSummary>, ProjectError> {
    let dir = sequences_dir(data_dir, profile_slug);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut seqs = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let seq: Sequence = match read_json(&entry.path()) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let slug = entry
            .path()
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        seqs.push(SequenceSummary {
            name: seq.name,
            slug,
        });
    }

    Ok(seqs)
}

/// Create a new empty sequence in a profile.
pub fn create_sequence(
    data_dir: &Path,
    profile_slug: &str,
    name: &str,
) -> Result<SequenceSummary, ProjectError> {
    let slug = slugify(name);
    let dir = sequences_dir(data_dir, profile_slug);
    fs::create_dir_all(&dir)?;

    let path = dir.join(format!("{}.json", slug));
    if path.exists() {
        return Err(ProjectError::InvalidProject(format!(
            "Sequence '{}' already exists",
            slug
        )));
    }

    let seq = Sequence {
        name: name.to_string(),
        duration: 30.0,
        frame_rate: 30.0,
        audio_file: None,
        tracks: Vec::new(),
    };
    write_json(&path, &seq)?;

    Ok(SequenceSummary {
        name: name.to_string(),
        slug,
    })
}

/// Load a single sequence from a profile.
pub fn load_sequence(
    data_dir: &Path,
    profile_slug: &str,
    seq_slug: &str,
) -> Result<Sequence, ProjectError> {
    let path = sequences_dir(data_dir, profile_slug).join(format!("{}.json", seq_slug));
    if !path.exists() {
        return Err(ProjectError::InvalidProject(format!(
            "Sequence '{}' not found",
            seq_slug
        )));
    }
    read_json(&path)
}

/// Save a sequence to a profile.
pub fn save_sequence(
    data_dir: &Path,
    profile_slug: &str,
    seq_slug: &str,
    sequence: &Sequence,
) -> Result<(), ProjectError> {
    let dir = sequences_dir(data_dir, profile_slug);
    fs::create_dir_all(&dir)?;
    write_json(&dir.join(format!("{}.json", seq_slug)), sequence)
}

/// Delete a sequence from a profile.
pub fn delete_sequence(
    data_dir: &Path,
    profile_slug: &str,
    seq_slug: &str,
) -> Result<(), ProjectError> {
    let path = sequences_dir(data_dir, profile_slug).join(format!("{}.json", seq_slug));
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

// ── Media operations ───────────────────────────────────────────────

const MEDIA_EXTENSIONS: &[&str] = &["mp3", "wav", "ogg", "flac", "m4a", "aac"];

pub fn media_dir(data_dir: &Path, profile_slug: &str) -> std::path::PathBuf {
    profile_dir(data_dir, profile_slug).join("media")
}

/// List audio files in a profile's media directory.
pub fn list_media(
    data_dir: &Path,
    profile_slug: &str,
) -> Result<Vec<MediaFile>, ProjectError> {
    let dir = media_dir(data_dir, profile_slug);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !MEDIA_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }
        let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
        files.push(MediaFile {
            filename: entry.file_name().to_string_lossy().to_string(),
            size_bytes,
        });
    }

    Ok(files)
}

/// Import (copy) an audio file into the profile's media directory.
pub fn import_media(
    data_dir: &Path,
    profile_slug: &str,
    source_path: &Path,
) -> Result<MediaFile, ProjectError> {
    let dir = media_dir(data_dir, profile_slug);
    fs::create_dir_all(&dir)?;

    let filename = source_path
        .file_name()
        .ok_or_else(|| ProjectError::InvalidProject("Invalid source path".into()))?;

    let dest = dir.join(filename);
    fs::copy(source_path, &dest)?;

    let size_bytes = fs::metadata(&dest)?.len();

    Ok(MediaFile {
        filename: filename.to_string_lossy().to_string(),
        size_bytes,
    })
}

/// Delete an audio file from the profile's media directory.
pub fn delete_media(
    data_dir: &Path,
    profile_slug: &str,
    filename: &str,
) -> Result<(), ProjectError> {
    let path = media_dir(data_dir, profile_slug).join(filename);
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

// ── Assembly ───────────────────────────────────────────────────────

/// Combine a Profile (fixtures, setup) with a single Sequence into a full Show
/// that the engine can evaluate.
pub fn assemble_show(profile: &Profile, sequence: &Sequence) -> Show {
    Show {
        name: sequence.name.clone(),
        fixtures: profile.fixtures.clone(),
        groups: profile.groups.clone(),
        layout: profile.layout.clone(),
        sequences: vec![sequence.clone()],
        patches: profile.patches.clone(),
        controllers: profile.controllers.clone(),
    }
}

// ── Vixen GUID map persistence ─────────────────────────────────────

/// Save a Vixen GUID → ID map into a profile's fixtures file.
pub fn save_vixen_guid_map(
    data_dir: &Path,
    profile_slug: &str,
    guid_map: &std::collections::HashMap<String, u32>,
) -> Result<(), ProjectError> {
    let dir = profile_dir(data_dir, profile_slug);
    let path = dir.join("fixtures.json");
    let mut file: FixturesFile = read_json(&path)?;
    file.vixen_guid_map = guid_map.clone();
    write_json(&path, &file)
}

/// Load a Vixen GUID → ID map from a profile's fixtures file.
pub fn load_vixen_guid_map(
    data_dir: &Path,
    profile_slug: &str,
) -> Result<std::collections::HashMap<String, u32>, ProjectError> {
    let dir = profile_dir(data_dir, profile_slug);
    let file: FixturesFile = read_json(&dir.join("fixtures.json"))?;
    Ok(file.vixen_guid_map)
}

// ── Helpers ────────────────────────────────────────────────────────

/// Simple ISO 8601 timestamp (no external crate dependency).
fn chrono_now() -> String {
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    // Simple UTC timestamp: seconds since epoch as ISO-ish string
    // Format: YYYY-MM-DDTHH:MM:SSZ (approximate, no leap seconds)
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Calculate year/month/day from days since epoch (1970-01-01)
    let (year, month, day) = days_to_ymd(days);

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let months_days: &[u64] = if is_leap(year) {
        &[31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        &[31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1;
    for &md in months_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(year: u64) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicU32, Ordering};
    static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn setup_test_dir() -> std::path::PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "vibelights_test_profile_{}_{}",
            std::process::id(),
            id
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_create_and_list_profiles() {
        let data_dir = setup_test_dir();

        // Initially empty
        let profiles = list_profiles(&data_dir).unwrap();
        assert!(profiles.is_empty());

        // Create a profile
        let summary = create_profile(&data_dir, "My House").unwrap();
        assert_eq!(summary.name, "My House");
        assert_eq!(summary.slug, "my-house");
        assert_eq!(summary.sequence_count, 0);
        assert_eq!(summary.fixture_count, 0);

        // List should have one
        let profiles = list_profiles(&data_dir).unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].slug, "my-house");

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn test_load_save_profile() {
        let data_dir = setup_test_dir();
        create_profile(&data_dir, "Test").unwrap();

        let mut profile = load_profile(&data_dir, "test").unwrap();
        assert_eq!(profile.name, "Test");
        assert!(profile.fixtures.is_empty());

        // Add a fixture and save
        profile.fixtures.push(crate::model::FixtureDef {
            id: crate::model::FixtureId(1),
            name: "Pixel String 1".into(),
            color_model: crate::model::fixture::ColorModel::Rgb,
            pixel_count: 50,
            pixel_type: Default::default(),
            bulb_shape: Default::default(),
            display_radius_override: None,
            channel_order: Default::default(),
        });
        save_profile(&data_dir, "test", &profile).unwrap();

        // Reload and verify
        let reloaded = load_profile(&data_dir, "test").unwrap();
        assert_eq!(reloaded.fixtures.len(), 1);
        assert_eq!(reloaded.fixtures[0].name, "Pixel String 1");

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn test_sequence_crud() {
        let data_dir = setup_test_dir();
        create_profile(&data_dir, "Test").unwrap();

        // Create sequence
        let seq = create_sequence(&data_dir, "test", "Intro Scene").unwrap();
        assert_eq!(seq.slug, "intro-scene");

        // List sequences
        let seqs = list_sequences(&data_dir, "test").unwrap();
        assert_eq!(seqs.len(), 1);

        // Load sequence
        let loaded = load_sequence(&data_dir, "test", "intro-scene").unwrap();
        assert_eq!(loaded.name, "Intro Scene");
        assert_eq!(loaded.duration, 30.0);

        // Delete sequence
        delete_sequence(&data_dir, "test", "intro-scene").unwrap();
        let seqs = list_sequences(&data_dir, "test").unwrap();
        assert!(seqs.is_empty());

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn test_delete_profile() {
        let data_dir = setup_test_dir();
        create_profile(&data_dir, "Deletable").unwrap();
        assert_eq!(list_profiles(&data_dir).unwrap().len(), 1);

        delete_profile(&data_dir, "deletable").unwrap();
        assert!(list_profiles(&data_dir).unwrap().is_empty());

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn test_assemble_show() {
        let profile = Profile {
            name: "House".into(),
            slug: "house".into(),
            fixtures: vec![crate::model::FixtureDef {
                id: crate::model::FixtureId(1),
                name: "String".into(),
                color_model: crate::model::fixture::ColorModel::Rgb,
                pixel_count: 10,
                pixel_type: Default::default(),
                bulb_shape: Default::default(),
                display_radius_override: None,
                channel_order: Default::default(),
            }],
            groups: Vec::new(),
            controllers: Vec::new(),
            patches: Vec::new(),
            layout: Layout { fixtures: Vec::new() },
        };
        let sequence = Sequence {
            name: "Xmas".into(),
            duration: 30.0,
            frame_rate: 30.0,
            audio_file: None,
            tracks: Vec::new(),
        };
        let show = assemble_show(&profile, &sequence);
        assert_eq!(show.name, "Xmas");
        assert_eq!(show.fixtures.len(), 1);
        assert_eq!(show.sequences.len(), 1);
    }
}
