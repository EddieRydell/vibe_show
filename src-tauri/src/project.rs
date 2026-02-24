use std::collections::HashMap;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use serde::Serialize;

use crate::model::show::{Layout, Show};
use crate::model::fixture::{Controller, FixtureDef, FixtureGroup, Patch};
use crate::model::timeline::Sequence;

/// Project file format version.
const PROJECT_VERSION: u32 = 1;

// ── Error type ──────────────────────────────────────────────────────

#[derive(Debug)]
pub enum ProjectError {
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidProject(String),
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjectError::Io(e) => write!(f, "I/O error: {e}"),
            ProjectError::Json(e) => write!(f, "JSON error: {e}"),
            ProjectError::InvalidProject(msg) => write!(f, "Invalid project: {msg}"),
        }
    }
}

impl From<std::io::Error> for ProjectError {
    fn from(e: std::io::Error) -> Self {
        ProjectError::Io(e)
    }
}

impl From<serde_json::Error> for ProjectError {
    fn from(e: serde_json::Error) -> Self {
        ProjectError::Json(e)
    }
}

impl Serialize for ProjectError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ── JSON envelope types ─────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize)]
struct ProjectMeta {
    version: u32,
    name: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct FixturesFile {
    fixtures: Vec<FixtureDef>,
    groups: Vec<FixtureGroup>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SetupFile {
    controllers: Vec<Controller>,
    patches: Vec<Patch>,
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Convert a name to a safe filename slug.
pub(crate) fn slugify(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    // Collapse multiple dashes
    let mut result = String::new();
    let mut last_dash = false;
    for c in slug.chars() {
        if c == '-' {
            if !last_dash && !result.is_empty() {
                result.push('-');
            }
            last_dash = true;
        } else {
            result.push(c);
            last_dash = false;
        }
    }
    // Trim trailing dash
    while result.ends_with('-') {
        result.pop();
    }
    if result.is_empty() {
        "untitled".to_string()
    } else {
        result
    }
}

/// Per-file mutex map to serialize concurrent writes to the same path.
static FILE_LOCKS: LazyLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Atomically write bytes to a file using write-to-temp-then-rename.
///
/// 1. Acquires a per-file mutex to prevent concurrent writes to the same path
/// 2. Writes data to a `.tmp` sibling file
/// 3. Calls `fsync` to flush to disk
/// 4. Renames the existing file to `.bak` (best-effort)
/// 5. Renames the `.tmp` file to the target path
///
/// This prevents data corruption from power loss or crashes mid-write,
/// and the per-file lock prevents concurrent callers from racing on the `.tmp` file.
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<(), ProjectError> {
    // Acquire per-file lock to serialize writes to the same path
    let lock = {
        let mut locks = FILE_LOCKS.lock().map_err(|e| {
            ProjectError::Io(std::io::Error::other(e.to_string()))
        })?;
        locks
            .entry(path.to_path_buf())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    let _guard = lock.lock().map_err(|e| {
        ProjectError::Io(std::io::Error::other(e.to_string()))
    })?;

    // Build sibling paths: foo.json → foo.json.tmp, foo.json.bak
    let file_name = path.file_name().unwrap_or_default();

    let mut tmp_name = OsString::from(file_name);
    tmp_name.push(".tmp");
    let tmp_path = path.with_file_name(&tmp_name);

    let mut bak_name = OsString::from(file_name);
    bak_name.push(".bak");
    let bak_path = path.with_file_name(&bak_name);

    // Write to temporary file + fsync
    let mut file = fs::File::create(&tmp_path)?;
    file.write_all(data)?;
    file.sync_all()?;
    drop(file);

    // Backup existing file (best-effort — ignore errors)
    if path.exists() {
        let _ = fs::rename(path, &bak_path);
    }

    // Rename temp to target
    fs::rename(&tmp_path, path)?;

    Ok(())
}

pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), ProjectError> {
    let json = serde_json::to_string_pretty(value)?;
    atomic_write(path, json.as_bytes())
}

pub(crate) fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, ProjectError> {
    let data = fs::read_to_string(path)?;
    let value = serde_json::from_str(&data)?;
    Ok(value)
}

// ── Save / Load ─────────────────────────────────────────────────────

/// Save a Show to a .vibelights project directory.
pub fn save_project(show: &Show, dir: &Path) -> Result<(), ProjectError> {
    // Create the directory structure
    fs::create_dir_all(dir)?;
    let seq_dir = dir.join("sequences");
    fs::create_dir_all(&seq_dir)?;

    // project.json
    write_json(
        &dir.join("project.json"),
        &ProjectMeta {
            version: PROJECT_VERSION,
            name: show.name.clone(),
        },
    )?;

    // fixtures.json
    write_json(
        &dir.join("fixtures.json"),
        &FixturesFile {
            fixtures: show.fixtures.clone(),
            groups: show.groups.clone(),
        },
    )?;

    // setup.json
    write_json(
        &dir.join("setup.json"),
        &SetupFile {
            controllers: show.controllers.clone(),
            patches: show.patches.clone(),
        },
    )?;

    // layout.json
    write_json(&dir.join("layout.json"), &show.layout)?;

    // sequences/*.json
    // Clean out old sequence files first
    if seq_dir.exists() {
        for entry in fs::read_dir(&seq_dir)? {
            let entry = entry?;
            if entry.path().extension().is_some_and(|e| e == "json") {
                fs::remove_file(entry.path())?;
            }
        }
    }

    for seq in &show.sequences {
        let filename = format!("{}.json", slugify(&seq.name));
        write_json(&seq_dir.join(filename), seq)?;
    }

    Ok(())
}

/// Load a Show from a .vibelights project directory.
pub fn load_project(dir: &Path) -> Result<Show, ProjectError> {
    // project.json
    let meta: ProjectMeta = read_json(&dir.join("project.json"))?;
    if meta.version > PROJECT_VERSION {
        return Err(ProjectError::InvalidProject(format!(
            "Project version {} is newer than supported version {}",
            meta.version, PROJECT_VERSION
        )));
    }

    // fixtures.json
    let fixtures_file: FixturesFile = read_json(&dir.join("fixtures.json"))?;

    // setup.json
    let setup_file: SetupFile = read_json(&dir.join("setup.json"))?;

    // layout.json
    let layout: Layout = read_json(&dir.join("layout.json"))?;

    // sequences/*.json — sorted by filename for deterministic order
    let seq_dir = dir.join("sequences");
    let mut sequences: Vec<Sequence> = Vec::new();
    if seq_dir.exists() {
        let mut entries: Vec<_> = fs::read_dir(&seq_dir)?
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
            .collect();
        entries.sort_by_key(std::fs::DirEntry::file_name);

        for entry in entries {
            let seq: Sequence = read_json(&entry.path())?;
            sequences.push(seq);
        }
    }

    Ok(Show {
        name: meta.name,
        fixtures: fixtures_file.fixtures,
        groups: fixtures_file.groups,
        layout,
        sequences,
        patches: setup_file.patches,
        controllers: setup_file.controllers,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::model::fixture::{
        BulbShape, ChannelOrder, ColorModel, EffectTarget, FixtureDef, FixtureGroup, FixtureId,
        GroupId, GroupMember, PixelType,
    };
    use crate::model::show::{Layout, Position2D, FixtureLayout, LayoutShape};
    use crate::model::timeline::{
        BlendMode, EffectInstance, EffectKind, EffectParams, ParamKey, ParamValue, Sequence, TimeRange, Track,
    };
    use crate::model::Color;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Demo Sequence"), "demo-sequence");
        assert_eq!(slugify("Jing Jing Jing!"), "jing-jing-jing");
        assert_eq!(slugify("   "), "untitled");
        assert_eq!(slugify("hello---world"), "hello-world");
        assert_eq!(slugify("Test_Name-123"), "test_name-123");
    }

    fn test_show() -> Show {
        Show {
            name: "Test Show".into(),
            fixtures: vec![FixtureDef {
                id: FixtureId(1),
                name: "Strand 1".into(),
                color_model: ColorModel::Rgb,
                pixel_count: 50,
                pixel_type: PixelType::Smart,
                bulb_shape: BulbShape::C9,
                display_radius_override: None,
                channel_order: ChannelOrder::Grb,
            }],
            groups: vec![FixtureGroup {
                id: GroupId(10),
                name: "Front".into(),
                members: vec![GroupMember::Fixture(FixtureId(1))],
            }],
            layout: Layout {
                fixtures: vec![FixtureLayout {
                    fixture_id: FixtureId(1),
                    pixel_positions: vec![Position2D { x: 0.0, y: 0.5 }],
                    shape: LayoutShape::Custom,
                }],
            },
            sequences: vec![Sequence {
                name: "Main".into(),
                duration: 60.0,
                frame_rate: 30.0,
                audio_file: Some("song.mp3".into()),
                tracks: vec![Track {
                    name: "Track 1".into(),
                    target: EffectTarget::All,
                    effects: vec![EffectInstance {
                        kind: EffectKind::Solid,
                        params: EffectParams::new()
                            .set(ParamKey::Color, ParamValue::Color(Color::rgb(255, 0, 0))),
                        time_range: TimeRange::new(0.0, 10.0).unwrap(),
                        blend_mode: BlendMode::Override,
                        opacity: 1.0,
                    }],
                }],
                motion_paths: std::collections::HashMap::new(),
            }],
            patches: vec![],
            controllers: vec![],
            }
    }

    #[test]
    fn save_then_load_round_trip() {
        let dir = std::env::temp_dir().join("vibelights_test_roundtrip");
        let _ = fs::remove_dir_all(&dir);

        let show = test_show();
        save_project(&show, &dir).expect("save failed");
        let loaded = load_project(&dir).expect("load failed");

        assert_eq!(loaded.name, show.name);
        assert_eq!(loaded.fixtures.len(), 1);
        assert_eq!(loaded.fixtures[0].name, "Strand 1");
        assert_eq!(loaded.fixtures[0].pixel_count, 50);
        assert_eq!(loaded.fixtures[0].channel_order, ChannelOrder::Grb);
        assert_eq!(loaded.groups.len(), 1);
        assert_eq!(loaded.groups[0].name, "Front");
        assert_eq!(loaded.sequences.len(), 1);
        assert_eq!(loaded.sequences[0].name, "Main");
        assert_eq!(loaded.sequences[0].tracks.len(), 1);
        assert_eq!(loaded.layout.fixtures.len(), 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sequence_ordering_is_deterministic() {
        let dir = std::env::temp_dir().join("vibelights_test_ordering");
        let _ = fs::remove_dir_all(&dir);

        let mut show = test_show();
        // Add sequences that sort differently by slug than insertion order
        show.sequences = vec![
            Sequence {
                name: "Zebra".into(),
                duration: 10.0,
                frame_rate: 30.0,
                audio_file: None,
                tracks: vec![],
                motion_paths: std::collections::HashMap::new(),
            },
            Sequence {
                name: "Alpha".into(),
                duration: 20.0,
                frame_rate: 30.0,
                audio_file: None,
                tracks: vec![],
                motion_paths: std::collections::HashMap::new(),
            },
        ];

        save_project(&show, &dir).expect("save failed");
        let loaded = load_project(&dir).expect("load failed");

        // Loaded order is sorted by filename slug
        assert_eq!(loaded.sequences[0].name, "Alpha");
        assert_eq!(loaded.sequences[1].name, "Zebra");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn future_version_rejected() {
        let dir = std::env::temp_dir().join("vibelights_test_version");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // Write a project.json with a future version
        let meta = serde_json::json!({ "version": 999, "name": "Future" });
        fs::write(dir.join("project.json"), meta.to_string()).unwrap();

        let result = load_project(&dir);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("newer than supported"),
            "Expected version error, got: {err}"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
