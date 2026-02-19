use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// Newtype for fixture identity. Prevents mixing up fixture IDs with other integers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FixtureId(pub u32);

/// Newtype for group identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GroupId(pub u32);

/// Newtype for controller identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ControllerId(pub u32);

// ── Color & Channel Models ──────────────────────────────────────────

/// How a fixture's channels map to color data.
/// Extensible to cover all common LED and conventional fixture types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorModel {
    /// Single intensity channel (dimmers, single-color LEDs).
    Single,
    /// 3 channels for color. Order specified by `ChannelOrder`.
    Rgb,
    /// 4 channels: RGB + dedicated white.
    Rgbw,
}

impl ColorModel {
    /// Number of DMX channels consumed per pixel for this color model.
    pub const fn channels_per_pixel(self) -> u16 {
        match self {
            ColorModel::Single => 1,
            ColorModel::Rgb => 3,
            ColorModel::Rgbw => 4,
        }
    }
}

/// Channel byte ordering within a pixel. Different protocols/chips use different orders.
/// WS2811 defaults to GRB, WS2812 uses GRB, SK6812 uses GRBW, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ChannelOrder {
    #[default]
    Rgb,
    Grb,
    Brg,
    Rbg,
    Gbr,
    Bgr,
}

// ── DMX Addressing ──────────────────────────────────────────────────

/// DMX universe number (0-indexed internally, shown as 1-indexed to users).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Universe(pub u16);

/// DMX channel address within a universe. Valid range: 1..=512.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DmxAddress(u16);

impl DmxAddress {
    /// Create a DMX address. Returns None if out of valid range (1-512).
    pub fn new(addr: u16) -> Option<Self> {
        if (1..=512).contains(&addr) {
            Some(Self(addr))
        } else {
            None
        }
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

// ── Output / Patching ───────────────────────────────────────────────

/// How a fixture's pixel data gets mapped to a physical output.
/// This is the "patch" - it connects logical fixtures to physical channels.
/// Kept as a separate concern from the fixture itself so the same fixture
/// definition can be re-patched to different controllers/universes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patch {
    pub fixture_id: FixtureId,
    pub output: OutputMapping,
}

/// Where a fixture's channel data is sent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputMapping {
    /// Standard DMX (E1.31/sACN, ArtNet, or serial DMX).
    Dmx {
        universe: Universe,
        start_address: DmxAddress,
        channel_order: ChannelOrder,
    },
    /// Future: direct pixel protocol output (e.g. WS2811 via a pixel controller).
    /// The controller handles the protocol; we just need to know which output port.
    PixelPort {
        controller_id: ControllerId,
        port: u16,
        channel_order: ChannelOrder,
    },
}

// ── Controller ──────────────────────────────────────────────────────

/// How a controller communicates with the sequencer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControllerProtocol {
    /// E1.31 (Streaming ACN) over network.
    E131 { unicast_address: Option<String> },
    /// ArtNet over network.
    ArtNet { address: Option<String> },
    /// Serial (USB) for direct pixel output.
    Serial { port: String, baud_rate: u32 },
}

/// A physical controller that drives one or more outputs.
/// Examples: Falcon F16V4, ESPixelStick, Kulp K32, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Controller {
    pub id: ControllerId,
    pub name: String,
    pub protocol: ControllerProtocol,
}

// ── Fixtures ────────────────────────────────────────────────────────

/// A fixture definition. Represents a logical light or string of lights.
/// This is purely about *what* the light is, not *how* it's connected.
/// Connection info lives in `Patch` and `Controller`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureDef {
    pub id: FixtureId,
    pub name: String,
    pub color_model: ColorModel,
    /// Number of individually addressable pixels. 1 for simple fixtures.
    pub pixel_count: u32,
}

impl FixtureDef {
    /// Total DMX channels this fixture consumes.
    pub fn total_channels(&self) -> u32 {
        self.pixel_count * self.color_model.channels_per_pixel() as u32
    }
}

// ── Groups & Targeting ──────────────────────────────────────────────

/// A member of a group: either a direct fixture or a nested sub-group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GroupMember {
    Fixture(FixtureId),
    Group(GroupId),
}

/// A named group of fixtures for targeting effects. Supports hierarchical nesting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureGroup {
    pub id: GroupId,
    pub name: String,
    pub members: Vec<GroupMember>,
}

impl FixtureGroup {
    /// Recursively resolve all fixture IDs in this group, with cycle detection.
    pub fn resolve_fixture_ids(&self, all_groups: &[FixtureGroup]) -> Vec<FixtureId> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        visited.insert(self.id);
        Self::resolve_recursive(&self.members, all_groups, &mut visited, &mut result);
        result
    }

    fn resolve_recursive(
        members: &[GroupMember],
        all_groups: &[FixtureGroup],
        visited: &mut HashSet<GroupId>,
        result: &mut Vec<FixtureId>,
    ) {
        for member in members {
            match member {
                GroupMember::Fixture(id) => result.push(*id),
                GroupMember::Group(gid) => {
                    if visited.insert(*gid) {
                        if let Some(group) = all_groups.iter().find(|g| g.id == *gid) {
                            Self::resolve_recursive(
                                &group.members,
                                all_groups,
                                visited,
                                result,
                            );
                        }
                    }
                }
            }
        }
    }
}

/// What an effect targets: a specific set of fixtures or a named group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EffectTarget {
    Group(GroupId),
    Fixtures(Vec<FixtureId>),
    All,
}
