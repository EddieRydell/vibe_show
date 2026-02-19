use serde::{Deserialize, Serialize};

use super::fixture::{Controller, FixtureDef, FixtureGroup, FixtureId, Patch};
use super::timeline::Sequence;

/// 2D position for preview rendering. Coordinates are normalized (0.0 to 1.0).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position2D {
    pub x: f32,
    pub y: f32,
}

/// Maps a fixture's pixels to 2D positions for the preview renderer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureLayout {
    pub fixture_id: FixtureId,
    /// One position per pixel. Length must equal the fixture's pixel_count.
    pub pixel_positions: Vec<Position2D>,
}

/// The spatial layout of all fixtures for preview rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layout {
    pub fixtures: Vec<FixtureLayout>,
}

/// The top-level show model. Contains everything needed to describe and render a light show.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Show {
    pub name: String,
    pub fixtures: Vec<FixtureDef>,
    pub groups: Vec<FixtureGroup>,
    pub layout: Layout,
    pub sequences: Vec<Sequence>,
    /// Output patching: maps fixtures to physical controller outputs.
    pub patches: Vec<Patch>,
    /// Physical controllers in this show's setup.
    pub controllers: Vec<Controller>,
}

impl Show {
    /// Create an empty show with no fixtures, sequences, or controllers.
    pub fn empty() -> Self {
        Self {
            name: String::new(),
            fixtures: Vec::new(),
            groups: Vec::new(),
            layout: Layout {
                fixtures: Vec::new(),
            },
            sequences: Vec::new(),
            patches: Vec::new(),
            controllers: Vec::new(),
        }
    }
}
