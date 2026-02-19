use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::model::fixture::{Controller, FixtureDef, FixtureGroup, Patch};
use crate::model::show::{Layout, Show};
use crate::model::timeline::Sequence;
use crate::project::{read_json, slugify, write_json, ProjectError};

// ── Profile types ──────────────────────────────────────────────────

const PROFILE_VERSION: u32 = 1;
const SHOW_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMeta {
    pub version: u32,
    pub name: String,
    pub created_at: String,
}

/// Summary info for listing profiles (cheap to compute).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileSummary {
    pub name: String,
    pub slug: String,
    pub created_at: String,
    pub show_count: usize,
    pub fixture_count: usize,
}

/// Full profile data loaded into memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub slug: String,
    pub fixtures: Vec<FixtureDef>,
    pub groups: Vec<FixtureGroup>,
    pub controllers: Vec<Controller>,
    pub patches: Vec<Patch>,
    pub layout: Layout,
}

// ── Show types ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShowMeta {
    pub version: u32,
    pub name: String,
}

/// Summary info for listing shows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShowSummary {
    pub name: String,
    pub slug: String,
    pub sequence_count: usize,
}

/// Full show data (sequences only — profile provides fixtures etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShowData {
    pub name: String,
    pub sequences: Vec<Sequence>,
}

// ── Media types ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaFile {
    pub filename: String,
    pub size_bytes: u64,
}

// ── Envelope types for disk files ──────────────────────────────────

#[derive(Serialize, Deserialize)]
struct FixturesFile {
    fixtures: Vec<FixtureDef>,
    groups: Vec<FixtureGroup>,
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

