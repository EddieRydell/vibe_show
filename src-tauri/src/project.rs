use std::fmt;
use std::fs;
use std::path::Path;

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

pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), ProjectError> {
    let json = serde_json::to_string_pretty(value)?;
    fs::write(path, json)?;
    Ok(())
}

pub(crate) fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, ProjectError> {
    let data = fs::read_to_string(path)?;
    let value = serde_json::from_str(&data)?;
    Ok(value)
}

// ── Save / Load ─────────────────────────────────────────────────────

/// Save a Show to a .vibeshow project directory.
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

/// Load a Show from a .vibeshow project directory.
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
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
            .collect();
        entries.sort_by_key(|e| e.file_name());

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
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Demo Sequence"), "demo-sequence");
        assert_eq!(slugify("Jing Jing Jing!"), "jing-jing-jing");
        assert_eq!(slugify("   "), "untitled");
        assert_eq!(slugify("hello---world"), "hello-world");
        assert_eq!(slugify("Test_Name-123"), "test_name-123");
    }
}