        // Count shows
        let shows_dir = entry.path().join("shows");
        let show_count = if shows_dir.exists() {
            fs::read_dir(&shows_dir)
                .map(|rd| rd.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()).count())
                .unwrap_or(0)
        } else {
            0
        };

        profiles.push(ProfileSummary {
            name: meta.name,
            slug,
            created_at: meta.created_at,
            show_count,
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
    fs::create_dir_all(dir.join("shows"))?;
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
        show_count: 0,
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

    write_json(
        &dir.join("fixtures.json"),
        &FixturesFile {
            fixtures: profile.fixtures.clone(),
            groups: profile.groups.clone(),
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

// ── Show operations ────────────────────────────────────────────────

fn shows_dir(data_dir: &Path, profile_slug: &str) -> std::path::PathBuf {
    profile_dir(data_dir, profile_slug).join("shows")
}

fn show_dir(data_dir: &Path, profile_slug: &str, show_slug: &str) -> std::path::PathBuf {
    shows_dir(data_dir, profile_slug).join(show_slug)
}

/// List all shows in a profile.
pub fn list_shows(
    data_dir: &Path,
    profile_slug: &str,
) -> Result<Vec<ShowSummary>, ProjectError> {
    let dir = shows_dir(data_dir, profile_slug);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut shows = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let meta_path = entry.path().join("show.json");
        if !meta_path.exists() {
            continue;
        }
        let meta: ShowMeta = match read_json(&meta_path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let slug = entry.file_name().to_string_lossy().to_string();

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

        shows.push(ShowSummary {
            name: meta.name,
            slug,
            sequence_count,
        });
    }

    Ok(shows)
}

/// Create a new empty show in a profile.
pub fn create_show(
    data_dir: &Path,
    profile_slug: &str,
    name: &str,
) -> Result<ShowSummary, ProjectError> {
    let slug = slugify(name);
    let dir = show_dir(data_dir, profile_slug, &slug);

    if dir.exists() {
        return Err(ProjectError::InvalidProject(format!(
            "Show '{}' already exists",
            slug
        )));
    }

    fs::create_dir_all(&dir)?;
    fs::create_dir_all(dir.join("sequences"))?;

    write_json(
        &dir.join("show.json"),
        &ShowMeta {
            version: SHOW_VERSION,
            name: name.to_string(),
        },
    )?;

    Ok(ShowSummary {
        name: name.to_string(),
        slug,
        sequence_count: 0,
    })
}

/// Load show data (sequences).
pub fn load_show(
    data_dir: &Path,
    profile_slug: &str,
    show_slug: &str,
) -> Result<ShowData, ProjectError> {
    let dir = show_dir(data_dir, profile_slug, show_slug);
    if !dir.exists() {
        return Err(ProjectError::InvalidProject(format!(
            "Show '{}' not found",
            show_slug
        )));
    }

    let meta: ShowMeta = read_json(&dir.join("show.json"))?;

    let seq_dir = dir.join("sequences");
    let mut sequences: Vec<Sequence> = Vec::new();
    if seq_dir.exists() {
        let mut entries: Vec<_> = fs::read_dir(&seq_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let seq: Sequence = read_json(&entry.path())?;
            sequences.push(seq);
        }
    }

    Ok(ShowData {
        name: meta.name,
        sequences,
    })
}

/// Save show data (sequences).
pub fn save_show(
    data_dir: &Path,
    profile_slug: &str,
    show_slug: &str,
    show_data: &ShowData,
) -> Result<(), ProjectError> {
    let dir = show_dir(data_dir, profile_slug, show_slug);
    let seq_dir = dir.join("sequences");
    fs::create_dir_all(&seq_dir)?;

    // Update show.json name
    write_json(
        &dir.join("show.json"),
        &ShowMeta {
            version: SHOW_VERSION,
            name: show_data.name.clone(),
        },
    )?;

    // Clean out old sequence files
    if seq_dir.exists() {
        for entry in fs::read_dir(&seq_dir)? {
            let entry = entry?;
            if entry.path().extension().is_some_and(|e| e == "json") {
                fs::remove_file(entry.path())?;
            }
        }
    }

    for seq in &show_data.sequences {
        let filename = format!("{}.json", slugify(&seq.name));
        write_json(&seq_dir.join(filename), seq)?;
    }

    Ok(())
}

/// Delete a show.
pub fn delete_show(
    data_dir: &Path,
    profile_slug: &str,
    show_slug: &str,
) -> Result<(), ProjectError> {
    let dir = show_dir(data_dir, profile_slug, show_slug);
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
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

/// Combine a Profile (fixtures, setup) with ShowData (sequences) into a full Show
/// that the engine can evaluate.
pub fn assemble_show(profile: &Profile, show_data: &ShowData) -> Show {
    Show {
        name: show_data.name.clone(),
        fixtures: profile.fixtures.clone(),
        groups: profile.groups.clone(),
        layout: profile.layout.clone(),
        sequences: show_data.sequences.clone(),
        patches: profile.patches.clone(),
        controllers: profile.controllers.clone(),
    }
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
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicU32, Ordering};
    static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn setup_test_dir() -> std::path::PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "vibeshow_test_profile_{}_{}",
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
        assert_eq!(summary.show_count, 0);
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
    fn test_show_crud() {
        let data_dir = setup_test_dir();
        create_profile(&data_dir, "Test").unwrap();

        // Create show
        let show = create_show(&data_dir, "test", "Christmas 2024").unwrap();
        assert_eq!(show.slug, "christmas-2024");

        // List shows
        let shows = list_shows(&data_dir, "test").unwrap();
        assert_eq!(shows.len(), 1);

        // Load show
        let show_data = load_show(&data_dir, "test", "christmas-2024").unwrap();
        assert_eq!(show_data.name, "Christmas 2024");
        assert!(show_data.sequences.is_empty());

        // Delete show
        delete_show(&data_dir, "test", "christmas-2024").unwrap();
        let shows = list_shows(&data_dir, "test").unwrap();
        assert!(shows.is_empty());

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
        let show_data = ShowData {
            name: "Xmas".into(),
            sequences: Vec::new(),
        };
        let show = assemble_show(&profile, &show_data);
        assert_eq!(show.name, "Xmas");
        assert_eq!(show.fixtures.len(), 1);
    }
}
