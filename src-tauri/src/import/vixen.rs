use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::color::Color;
use crate::model::color_gradient::{ColorGradient, ColorStop};
use crate::model::curve::{Curve, CurvePoint};
use crate::model::fixture::{
    ColorModel, Controller, ControllerId, ControllerProtocol, EffectTarget, FixtureDef,
    FixtureGroup, FixtureId, GroupId, GroupMember,
};
use crate::model::show::{FixtureLayout, Layout, Show};
use crate::model::timeline::{
    BlendMode, EffectInstance, EffectKind, EffectParams, ParamValue, Sequence, TimeRange, Track,
};

use super::vixen_preview;

// ── Error type ──────────────────────────────────────────────────────

#[derive(Debug)]
pub enum ImportError {
    Io(std::io::Error),
    Xml(quick_xml::Error),
    Parse(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::Io(e) => write!(f, "I/O error: {e}"),
            ImportError::Xml(e) => write!(f, "XML error: {e}"),
            ImportError::Parse(msg) => write!(f, "Parse error: {msg}"),
        }
    }
}

impl From<std::io::Error> for ImportError {
    fn from(e: std::io::Error) -> Self {
        ImportError::Io(e)
    }
}

impl From<quick_xml::Error> for ImportError {
    fn from(e: quick_xml::Error) -> Self {
        ImportError::Xml(e)
    }
}

// ── Wizard types ────────────────────────────────────────────────────

/// Discovery result from scanning a Vixen directory.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct VixenDiscovery {
    pub vixen_dir: String,
    pub fixtures_found: usize,
    pub groups_found: usize,
    pub controllers_found: usize,
    pub preview_available: bool,
    pub preview_item_count: usize,
    /// Path to the file containing preview data (if found).
    pub preview_file_path: Option<String>,
    pub sequences: Vec<VixenSequenceInfo>,
    pub media_files: Vec<VixenMediaInfo>,
}

/// Info about a discovered Vixen sequence file.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct VixenSequenceInfo {
    pub filename: String,
    pub path: String,
    #[ts(type = "number")]
    pub size_bytes: u64,
}

/// Info about a discovered Vixen media file.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct VixenMediaInfo {
    pub filename: String,
    pub path: String,
    #[ts(type = "number")]
    pub size_bytes: u64,
}

/// What the user selected for import (sent from frontend).
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct VixenImportConfig {
    pub vixen_dir: String,
    pub profile_name: String,
    pub import_controllers: bool,
    pub import_layout: bool,
    /// Optional user-provided path to the file containing preview/layout data.
    /// When set, overrides auto-detection in `find_preview_file`.
    pub preview_file_override: Option<String>,
    pub sequence_paths: Vec<String>,
    pub media_filenames: Vec<String>,
}

/// Result returned after full import.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct VixenImportResult {
    pub profile_slug: String,
    pub fixtures_imported: usize,
    pub groups_imported: usize,
    pub controllers_imported: usize,
    pub layout_items_imported: usize,
    pub sequences_imported: usize,
    pub media_imported: usize,
    pub warnings: Vec<String>,
}

// ── ISO 8601 duration parser ────────────────────────────────────────

/// Parse ISO 8601 duration strings like `PT1M53.606S`, `P0DT0H5M30.500S`, etc.
/// Returns duration in seconds.
pub fn parse_iso_duration(s: &str) -> Option<f64> {
    let s = s.trim();
    if !s.starts_with('P') {
        return None;
    }

    let s = &s[1..]; // Strip 'P'
    let mut total_seconds = 0.0;
    let mut current_num = String::new();
    let mut in_time_part = false;

    for ch in s.chars() {
        match ch {
            'T' => {
                in_time_part = true;
                current_num.clear();
            }
            'D' => {
                let days: f64 = current_num.parse().ok()?;
                total_seconds += days * 86400.0;
                current_num.clear();
            }
            'H' if in_time_part => {
                let hours: f64 = current_num.parse().ok()?;
                total_seconds += hours * 3600.0;
                current_num.clear();
            }
            'M' if in_time_part => {
                let minutes: f64 = current_num.parse().ok()?;
                total_seconds += minutes * 60.0;
                current_num.clear();
            }
            'S' if in_time_part => {
                let secs: f64 = current_num.parse().ok()?;
                total_seconds += secs;
                current_num.clear();
            }
            _ => {
                current_num.push(ch);
            }
        }
    }

    Some(total_seconds)
}

// ── CIE XYZ → sRGB conversion ──────────────────────────────────────

/// Convert CIE XYZ (D65, 0-100 scale) to sRGB Color.
pub fn xyz_to_srgb(x: f64, y: f64, z: f64) -> Color {
    // Normalize from 0-100 to 0-1
    let x = x / 100.0;
    let y = y / 100.0;
    let z = z / 100.0;

    // XYZ to linear sRGB (D65 reference, sRGB primaries)
    let r_lin = x * 3.2404542 + y * -1.5371385 + z * -0.4985314;
    let g_lin = x * -0.9692660 + y * 1.8760108 + z * 0.0415560;
    let b_lin = x * 0.0556434 + y * -0.2040259 + z * 1.0572252;

    // Apply sRGB gamma
    fn gamma(c: f64) -> f64 {
        let c = c.clamp(0.0, 1.0);
        if c <= 0.0031308 {
            c * 12.92
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        }
    }

    Color::rgb(
        (gamma(r_lin) * 255.0).round() as u8,
        (gamma(g_lin) * 255.0).round() as u8,
        (gamma(b_lin) * 255.0).round() as u8,
    )
}

// ── Effect type mapping ─────────────────────────────────────────────

/// Build a Curve ParamValue from Vixen curve points (0-100 scale → 0-1 normalized).
fn build_curve_param(points: &[(f64, f64)]) -> Option<ParamValue> {
    if points.len() < 2 {
        return None;
    }
    let curve_points: Vec<CurvePoint> = points
        .iter()
        .map(|(x, y)| CurvePoint {
            x: x / 100.0,
            y: y / 100.0,
        })
        .collect();
    Curve::new(curve_points).map(ParamValue::Curve)
}

/// Build a ColorGradient ParamValue from Vixen gradient stops (positions 0-1).
fn build_gradient_param(stops: &[(f64, Color)]) -> Option<ParamValue> {
    if stops.is_empty() {
        return None;
    }
    let color_stops: Vec<ColorStop> = stops
        .iter()
        .map(|(pos, color)| ColorStop {
            position: *pos,
            color: *color,
        })
        .collect();
    ColorGradient::new(color_stops).map(ParamValue::ColorGradient)
}

/// Map Vixen color handling string to our color_mode param value.
fn map_color_handling(handling: Option<&str>) -> &'static str {
    match handling {
        Some("GradientThroughWholeEffect") => "gradient_through_effect",
        Some("GradientAcrossItems") | Some("ColorAcrossItems") => "gradient_across_items",
        Some("GradientForEachPulse") | Some("GradientOverEachPulse")
        | Some("GradientPerPulse") => "gradient_per_pulse",
        Some("StaticColor") => "static",
        _ => "static",
    }
}

/// Map a Vixen effect type name to a VibeLights EffectKind + default params.
fn map_vixen_effect(effect: &VixenEffect) -> (EffectKind, EffectParams) {
    let type_name = effect.type_name.as_str();
    let color = effect.color;
    let movement_curve = effect.movement_curve.as_ref();
    let pulse_curve = effect.pulse_curve.as_ref();
    let intensity_curve = effect.intensity_curve.as_ref();
    let gradient_colors = effect.gradient_colors.as_ref();
    let color_handling = effect.color_handling.as_deref();
    let level = effect.level;
    let base_color = color.unwrap_or(Color::WHITE);

    match type_name {
        "Pulse" | "SetLevel" => {
            let mut params = EffectParams::new();
            // Pulse maps to our Fade effect with the parsed intensity curve.
            // SetLevel uses a constant intensity at the given level.
            if let Some(pts) = intensity_curve {
                if let Some(curve_val) = build_curve_param(pts) {
                    params = params.set("intensity_curve", curve_val);
                }
            } else {
                // No curve data: use constant intensity at the given level (default 1.0 = full on)
                let intensity = level.unwrap_or(1.0).clamp(0.0, 1.0);
                params = params.set(
                    "intensity_curve",
                    ParamValue::Curve(Curve::constant(intensity)),
                );
            }
            // Build gradient from gradient data or single color
            if let Some(stops) = gradient_colors {
                if let Some(grad_val) = build_gradient_param(stops) {
                    params = params.set("gradient", grad_val);
                }
            } else {
                params = params.set(
                    "gradient",
                    ParamValue::ColorGradient(ColorGradient::solid(base_color)),
                );
            }
            let color_mode = map_color_handling(color_handling);
            params = params.set("color_mode", ParamValue::Text(color_mode.into()));
            (EffectKind::Fade, params)
        }
        "Chase" => {
            let mut params = EffectParams::new();
            // Build gradient
            if let Some(stops) = gradient_colors {
                if let Some(grad_val) = build_gradient_param(stops) {
                    params = params.set("gradient", grad_val);
                }
            } else {
                params = params.set(
                    "gradient",
                    ParamValue::ColorGradient(ColorGradient::solid(base_color)),
                );
            }
            // Movement curve (ChaseMovement — head position over time)
            if let Some(pts) = movement_curve {
                if let Some(curve_val) = build_curve_param(pts) {
                    params = params.set("movement_curve", curve_val);
                }
            }
            // Pulse curve (PulseCurve — intensity envelope per pulse)
            if let Some(pts) = pulse_curve {
                if let Some(curve_val) = build_curve_param(pts) {
                    params = params.set("pulse_curve", curve_val);
                }
            }
            let color_mode = map_color_handling(color_handling);
            params = params
                .set("color_mode", ParamValue::Text(color_mode.into()))
                .set("speed", ParamValue::Float(1.0))
                .set("pulse_width", ParamValue::Float(0.3));
            (EffectKind::Chase, params)
        }
        "ColorWash" => (
            EffectKind::Gradient,
            EffectParams::new().set(
                "colors",
                ParamValue::ColorList(vec![base_color, Color::BLACK]),
            ),
        ),
        "Twinkle" => (
            EffectKind::Twinkle,
            EffectParams::new()
                .set("color", ParamValue::Color(base_color))
                .set("density", ParamValue::Float(0.4))
                .set("speed", ParamValue::Float(6.0)),
        ),
        "Strobe" => (
            EffectKind::Strobe,
            EffectParams::new()
                .set("color", ParamValue::Color(base_color))
                .set("rate", ParamValue::Float(10.0))
                .set("duty_cycle", ParamValue::Float(0.5)),
        ),
        "Alternating" => (
            EffectKind::Chase,
            EffectParams::new()
                .set(
                    "gradient",
                    ParamValue::ColorGradient(ColorGradient::solid(base_color)),
                )
                .set("speed", ParamValue::Float(1.0))
                .set("pulse_width", ParamValue::Float(0.5)),
        ),
        "PinWheel" => (
            EffectKind::Rainbow,
            EffectParams::new()
                .set("speed", ParamValue::Float(1.5))
                .set("spread", ParamValue::Float(1.0)),
        ),
        "Spin" => {
            let mut params = EffectParams::new();
            // Build gradient
            if let Some(stops) = gradient_colors {
                if let Some(grad_val) = build_gradient_param(stops) {
                    params = params.set("gradient", grad_val);
                }
            } else {
                params = params.set(
                    "gradient",
                    ParamValue::ColorGradient(ColorGradient::solid(base_color)),
                );
            }
            // Pulse curve (PulseCurve — same as Chase)
            if let Some(pts) = pulse_curve {
                if let Some(curve_val) = build_curve_param(pts) {
                    params = params.set("pulse_curve", curve_val);
                }
            }
            // Speed: RevolutionCount = number of passes over the effect's duration
            let speed = effect.revolution_count.unwrap_or(4.0);
            // Pulse width: PulsePercentage is 0-100, convert to 0-1 fraction
            let pulse_width = effect.pulse_percentage.map_or(0.1, |p| (p / 100.0).clamp(0.01, 1.0));
            let reverse = effect.reverse_spin.unwrap_or(false);
            let color_mode = map_color_handling(color_handling);
            params = params
                .set("color_mode", ParamValue::Text(color_mode.into()))
                .set("speed", ParamValue::Float(speed))
                .set("pulse_width", ParamValue::Float(pulse_width))
                .set("reverse", ParamValue::Bool(reverse));
            (EffectKind::Chase, params)
        }
        "Wipe" => (
            EffectKind::Chase,
            EffectParams::new()
                .set(
                    "gradient",
                    ParamValue::ColorGradient(ColorGradient::solid(base_color)),
                )
                .set("speed", ParamValue::Float(2.0))
                .set("pulse_width", ParamValue::Float(0.6)),
        ),
        "Rainbow" => (
            EffectKind::Rainbow,
            EffectParams::new()
                .set("speed", ParamValue::Float(1.0))
                .set("spread", ParamValue::Float(2.0)),
        ),
        _ => (
            EffectKind::Solid,
            EffectParams::new().set("color", ParamValue::Color(Color::rgb(128, 128, 128))),
        ),
    }
}

// ── Internal intermediate types ─────────────────────────────────────

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct VixenNode {
    name: String,
    guid: String,
    children_guids: Vec<String>,
    channel_id: Option<String>,
}

#[derive(Debug, Clone)]
struct VixenEffect {
    type_name: String,
    start_time: f64,
    duration: f64,
    target_node_guids: Vec<String>,
    color: Option<Color>,
    /// ChaseMovement curve (position over time for Chase effects)
    movement_curve: Option<Vec<(f64, f64)>>,
    /// PulseCurve (intensity envelope per pulse for Chase/Spin effects)
    pulse_curve: Option<Vec<(f64, f64)>>,
    /// LevelCurve / IntensityCurve (brightness envelope for Pulse/SetLevel)
    intensity_curve: Option<Vec<(f64, f64)>>,
    gradient_colors: Option<Vec<(f64, Color)>>,
    color_handling: Option<String>,
    level: Option<f64>,
    /// Spin-specific: number of revolutions over the effect duration
    revolution_count: Option<f64>,
    /// Spin-specific: pulse width as percentage of revolution (0-100)
    pulse_percentage: Option<f64>,
    /// Spin-specific: pulse time in milliseconds (used when PulseLengthFormat=FixedTime)
    pulse_time_ms: Option<f64>,
    /// Spin-specific: whether the spin direction is reversed
    reverse_spin: Option<bool>,
}

// ── VixenImporter ───────────────────────────────────────────────────

pub struct VixenImporter {
    nodes: HashMap<String, VixenNode>,
    guid_to_id: HashMap<String, u32>,
    next_id: u32,
    fixtures: Vec<FixtureDef>,
    groups: Vec<FixtureGroup>,
    controllers: Vec<Controller>,
    patches: Vec<crate::model::fixture::Patch>,
    sequences: Vec<Sequence>,
    /// IDs of fixtures that were created by merging leaf channels (e.g. RGB leaves → multi-pixel fixture).
    /// These should NOT be re-merged by a parent node.
    merged_fixture_ids: HashSet<u32>,
    /// Warnings accumulated during import (orphan targets, unsupported shapes, etc.).
    warnings: Vec<String>,
}

impl Default for VixenImporter {
    fn default() -> Self {
        Self::new()
    }
}

impl VixenImporter {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            guid_to_id: HashMap::new(),
            next_id: 0,
            fixtures: Vec::new(),
            groups: Vec::new(),
            controllers: Vec::new(),
            patches: Vec::new(),
            sequences: Vec::new(),
            merged_fixture_ids: HashSet::new(),
            warnings: Vec::new(),
        }
    }

    /// Reconstruct importer state from an existing profile + saved GUID mapping.
    /// This allows importing sequences against a previously-imported profile.
    pub fn from_profile(
        fixtures: Vec<FixtureDef>,
        groups: Vec<FixtureGroup>,
        controllers: Vec<Controller>,
        patches: Vec<crate::model::fixture::Patch>,
        guid_map: HashMap<String, u32>,
    ) -> Self {
        let next_id = guid_map.values().copied().max().map(|m| m + 1).unwrap_or(0);
        Self {
            nodes: HashMap::new(),
            guid_to_id: guid_map,
            next_id,
            fixtures,
            groups,
            controllers,
            patches,
            sequences: Vec::new(),
            merged_fixture_ids: HashSet::new(),
            warnings: Vec::new(),
        }
    }

    /// Return the GUID → ID mapping (for persisting after profile import).
    pub fn guid_map(&self) -> &HashMap<String, u32> {
        &self.guid_to_id
    }

    /// Return warnings accumulated during import.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Count of parsed fixtures.
    pub fn fixture_count(&self) -> usize {
        self.fixtures.len()
    }

    /// Count of parsed groups.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Count of parsed controllers.
    pub fn controller_count(&self) -> usize {
        self.controllers.len()
    }

    /// Parse Vixen preview layout data and produce FixtureLayout entries.
    pub fn parse_preview(
        &mut self,
        vixen_dir: &Path,
        preview_file_override: Option<&Path>,
    ) -> Result<Vec<FixtureLayout>, ImportError> {
        let preview_path = if let Some(override_path) = preview_file_override {
            override_path.to_path_buf()
        } else {
            vixen_preview::find_preview_file(vixen_dir).ok_or_else(|| {
                ImportError::Parse("No preview data file found".into())
            })?
        };

        let preview_data = vixen_preview::parse_preview_file(&preview_path)?;

        // Build pixel count map from current fixtures
        let pixel_counts: HashMap<u32, u32> = self
            .fixtures
            .iter()
            .map(|f| (f.id.0, f.pixel_count))
            .collect();

        let layouts = vixen_preview::build_fixture_layouts(
            &preview_data,
            &self.guid_to_id,
            &pixel_counts,
            &mut self.warnings,
        );

        Ok(layouts)
    }

    fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Parse SystemConfig.xml to extract fixtures, groups, and controllers.
    pub fn parse_system_config(&mut self, path: &Path) -> Result<(), ImportError> {
        let file = File::open(path)?;
        let reader = BufReader::with_capacity(64 * 1024, file);
        let mut xml = Reader::from_reader(reader);
        xml.config_mut().trim_text(true);

        let mut buf = Vec::with_capacity(4096);

        // First pass: collect all nodes
        self.parse_nodes(&mut xml, &mut buf)?;

        // Rewind and parse controllers
        let file2 = File::open(path)?;
        let reader2 = BufReader::with_capacity(64 * 1024, file2);
        let mut xml2 = Reader::from_reader(reader2);
        xml2.config_mut().trim_text(true);
        buf.clear();
        self.parse_controllers(&mut xml2, &mut buf)?;

        // Build fixtures and groups from nodes
        self.build_fixtures_and_groups();

        Ok(())
    }

    fn parse_nodes(
        &mut self,
        xml: &mut Reader<BufReader<File>>,
        buf: &mut Vec<u8>,
    ) -> Result<(), ImportError> {
        // Vixen 3 SystemConfig stores nodes as nested XML:
        //   <Nodes>
        //     <Node name="Group" id="GUID-1">
        //       <Node name="Child" id="GUID-2" channelId="CH-GUID">
        //         <Properties>...</Properties>
        //       </Node>
        //     </Node>
        //   </Nodes>
        //
        // Parent-child relationships are implicit via nesting.
        // We use a stack to track the current hierarchy.

        let mut in_nodes_section = false;
        let mut node_stack: Vec<VixenNode> = Vec::new();

        loop {
            match xml.read_event_into(buf) {
                Ok(Event::Eof) => break,
                Ok(Event::Start(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    match name.as_str() {
                        "Nodes" | "SystemNodes" => {
                            in_nodes_section = true;
                        }
                        "Node" | "ElementNode" | "ChannelNode" if in_nodes_section => {
                            let mut node_id = String::new();
                            let mut node_name = String::new();
                            let mut channel_id = None;

                            for attr in e.attributes().flatten() {
                                let key =
                                    String::from_utf8_lossy(attr.key.as_ref()).to_string();
                                let val =
                                    String::from_utf8_lossy(&attr.value).to_string();
                                match key.as_str() {
                                    "id" | "Id" => node_id = val,
                                    "name" | "Name" => node_name = val,
                                    "channelId" | "ChannelId" => channel_id = Some(val),
                                    _ => {}
                                }
                            }

                            node_stack.push(VixenNode {
                                name: node_name,
                                guid: node_id,
                                children_guids: Vec::new(),
                                channel_id,
                            });
                        }
                        _ => {}
                    }
                }
                Ok(Event::End(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    match name.as_str() {
                        "Node" | "ElementNode" | "ChannelNode" if !node_stack.is_empty() => {
                            let node = node_stack.pop().unwrap();
                            if !node.guid.is_empty() {
                                let guid = node.guid.clone();
                                self.nodes.insert(guid.clone(), node);

                                // Register as child of parent node (if any)
                                if let Some(parent) = node_stack.last_mut() {
                                    parent.children_guids.push(guid);
                                }
                            }
                        }
                        "Nodes" | "SystemNodes" => {
                            in_nodes_section = false;
                        }
                        _ => {}
                    }
                }
                Ok(Event::Empty(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    // Self-closing leaf node: <Node name="..." id="..." channelId="..." />
                    if (name == "Node" || name == "ElementNode" || name == "ChannelNode")
                        && in_nodes_section
                    {
                        let mut node_id = String::new();
                        let mut node_name = String::new();
                        let mut channel_id = None;

                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            match key.as_str() {
                                "id" | "Id" => node_id = val,
                                "name" | "Name" => node_name = val,
                                "channelId" | "ChannelId" => channel_id = Some(val),
                                _ => {}
                            }
                        }

                        if !node_id.is_empty() {
                            let guid = node_id.clone();
                            self.nodes.insert(
                                guid.clone(),
                                VixenNode {
                                    name: node_name,
                                    guid: node_id,
                                    children_guids: Vec::new(),
                                    channel_id,
                                },
                            );

                            // Register as child of parent node
                            if let Some(parent) = node_stack.last_mut() {
                                parent.children_guids.push(guid);
                            }
                        }
                    }
                }
                Err(e) => return Err(ImportError::Xml(e)),
                _ => {}
            }
            buf.clear();
        }

        Ok(())
    }

    fn parse_controllers(
        &mut self,
        xml: &mut Reader<BufReader<File>>,
        buf: &mut Vec<u8>,
    ) -> Result<(), ImportError> {
        let mut in_controllers = false;
        let mut current_name = String::new();
        let mut current_outputs: Vec<(String, u16)> = Vec::new(); // (ip, universe)
        let mut depth = 0u32;
        let mut controller_id_counter = 0u32;

        loop {
            match xml.read_event_into(buf) {
                Ok(Event::Eof) => break,
                Ok(Event::Start(ref e)) => {
                    depth += 1;
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    if name == "Controllers" || name == "OutputControllers" {
                        in_controllers = true;
                    }

                    if in_controllers
                        && (name == "Controller"
                            || name == "OutputController"
                            || name.contains("Controller"))
                    {
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            if key == "name" || key == "Name" {
                                current_name = val;
                            }
                        }
                    }

                    // Look for universe/IP configuration in output elements
                    if in_controllers {
                        let mut ip = None;
                        let mut universe = None;
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            match key.as_str() {
                                "ip" | "IP" | "address" | "Address" | "UnicastAddress" => {
                                    ip = Some(val)
                                }
                                "universe" | "Universe" => {
                                    universe = val.parse().ok()
                                }
                                _ => {}
                            }
                        }
                        if let (Some(ip_addr), Some(uni)) = (ip, universe) {
                            current_outputs.push((ip_addr, uni));
                        }
                    }
                }
                Ok(Event::End(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    depth = depth.saturating_sub(1);

                    if name == "Controllers" || name == "OutputControllers" {
                        in_controllers = false;
                    }

                    if in_controllers
                        && (name == "Controller"
                            || name == "OutputController"
                            || name.contains("Controller"))
                        && !current_name.is_empty()
                    {
                        // Create a controller for each unique IP/universe combo
                        // If no outputs found, create a generic E1.31 controller
                        if current_outputs.is_empty() {
                            self.controllers.push(Controller {
                                id: ControllerId(controller_id_counter),
                                name: current_name.clone(),
                                protocol: ControllerProtocol::E131 {
                                    unicast_address: None,
                                },
                            });
                            controller_id_counter += 1;
                        } else {
                            for (ip, _universe) in &current_outputs {
                                self.controllers.push(Controller {
                                    id: ControllerId(controller_id_counter),
                                    name: format!("{} ({})", current_name, ip),
                                    protocol: ControllerProtocol::E131 {
                                        unicast_address: Some(ip.clone()),
                                    },
                                });
                                controller_id_counter += 1;
                            }
                        }
                        current_name.clear();
                        current_outputs.clear();
                    }
                }
                Err(e) => return Err(ImportError::Xml(e)),
                _ => {}
            }
            buf.clear();
        }

        Ok(())
    }

    fn build_fixtures_and_groups(&mut self) {
        // Find root nodes (nodes not referenced as children by any other node)
        let all_child_guids: std::collections::HashSet<&str> = self
            .nodes
            .values()
            .flat_map(|n| n.children_guids.iter().map(|s| s.as_str()))
            .collect();

        let root_guids: Vec<String> = self
            .nodes
            .keys()
            .filter(|guid| !all_child_guids.contains(guid.as_str()))
            .cloned()
            .collect();

        // Assign IDs and build fixtures/groups
        for guid in &root_guids {
            self.build_node(guid);
        }
    }

    /// Recursively build a fixture or group from a Vixen node GUID.
    /// Returns either a FixtureId or GroupId if successfully created.
    fn build_node(&mut self, guid: &str) -> Option<GroupMember> {
        // Already processed?
        if let Some(&id) = self.guid_to_id.get(guid) {
            let node = self.nodes.get(guid)?;
            if node.children_guids.is_empty() {
                return Some(GroupMember::Fixture(FixtureId(id)));
            } else {
                return Some(GroupMember::Group(GroupId(id)));
            }
        }

        let node = self.nodes.get(guid)?.clone();
        let id = self.alloc_id();
        self.guid_to_id.insert(guid.to_string(), id);

        if node.children_guids.is_empty() {
            // Leaf node → fixture
            self.fixtures.push(FixtureDef {
                id: FixtureId(id),
                name: node.name.clone(),
                color_model: ColorModel::Rgb,
                pixel_count: 1,
                pixel_type: Default::default(),
                bulb_shape: Default::default(),
                display_radius_override: None,
                channel_order: Default::default(),
            });
            Some(GroupMember::Fixture(FixtureId(id)))
        } else {
            // Interior node → group
            // First, recursively build all children
            let mut members = Vec::new();
            for child_guid in &node.children_guids {
                if let Some(member) = self.build_node(child_guid) {
                    members.push(member);
                }
            }

            // Check if all children are *original leaf* fixtures (not already-merged multi-pixel ones).
            // Only merge leaves into a multi-pixel fixture; if any child is already merged,
            // create a group instead to preserve the hierarchy.
            let all_original_leaves = members.iter().all(|m| match m {
                GroupMember::Fixture(fid) => !self.merged_fixture_ids.contains(&fid.0),
                _ => false,
            });
            let child_count = members.len();

            if all_original_leaves && child_count > 1 {
                // Merge: remove individual leaf fixtures, create one multi-pixel fixture.
                // In Vixen, each leaf element node is one pixel (an RGB fixture).
                // The channelId on the leaf references the output channel.
                let fixture_ids: Vec<FixtureId> = members
                    .iter()
                    .filter_map(|m| match m {
                        GroupMember::Fixture(fid) => Some(*fid),
                        _ => None,
                    })
                    .collect();

                // Remove individual fixtures
                self.fixtures.retain(|f| !fixture_ids.contains(&f.id));

                // Create one multi-pixel fixture for this group of leaves
                let pixel_count = child_count as u32;
                self.fixtures.push(FixtureDef {
                    id: FixtureId(id),
                    name: node.name.clone(),
                    color_model: ColorModel::Rgb,
                    pixel_count,
                    pixel_type: Default::default(),
                    bulb_shape: Default::default(),
                    display_radius_override: None,
                    channel_order: Default::default(),
                });

                // Record this as a merged fixture so parent nodes don't re-merge it
                self.merged_fixture_ids.insert(id);

                // Remap child leaf GUIDs to point to the parent fixture ID.
                // This is critical for layout resolution: preview pixels reference
                // leaf node GUIDs, which must resolve to the merged parent fixture.
                for child_guid in &node.children_guids {
                    self.guid_to_id.insert(child_guid.clone(), id);
                }

                Some(GroupMember::Fixture(FixtureId(id)))
            } else if !members.is_empty() {
                self.groups.push(FixtureGroup {
                    id: GroupId(id),
                    name: node.name.clone(),
                    members,
                });
                Some(GroupMember::Group(GroupId(id)))
            } else {
                None
            }
        }
    }

    /// Parse a .tim sequence file.
    pub fn parse_sequence(&mut self, path: &Path) -> Result<(), ImportError> {
        let file = File::open(path)?;
        let reader = BufReader::with_capacity(64 * 1024, file);
        let mut xml = Reader::from_reader(reader);
        xml.config_mut().trim_text(true);

        let mut buf = Vec::with_capacity(4096);

        let seq_name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".to_string());

        let mut duration = 30.0f64;
        let mut effects: Vec<VixenEffect> = Vec::new();
        let mut audio_file: Option<String> = None;

        // Parsing state
        let mut current_element = String::new();
        let mut in_data_models = false;
        let mut in_effect_nodes = false;
        let mut in_media = false;

        // Current effect being parsed
        let mut effect_type = String::new();
        let mut effect_start = 0.0f64;
        let mut effect_duration = 0.0f64;
        let mut effect_targets: Vec<String> = Vec::new();
        let mut effect_color: Option<Color> = None;

        // For effect data models, we store type_name keyed by ModuleInstanceId
        let mut data_model_types: HashMap<String, String> = HashMap::new();
        let mut data_model_colors: HashMap<String, Color> = HashMap::new();
        // Curve data keyed by ModuleInstanceId, separated by curve type
        let mut data_model_movement_curves: HashMap<String, Vec<(f64, f64)>> = HashMap::new();
        let mut data_model_pulse_curves: HashMap<String, Vec<(f64, f64)>> = HashMap::new();
        let mut data_model_intensity_curves: HashMap<String, Vec<(f64, f64)>> = HashMap::new();
        let mut data_model_gradients: HashMap<String, Vec<(f64, Color)>> = HashMap::new();
        let mut data_model_color_handling: HashMap<String, String> = HashMap::new();
        // Also map ModuleTypeId → type_name (class-level; many instances share one type)
        let mut module_type_to_name: HashMap<String, String> = HashMap::new();
        let mut current_data_model_id = String::new();
        let mut current_data_model_type_id = String::new();
        let mut in_data_model_entry = false;
        let mut data_model_depth = 0u32;
        // Temporary state for parsing curve points within a data model
        let mut current_curve_points: Vec<(f64, f64)> = Vec::new();
        let mut current_gradient_stops: Vec<(f64, Color)> = Vec::new();
        let mut in_curve_element = false;
        let mut current_curve_kind = String::new(); // "movement", "pulse", or "intensity"
        let mut in_gradient_element = false;
        let mut current_color_handling = String::new();
        // State for parsing PointPair child elements (X, Y are text, not attributes)
        let mut in_point_pair = false;
        let mut point_pair_x: Option<f64> = None;
        let mut point_pair_y: Option<f64> = None;
        // State for parsing ColorPoint child elements
        let mut in_color_point = false;
        let mut color_point_position: Option<f64> = None;
        // State for parsing _color child elements within ColorPoint (XYZ as child text)
        let mut in_gradient_color = false;
        let mut gradient_color_x: Option<f64> = None;
        let mut gradient_color_y: Option<f64> = None;
        let mut gradient_color_z: Option<f64> = None;
        // State for parsing SetLevel-style direct RGB color (_r/_g/_b 0-1 scale)
        let mut in_direct_color = false;
        let mut direct_color_r: Option<f64> = None;
        let mut direct_color_g: Option<f64> = None;
        let mut direct_color_b: Option<f64> = None;
        let mut data_model_levels: HashMap<String, f64> = HashMap::new();
        // Spin-specific data keyed by ModuleInstanceId
        let mut data_model_revolution_count: HashMap<String, f64> = HashMap::new();
        let mut data_model_pulse_percentage: HashMap<String, f64> = HashMap::new();
        let mut data_model_pulse_time_ms: HashMap<String, f64> = HashMap::new();
        let mut data_model_reverse_spin: HashMap<String, bool> = HashMap::new();

        // For effect node surrogates
        let mut in_effect_node_entry = false;
        let mut current_module_id = String::new();
        let mut current_effect_instance_id = String::new();
        let mut effect_node_depth = 0u32;

        let mut depth = 0u32;

        loop {
            match xml.read_event_into(&mut buf) {
                Ok(Event::Eof) => break,
                Ok(Event::Start(ref e)) => {
                    depth += 1;
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    current_element = tag.clone();

                    match tag.as_str() {
                        "_dataModels" | "DataModels" => {
                            in_data_models = true;
                        }
                        "_effectNodeSurrogates" | "EffectNodeSurrogates" => {
                            in_effect_nodes = true;
                        }
                        "_mediaSurrogates" | "MediaSurrogates" => {
                            in_media = true;
                        }
                        _ => {}
                    }

                    // Inside _dataModels, each entry is a <d1p1:anyType> wrapper
                    // with i:type attribute containing the effect type.
                    // Also handle older formats with explicit DataModel tags.
                    if in_data_models && !in_data_model_entry {
                        // Match the wrapper element: anyType, or tags with "DataModel" in the name
                        let local_tag = tag.rsplit(':').next().unwrap_or(&tag);
                        if local_tag == "anyType" || tag.contains("DataModel") {
                            in_data_model_entry = true;
                            data_model_depth = depth;
                            current_data_model_id.clear();
                            current_data_model_type_id.clear();
                            effect_type.clear();
                            for attr in e.attributes().flatten() {
                                let key =
                                    String::from_utf8_lossy(attr.key.as_ref()).to_string();
                                let val =
                                    String::from_utf8_lossy(&attr.value).to_string();
                                let local_key = key.rsplit(':').next().unwrap_or(&key);
                                match local_key {
                                    "type" | "Type" | "typeName" => {
                                        // Extract class name: "d2p1:PinWheelData" → "PinWheel"
                                        let raw = val.rsplit(':').next().unwrap_or(&val);
                                        let cleaned = raw
                                            .rsplit('.')
                                            .next()
                                            .unwrap_or(raw);
                                        let cleaned = cleaned
                                            .strip_suffix("Module")
                                            .or_else(|| cleaned.strip_suffix("Data"))
                                            .unwrap_or(cleaned);
                                        effect_type = cleaned.to_string();
                                    }
                                    "id" | "Id" => {
                                        current_data_model_id = val;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }

                    // Inside _effectNodeSurrogates, each effect is an <EffectNodeSurrogate>.
                    // Must NOT match child <ChannelNodeReferenceSurrogate> tags.
                    if in_effect_nodes
                        && !in_effect_node_entry
                        && tag == "EffectNodeSurrogate"
                    {
                        in_effect_node_entry = true;
                        effect_node_depth = depth;
                        effect_start = 0.0;
                        effect_duration = 0.0;
                        effect_targets.clear();
                        effect_color = None;
                        current_module_id.clear();
                        current_effect_instance_id.clear();

                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            match key.as_str() {
                                "startTime" | "StartTime" => {
                                    effect_start = parse_iso_duration(&val).unwrap_or(0.0);
                                }
                                "timeSpan" | "TimeSpan" | "duration" | "Duration" => {
                                    effect_duration = parse_iso_duration(&val).unwrap_or(0.0);
                                }
                                "typeId" | "TypeId" => {
                                    current_module_id = val;
                                }
                                "moduleInstanceId" | "ModuleInstanceId"
                                | "instanceId" | "InstanceId" => {
                                    current_effect_instance_id = val;
                                }
                                _ => {}
                            }
                        }
                    }

                    // Detect curve/gradient container elements within data model entries.
                    // Vixen XML uses namespace prefixes (d2p1:, d3p1:, etc.) — match local name.
                    if in_data_model_entry {
                        let local_tag = tag.rsplit(':').next().unwrap_or(&tag);
                        match local_tag {
                            "ChaseMovement" => {
                                in_curve_element = true;
                                current_curve_kind = "movement".to_string();
                                current_curve_points.clear();
                            }
                            "PulseCurve" => {
                                in_curve_element = true;
                                current_curve_kind = "pulse".to_string();
                                current_curve_points.clear();
                            }
                            "LevelCurve" | "IntensityCurve" | "Curve" => {
                                in_curve_element = true;
                                current_curve_kind = "intensity".to_string();
                                current_curve_points.clear();
                            }
                            "ColorGradient" => {
                                in_gradient_element = true;
                                current_gradient_stops.clear();
                            }
                            "_colors" | "Colors" if in_gradient_element => {
                                // nested container within ColorGradient; already tracking
                            }
                            _ => {}
                        }

                        // PointPair: X and Y are child text elements, not attributes
                        if in_curve_element && local_tag == "PointPair" {
                            in_point_pair = true;
                            point_pair_x = None;
                            point_pair_y = None;
                        }

                        // ColorPoint: _position and _color are child elements
                        if in_gradient_element && local_tag == "ColorPoint" {
                            in_color_point = true;
                            color_point_position = None;
                        }

                        // _color inside ColorPoint: XYZ stored as child text elements _x, _y, _z
                        if in_color_point && local_tag == "_color" {
                            in_gradient_color = true;
                            gradient_color_x = None;
                            gradient_color_y = None;
                            gradient_color_z = None;
                        }

                        // SetLevel-style direct RGB color: <color> with _r/_g/_b children (0-1 scale)
                        if local_tag == "color" && !in_gradient_element && !in_color_point {
                            in_direct_color = true;
                            direct_color_r = None;
                            direct_color_g = None;
                            direct_color_b = None;
                        }
                    }

                    // Look for XYZ color values as attributes (older/alternate formats)
                    if (in_data_model_entry || in_effect_node_entry) && tag == "Color" {
                        let mut x = None;
                        let mut y = None;
                        let mut z = None;
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            match key.as_str() {
                                "X" | "x" => x = val.parse().ok(),
                                "Y" | "y" => y = val.parse().ok(),
                                "Z" | "z" => z = val.parse().ok(),
                                _ => {}
                            }
                        }
                        if let (Some(x), Some(y), Some(z)) = (x, y, z) {
                            let color = xyz_to_srgb(x, y, z);
                            if in_data_model_entry && !current_data_model_id.is_empty() {
                                data_model_colors
                                    .insert(current_data_model_id.clone(), color);
                            }
                            effect_color = Some(color);
                        }
                    }
                }
                Ok(Event::Text(ref e)) => {
                    let text = e.unescape().unwrap_or_default().trim().to_string();
                    if text.is_empty() {
                        // skip
                    } else if in_data_model_entry && current_element == "ModuleInstanceId" {
                        // Capture the data model's ID from child element text
                        current_data_model_id = text;
                    } else if in_data_model_entry && current_element == "ModuleTypeId" {
                        current_data_model_type_id = text;
                    } else if in_data_model_entry
                        && (current_element == "ColorHandling"
                            || current_element == "ColorMode"
                            || current_element == "_colorHandling")
                    {
                        current_color_handling = text;
                    } else if in_point_pair {
                        // PointPair X/Y are child text elements: <X>0</X> <Y>100</Y>
                        let local_el = current_element.rsplit(':').next().unwrap_or(&current_element);
                        match local_el {
                            "X" => point_pair_x = text.parse().ok(),
                            "Y" => point_pair_y = text.parse().ok(),
                            _ => {}
                        }
                    } else if in_gradient_color {
                        // Gradient color XYZ as child text: <_x>95.047</_x> <_y>100</_y> <_z>108.883</_z>
                        let local_el = current_element.rsplit(':').next().unwrap_or(&current_element);
                        match local_el {
                            "_x" | "X" => gradient_color_x = text.parse().ok(),
                            "_y" | "Y" => gradient_color_y = text.parse().ok(),
                            "_z" | "Z" => gradient_color_z = text.parse().ok(),
                            _ => {}
                        }
                    } else if in_color_point {
                        // ColorPoint position as child text: <_position>0</_position>
                        let local_el = current_element.rsplit(':').next().unwrap_or(&current_element);
                        if local_el == "_position" || local_el == "Position" {
                            color_point_position = text.parse().ok();
                        }
                    } else if in_direct_color {
                        // SetLevel direct RGB: <_r>1</_r> <_g>0</_g> <_b>0</_b> (0-1 scale)
                        let local_el = current_element.rsplit(':').next().unwrap_or(&current_element);
                        match local_el {
                            "_r" | "R" => direct_color_r = text.parse().ok(),
                            "_g" | "G" => direct_color_g = text.parse().ok(),
                            "_b" | "B" => direct_color_b = text.parse().ok(),
                            _ => {}
                        }
                    } else if in_data_model_entry {
                        let local_el = current_element.rsplit(':').next().unwrap_or(&current_element);
                        let id = &current_data_model_id;
                        if !id.is_empty() {
                            match local_el {
                                // SetLevel intensity level
                                "level" | "Level" | "IntensityLevel" => {
                                    if let Ok(v) = text.parse::<f64>() {
                                        data_model_levels.insert(id.clone(), v);
                                    }
                                }
                                // Spin: revolution count (= speed in passes)
                                "RevolutionCount" => {
                                    if let Ok(v) = text.parse::<f64>() {
                                        data_model_revolution_count.insert(id.clone(), v);
                                    }
                                }
                                // Spin: pulse width as percentage of revolution
                                "PulsePercentage" => {
                                    if let Ok(v) = text.parse::<f64>() {
                                        data_model_pulse_percentage.insert(id.clone(), v);
                                    }
                                }
                                // Spin: pulse time in ms (when PulseLengthFormat=FixedTime)
                                "PulseTime" => {
                                    if let Ok(v) = text.parse::<f64>() {
                                        data_model_pulse_time_ms.insert(id.clone(), v);
                                    }
                                }
                                // Spin: reverse direction
                                "ReverseSpin" => {
                                    if let Ok(v) = text.parse::<bool>() {
                                        data_model_reverse_spin.insert(id.clone(), v);
                                    }
                                }
                                _ => {}
                            }
                        }
                    } else if current_element == "Length" {
                        if let Some(dur) = parse_iso_duration(&text) {
                            duration = dur;
                        }
                    } else if in_effect_node_entry {
                        // Inside an EffectNodeSurrogate: capture timing, targets, type
                        match current_element.as_str() {
                            "StartTime" | "startTime" => {
                                if let Some(t) = parse_iso_duration(&text) {
                                    effect_start = t;
                                }
                            }
                            "TimeSpan" | "timeSpan" | "Duration" => {
                                if let Some(d) = parse_iso_duration(&text) {
                                    effect_duration = d;
                                }
                            }
                            "NodeId" => {
                                // NodeId inside ChannelNodeReferenceSurrogate → target GUID
                                let guid = text.trim();
                                if !guid.is_empty() {
                                    effect_targets.push(guid.to_string());
                                }
                            }
                            "TargetNodeId" | "TargetNodes" => {
                                // Semicolon-separated GUID list (older format)
                                for guid in text.split(';') {
                                    let guid = guid.trim();
                                    if !guid.is_empty() {
                                        effect_targets.push(guid.to_string());
                                    }
                                }
                            }
                            "TypeId" | "typeId" => {
                                // TypeId is the module class ID (e.g., "Chase type GUID")
                                current_module_id = text;
                            }
                            "InstanceId" => {
                                // InstanceId links to a specific ModuleInstanceId in _dataModels
                                current_effect_instance_id = text;
                            }
                            _ => {}
                        }
                    } else if in_media
                        && (current_element == "FilePath"
                            || current_element == "FileName"
                            || current_element == "MediaFilePath"
                            || current_element.ends_with(":FilePath")
                            || current_element.ends_with(":FileName")
                            || current_element.ends_with(":RelativeAudioPath"))
                    {
                        audio_file = Some(text);
                    }
                }
                Ok(Event::End(ref e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    depth = depth.saturating_sub(1);

                    match tag.as_str() {
                        "_dataModels" | "DataModels" => {
                            in_data_models = false;
                        }
                        "_effectNodeSurrogates" | "EffectNodeSurrogates" => {
                            in_effect_nodes = false;
                        }
                        "_mediaSurrogates" | "MediaSurrogates" => {
                            in_media = false;
                        }
                        _ => {}
                    }

                    // End of curve/gradient child elements — use local tag name
                    if in_data_model_entry {
                        let local_tag = tag.rsplit(':').next().unwrap_or(&tag);

                        // End of PointPair → finalize point
                        if in_point_pair && local_tag == "PointPair" {
                            if let (Some(x), Some(y)) = (point_pair_x, point_pair_y) {
                                current_curve_points.push((x, y));
                            }
                            in_point_pair = false;
                        }

                        // End of _color inside ColorPoint → convert XYZ to sRGB
                        if in_gradient_color && local_tag == "_color" {
                            if let (Some(x), Some(y), Some(z)) =
                                (gradient_color_x, gradient_color_y, gradient_color_z)
                            {
                                let color = xyz_to_srgb(x, y, z);
                                // Also set as the data model color and effect color
                                if !current_data_model_id.is_empty() {
                                    data_model_colors
                                        .insert(current_data_model_id.clone(), color);
                                }
                                effect_color = Some(color);
                                // Store for the current ColorPoint (resolved when ColorPoint closes)
                                gradient_color_x = None; // reuse fields to pass color
                                gradient_color_y = None;
                                gradient_color_z = None;
                                // Tag the last gradient stop with this color
                                // (ColorPoint may not be finalized yet, so we update the pending stop)
                                if in_color_point {
                                    // We'll add the stop when ColorPoint closes
                                    // For now store the color in effect_color
                                }
                            }
                            in_gradient_color = false;
                        }

                        // End of ColorPoint → finalize gradient stop
                        if in_color_point && local_tag == "ColorPoint" {
                            let pos = color_point_position.unwrap_or(0.0);
                            let color = effect_color.unwrap_or(Color::WHITE);
                            current_gradient_stops.push((pos, color));
                            in_color_point = false;
                            color_point_position = None;
                        }

                        // End of direct color element (SetLevel _r/_g/_b)
                        if in_direct_color && local_tag == "color" {
                            if let (Some(r), Some(g), Some(b)) =
                                (direct_color_r, direct_color_g, direct_color_b)
                            {
                                let color = Color::rgb(
                                    (r.clamp(0.0, 1.0) * 255.0) as u8,
                                    (g.clamp(0.0, 1.0) * 255.0) as u8,
                                    (b.clamp(0.0, 1.0) * 255.0) as u8,
                                );
                                if !current_data_model_id.is_empty() {
                                    data_model_colors
                                        .insert(current_data_model_id.clone(), color);
                                }
                                effect_color = Some(color);
                            }
                            in_direct_color = false;
                        }

                        // End of curve container
                        match local_tag {
                            "ChaseMovement" | "PulseCurve" | "LevelCurve"
                            | "IntensityCurve" | "Curve"
                                if in_curve_element =>
                            {
                                if !current_curve_points.is_empty()
                                    && !current_data_model_id.is_empty()
                                {
                                    let target_map = match current_curve_kind.as_str() {
                                        "movement" => &mut data_model_movement_curves,
                                        "pulse" => &mut data_model_pulse_curves,
                                        _ => &mut data_model_intensity_curves,
                                    };
                                    target_map.insert(
                                        current_data_model_id.clone(),
                                        current_curve_points.clone(),
                                    );
                                }
                                in_curve_element = false;
                                current_curve_kind.clear();
                            }
                            "ColorGradient" if in_gradient_element => {
                                if !current_gradient_stops.is_empty()
                                    && !current_data_model_id.is_empty()
                                {
                                    data_model_gradients.insert(
                                        current_data_model_id.clone(),
                                        current_gradient_stops.clone(),
                                    );
                                }
                                in_gradient_element = false;
                            }
                            _ => {}
                        }
                    }

                    // End of a data model entry
                    if in_data_model_entry && depth < data_model_depth {
                        if !current_data_model_id.is_empty() && !effect_type.is_empty() {
                            data_model_types.insert(
                                current_data_model_id.clone(),
                                effect_type.clone(),
                            );
                        }
                        // Store color handling
                        if !current_data_model_id.is_empty()
                            && !current_color_handling.is_empty()
                        {
                            data_model_color_handling.insert(
                                current_data_model_id.clone(),
                                current_color_handling.clone(),
                            );
                        }
                        // Also map ModuleTypeId → data (class-level lookup).
                        // Many effects share one ModuleTypeId but have different instances.
                        if !current_data_model_type_id.is_empty() {
                            if !effect_type.is_empty() {
                                module_type_to_name
                                    .entry(current_data_model_type_id.clone())
                                    .or_insert_with(|| effect_type.clone());
                            }
                            // Store curve/gradient/color handling under type ID too
                            for curve_map in [
                                &mut data_model_movement_curves,
                                &mut data_model_pulse_curves,
                                &mut data_model_intensity_curves,
                            ] {
                                let clone = curve_map
                                    .get(&current_data_model_id)
                                    .cloned();
                                if let Some(data) = clone {
                                    curve_map
                                        .entry(current_data_model_type_id.clone())
                                        .or_insert(data);
                                }
                            }
                            let grads_clone = data_model_gradients
                                .get(&current_data_model_id)
                                .cloned();
                            if let Some(grads) = grads_clone {
                                data_model_gradients
                                    .entry(current_data_model_type_id.clone())
                                    .or_insert(grads);
                            }
                            let ch_clone = data_model_color_handling
                                .get(&current_data_model_id)
                                .cloned();
                            if let Some(ch) = ch_clone {
                                data_model_color_handling
                                    .entry(current_data_model_type_id.clone())
                                    .or_insert(ch);
                            }
                        }
                        in_data_model_entry = false;
                        current_color_handling.clear();
                        current_curve_points.clear();
                        current_gradient_stops.clear();
                        in_curve_element = false;
                        in_gradient_element = false;
                    }

                    // End of an EffectNodeSurrogate → finalize the effect
                    if in_effect_node_entry && depth < effect_node_depth {
                        // Resolve effect type via three-level lookup:
                        // 1. InstanceId → data_model_types (instance-level, most specific)
                        // 2. TypeId → module_type_to_name (class-level, shared across instances)
                        // 3. Fallback to last parsed effect_type (unreliable)
                        let resolved_type = data_model_types
                            .get(&current_effect_instance_id)
                            .or_else(|| module_type_to_name.get(&current_module_id))
                            .cloned()
                            .unwrap_or_else(|| "Solid".to_string());

                        // Resolve color: instance-level first, then class-level
                        let resolved_color = effect_color
                            .or_else(|| data_model_colors.get(&current_effect_instance_id).copied())
                            .or_else(|| data_model_colors.get(&current_module_id).copied());

                        // Resolve curve/gradient data from data models.
                        // Instance-level first, then class-level (TypeId) fallback.
                        let resolved_movement = data_model_movement_curves
                            .get(&current_effect_instance_id)
                            .or_else(|| data_model_movement_curves.get(&current_module_id))
                            .cloned();
                        let resolved_pulse = data_model_pulse_curves
                            .get(&current_effect_instance_id)
                            .or_else(|| data_model_pulse_curves.get(&current_module_id))
                            .cloned();
                        let resolved_intensity = data_model_intensity_curves
                            .get(&current_effect_instance_id)
                            .or_else(|| data_model_intensity_curves.get(&current_module_id))
                            .cloned();
                        let resolved_gradients = data_model_gradients
                            .get(&current_effect_instance_id)
                            .or_else(|| data_model_gradients.get(&current_module_id))
                            .cloned();
                        let resolved_color_handling = data_model_color_handling
                            .get(&current_effect_instance_id)
                            .or_else(|| data_model_color_handling.get(&current_module_id))
                            .cloned();
                        let resolved_level = data_model_levels
                            .get(&current_effect_instance_id)
                            .or_else(|| data_model_levels.get(&current_module_id))
                            .copied();

                        // Spin-specific fields
                        let resolved_rev_count = data_model_revolution_count
                            .get(&current_effect_instance_id)
                            .or_else(|| data_model_revolution_count.get(&current_module_id))
                            .copied();
                        let resolved_pulse_pct = data_model_pulse_percentage
                            .get(&current_effect_instance_id)
                            .or_else(|| data_model_pulse_percentage.get(&current_module_id))
                            .copied();
                        let resolved_pulse_time = data_model_pulse_time_ms
                            .get(&current_effect_instance_id)
                            .or_else(|| data_model_pulse_time_ms.get(&current_module_id))
                            .copied();
                        let resolved_reverse = data_model_reverse_spin
                            .get(&current_effect_instance_id)
                            .or_else(|| data_model_reverse_spin.get(&current_module_id))
                            .copied();

                        if effect_duration > 0.0 {
                            effects.push(VixenEffect {
                                type_name: resolved_type,
                                start_time: effect_start,
                                duration: effect_duration,
                                target_node_guids: effect_targets.clone(),
                                color: resolved_color,
                                movement_curve: resolved_movement,
                                pulse_curve: resolved_pulse,
                                intensity_curve: resolved_intensity,
                                gradient_colors: resolved_gradients,
                                color_handling: resolved_color_handling,
                                level: resolved_level,
                                revolution_count: resolved_rev_count,
                                pulse_percentage: resolved_pulse_pct,
                                pulse_time_ms: resolved_pulse_time,
                                reverse_spin: resolved_reverse,
                            });
                        }

                        in_effect_node_entry = false;
                        effect_targets.clear();
                    }
                }
                Ok(Event::Empty(ref e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    let local_tag = tag.rsplit(':').next().unwrap_or(&tag);

                    // Handle self-closing Color elements with XYZ attributes
                    if (in_data_model_entry || in_effect_node_entry) && local_tag == "Color" {
                        let mut x = None;
                        let mut y = None;
                        let mut z = None;
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            match key.as_str() {
                                "X" | "x" => x = val.parse().ok(),
                                "Y" | "y" => y = val.parse().ok(),
                                "Z" | "z" => z = val.parse().ok(),
                                _ => {}
                            }
                        }
                        if let (Some(x), Some(y), Some(z)) = (x, y, z) {
                            let color = xyz_to_srgb(x, y, z);
                            if in_data_model_entry && !current_data_model_id.is_empty() {
                                data_model_colors
                                    .insert(current_data_model_id.clone(), color);
                            }
                            effect_color = Some(color);
                        }
                    }
                }
                Err(e) => return Err(ImportError::Xml(e)),
                _ => {}
            }
            buf.clear();
        }

        // Build tracks from effects, grouped by target node
        let tracks = self.build_tracks(effects);

        self.sequences.push(Sequence {
            name: seq_name,
            duration,
            frame_rate: 30.0,
            audio_file,
            tracks,
        });

        Ok(())
    }

    /// Merge adjacent effects that have the same type and color within a gap threshold.
    /// This collapses rapid-fire Vixen effects (e.g. 100 consecutive Pulse effects) into one.
    fn merge_adjacent_effects(effects: &mut Vec<VixenEffect>) {
        const GAP_THRESHOLD: f64 = 0.050; // 50ms

        if effects.len() < 2 {
            return;
        }

        // Sort by start time first
        effects.sort_by(|a, b| {
            a.start_time
                .partial_cmp(&b.start_time)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut merged = Vec::with_capacity(effects.len());
        let mut current = effects[0].clone();

        for next in effects.iter().skip(1) {
            let current_end = current.start_time + current.duration;
            let gap = next.start_time - current_end;
            let same_type = current.type_name == next.type_name;
            let same_color = current.color == next.color;

            if same_type && same_color && (-GAP_THRESHOLD..=GAP_THRESHOLD).contains(&gap) {
                // Merge: extend current to cover both
                let new_end = (next.start_time + next.duration).max(current_end);
                current.duration = new_end - current.start_time;
            } else {
                merged.push(current);
                current = next.clone();
            }
        }
        merged.push(current);

        *effects = merged;
    }

    /// Build tracks from parsed Vixen effects, grouped by target node.
    fn build_tracks(&self, effects: Vec<VixenEffect>) -> Vec<Track> {
        // Group effects by their primary target
        let mut effects_by_target: HashMap<String, Vec<VixenEffect>> = HashMap::new();

        for effect in effects {
            if effect.target_node_guids.is_empty() {
                effects_by_target
                    .entry("_all_".to_string())
                    .or_default()
                    .push(effect);
            } else {
                let target_guid = &effect.target_node_guids[0];
                // Skip orphan targets — if the GUID doesn't map to any known fixture/group, drop it
                if target_guid != "_all_" && !self.guid_to_id.contains_key(target_guid) {
                    continue;
                }
                effects_by_target
                    .entry(target_guid.clone())
                    .or_default()
                    .push(effect);
            }
        }

        let mut tracks = Vec::new();
        let mut total_effects = 0usize;
        const MAX_TOTAL_EFFECTS: usize = 10_000;

        for (target_guid, mut target_effects) in effects_by_target {
            // Merge adjacent same-type effects to reduce count
            Self::merge_adjacent_effects(&mut target_effects);

            // Sort by start time
            target_effects.sort_by(|a, b| {
                a.start_time
                    .partial_cmp(&b.start_time)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Assign effects to lanes (non-overlapping within each lane)
            let mut lanes: Vec<Vec<&VixenEffect>> = Vec::new();

            for effect in &target_effects {
                let mut assigned = false;

                for lane in &mut lanes {
                    let lane_end = lane
                        .last()
                        .map(|e| e.start_time + e.duration)
                        .unwrap_or(0.0);
                    if effect.start_time >= lane_end {
                        lane.push(effect);
                        assigned = true;
                        break;
                    }
                }

                if !assigned {
                    lanes.push(vec![effect]);
                }
            }

            // Resolve target
            let target = if target_guid == "_all_" {
                EffectTarget::All
            } else if let Some(&id) = self.guid_to_id.get(&target_guid) {
                // Check if it's a fixture or group
                if self.fixtures.iter().any(|f| f.id == FixtureId(id)) {
                    EffectTarget::Fixtures(vec![FixtureId(id)])
                } else if self.groups.iter().any(|g| g.id == GroupId(id)) {
                    EffectTarget::Group(GroupId(id))
                } else {
                    continue; // orphan — skip
                }
            } else {
                continue; // orphan — skip
            };

            let target_name = if target_guid == "_all_" {
                "All".to_string()
            } else {
                self.nodes
                    .get(&target_guid)
                    .map(|n| n.name.clone())
                    .unwrap_or_else(|| format!("Track {}", tracks.len() + 1))
            };

            // Create a track per lane
            for (lane_idx, lane) in lanes.iter().enumerate() {
                let lane_suffix = if lanes.len() > 1 {
                    format!(" ({})", lane_idx + 1)
                } else {
                    String::new()
                };

                let mut effect_instances: Vec<EffectInstance> = lane
                    .iter()
                    .filter_map(|e| {
                        let end = e.start_time + e.duration;
                        let time_range = TimeRange::new(e.start_time, end)?;
                        let (kind, params) = map_vixen_effect(e);
                        Some(EffectInstance {
                            kind,
                            params,
                            time_range,
                        })
                    })
                    .collect();

                // Sort effects by start time for efficient binary-search evaluation.
                effect_instances.sort_by(|a, b| {
                    a.time_range
                        .start()
                        .partial_cmp(&b.time_range.start())
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                if !effect_instances.is_empty() {
                    total_effects += effect_instances.len();
                    tracks.push(Track {
                        name: format!("{}{}", target_name, lane_suffix),
                        target: target.clone(),
                        blend_mode: BlendMode::Override,
                        effects: effect_instances,
                    });
                }
            }
        }

        // Cap total effects
        if total_effects > MAX_TOTAL_EFFECTS {
            eprintln!(
                "[VibeLights] Warning: {} effects exceed cap of {}. Truncating tracks.",
                total_effects, MAX_TOTAL_EFFECTS
            );
            let mut count = 0usize;
            for track in &mut tracks {
                let remaining = MAX_TOTAL_EFFECTS.saturating_sub(count);
                if remaining == 0 {
                    track.effects.clear();
                } else if track.effects.len() > remaining {
                    track.effects.truncate(remaining);
                }
                count += track.effects.len();
            }
            // Remove empty tracks
            tracks.retain(|t| !t.effects.is_empty());
        }

        tracks
    }

    /// Extract just the sequences (for sequence-only imports).
    pub fn into_sequences(self) -> Vec<Sequence> {
        self.sequences
    }

    /// Consume the importer and produce a Show.
    pub fn into_show(self) -> Show {
        Show {
            name: "Vixen Import".into(),
            fixtures: self.fixtures,
            groups: self.groups,
            layout: Layout {
                fixtures: Vec::new(), // Layout will need to be created separately
            },
            sequences: self.sequences,
            patches: self.patches,
            controllers: self.controllers,
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_iso_duration_simple() {
        assert!((parse_iso_duration("PT1M53.606S").unwrap() - 113.606).abs() < 0.001);
        assert!((parse_iso_duration("PT5M30.500S").unwrap() - 330.5).abs() < 0.001);
        assert!((parse_iso_duration("PT30S").unwrap() - 30.0).abs() < 0.001);
        assert!((parse_iso_duration("PT1H").unwrap() - 3600.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_iso_duration_with_days() {
        assert!((parse_iso_duration("P0DT0H5M30.500S").unwrap() - 330.5).abs() < 0.001);
        assert!((parse_iso_duration("P1DT0H0M0S").unwrap() - 86400.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_iso_duration_edge_cases() {
        assert!((parse_iso_duration("PT0S").unwrap() - 0.0).abs() < 0.001);
        assert!((parse_iso_duration("PT0.001S").unwrap() - 0.001).abs() < 0.0001);
        assert!(parse_iso_duration("not a duration").is_none());
        assert!(parse_iso_duration("").is_none());
    }

    #[test]
    fn test_xyz_to_srgb_white() {
        let white = xyz_to_srgb(95.047, 100.0, 108.883);
        // Should be close to (255, 255, 255)
        assert!(white.r >= 254, "r={}", white.r);
        assert!(white.g >= 254, "g={}", white.g);
        assert!(white.b >= 254, "b={}", white.b);
    }

    #[test]
    fn test_xyz_to_srgb_black() {
        let black = xyz_to_srgb(0.0, 0.0, 0.0);
        assert_eq!(black.r, 0);
        assert_eq!(black.g, 0);
        assert_eq!(black.b, 0);
    }

    #[test]
    fn test_xyz_to_srgb_red() {
        // sRGB red (255,0,0) in XYZ is approximately (41.24, 21.26, 1.93)
        let red = xyz_to_srgb(41.24, 21.26, 1.93);
        assert!(red.r > 240, "r={}", red.r);
        assert!(red.g < 15, "g={}", red.g);
        assert!(red.b < 15, "b={}", red.b);
    }

    fn test_effect(type_name: &str) -> VixenEffect {
        VixenEffect {
            type_name: type_name.to_string(),
            start_time: 0.0,
            duration: 5.0,
            target_node_guids: Vec::new(),
            color: None,
            movement_curve: None,
            pulse_curve: None,
            intensity_curve: None,
            gradient_colors: None,
            color_handling: None,
            level: None,
            revolution_count: None,
            pulse_percentage: None,
            pulse_time_ms: None,
            reverse_spin: None,
        }
    }

    #[test]
    fn test_effect_mapping() {
        let (kind, _) = map_vixen_effect(&test_effect("Pulse"));
        assert!(matches!(kind, EffectKind::Fade));

        let (kind, _) = map_vixen_effect(&test_effect("SetLevel"));
        assert!(matches!(kind, EffectKind::Fade));

        let mut chase = test_effect("Chase");
        chase.color = Some(Color::rgb(255, 0, 0));
        let (kind, _) = map_vixen_effect(&chase);
        assert!(matches!(kind, EffectKind::Chase));

        let (kind, _) = map_vixen_effect(&test_effect("ColorWash"));
        assert!(matches!(kind, EffectKind::Gradient));

        let (kind, _) = map_vixen_effect(&test_effect("Twinkle"));
        assert!(matches!(kind, EffectKind::Twinkle));

        let (kind, _) = map_vixen_effect(&test_effect("Strobe"));
        assert!(matches!(kind, EffectKind::Strobe));

        let (kind, _) = map_vixen_effect(&test_effect("PinWheel"));
        assert!(matches!(kind, EffectKind::Rainbow));

        let (kind, _) = map_vixen_effect(&test_effect("Alternating"));
        assert!(matches!(kind, EffectKind::Chase));

        let (kind, _) = map_vixen_effect(&test_effect("Wipe"));
        assert!(matches!(kind, EffectKind::Chase));

        let (kind, _) = map_vixen_effect(&test_effect("Spin"));
        assert!(matches!(kind, EffectKind::Chase));

        let (kind, _) = map_vixen_effect(&test_effect("Rainbow"));
        assert!(matches!(kind, EffectKind::Rainbow));

        // Unknown effect falls back to Solid
        let (kind, _) = map_vixen_effect(&test_effect("SomeUnknownEffect"));
        assert!(matches!(kind, EffectKind::Solid));
    }

    /// Verify that when leaf nodes are merged into a multi-pixel parent fixture,
    /// the guid_to_id entries for leaf GUIDs are remapped to the parent fixture ID.
    /// This is critical for layout resolution: preview pixels reference leaf node
    /// GUIDs and must resolve to the merged parent fixture.
    #[test]
    fn test_merged_leaf_guid_remapping() {
        let mut importer = VixenImporter::new();

        // Create a parent node with 3 leaf children (simulating an RGB fixture).
        let leaf_guids: Vec<String> = (0..3).map(|i| format!("leaf-{i}")).collect();
        let parent_guid = "parent".to_string();

        for (i, guid) in leaf_guids.iter().enumerate() {
            importer.nodes.insert(
                guid.clone(),
                VixenNode {
                    name: format!("Leaf {i}"),
                    guid: guid.clone(),
                    children_guids: vec![],
                    channel_id: Some(format!("ch-{i}")),
                },
            );
        }

        importer.nodes.insert(
            parent_guid.clone(),
            VixenNode {
                name: "Parent Fixture".to_string(),
                guid: parent_guid.clone(),
                children_guids: leaf_guids.clone(),
                channel_id: None,
            },
        );

        // Build the parent node (which will recursively build leaves, then merge them).
        let result = importer.build_node(&parent_guid);
        assert!(result.is_some());

        // After merging, there should be exactly one fixture (the merged parent).
        assert_eq!(importer.fixtures.len(), 1, "Expected 1 merged fixture");
        let parent_fixture = &importer.fixtures[0];
        assert_eq!(parent_fixture.pixel_count, 3);
        let parent_id = parent_fixture.id.0;

        // Critical: all leaf GUIDs must now resolve to the parent fixture ID.
        for leaf_guid in &leaf_guids {
            let mapped_id = importer.guid_to_id.get(leaf_guid).copied();
            assert_eq!(
                mapped_id,
                Some(parent_id),
                "Leaf GUID '{}' should map to parent ID {}, got {:?}",
                leaf_guid,
                parent_id,
                mapped_id
            );
        }

        // The parent GUID should also map to the parent ID.
        assert_eq!(
            importer.guid_to_id.get(&parent_guid).copied(),
            Some(parent_id)
        );
    }

    /// Integration test: parse real Vixen SystemConfig to verify fixture merging.
    /// The real config has ~25K leaf channels that should be merged into ~hundreds of fixtures.
    #[test]
    fn test_real_vixen_fixture_merging() {
        let config_path = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3\SystemData\SystemConfig.xml",
        );
        if !config_path.exists() {
            eprintln!("Skipping: real SystemConfig.xml not found");
            return;
        }

        let mut importer = VixenImporter::new();
        importer.parse_system_config(config_path).unwrap();

        let fixture_count = importer.fixture_count();
        let group_count = importer.group_count();

        eprintln!(
            "Real Vixen import: {} fixtures, {} groups",
            fixture_count, group_count
        );

        // With correct merging, 25K channels should collapse to far fewer fixtures.
        // The exact number depends on the hierarchy, but it should be well under 1000.
        assert!(
            fixture_count < 1000,
            "Expected fewer than 1000 fixtures after merging, got {}",
            fixture_count
        );
        // Should have at least some groups
        assert!(
            group_count > 0,
            "Expected some groups from the Vixen hierarchy"
        );

        // Print some fixture names for manual verification
        let show = importer.into_show();
        let single_pixel = show.fixtures.iter().filter(|f| f.pixel_count == 1).count();
        let multi_pixel = show.fixtures.iter().filter(|f| f.pixel_count > 1).count();
        eprintln!("  Single-pixel fixtures: {single_pixel}");
        eprintln!("  Multi-pixel fixtures: {multi_pixel}");
        for f in show.fixtures.iter().take(20) {
            eprintln!("  Fixture: {} (id={}, pixels={})", f.name, f.id.0, f.pixel_count);
        }
        for g in show.groups.iter().take(10) {
            eprintln!("  Group: {} (id={}, members={})", g.name, g.id.0, g.members.len());
        }
    }

    /// Integration test: parse a real .tim sequence to verify curve and gradient extraction.
    #[test]
    fn test_real_vixen_sequence_curves_and_gradients() {
        let config_path = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3\SystemData\SystemConfig.xml",
        );
        let seq_path = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3\Sequence\60-bpm-metronome.tim",
        );
        if !config_path.exists() || !seq_path.exists() {
            eprintln!("Skipping: real Vixen files not found");
            return;
        }

        let mut importer = VixenImporter::new();
        importer.parse_system_config(config_path).unwrap();
        importer.parse_sequence(seq_path).unwrap();

        let sequences = importer.into_sequences();
        assert!(!sequences.is_empty(), "Should have at least one sequence");
        let seq = &sequences[0];

        let mut fade_count = 0usize;
        let mut chase_count = 0usize;
        let mut has_gradient = 0usize;
        let mut has_curve = 0usize;
        let mut non_white_gradient = 0usize;

        for track in &seq.tracks {
            for effect in &track.effects {
                match effect.kind {
                    EffectKind::Fade => {
                        fade_count += 1;
                        if effect.params.get("gradient").is_some() {
                            has_gradient += 1;
                        }
                        if effect.params.get("intensity_curve").is_some() {
                            has_curve += 1;
                        }
                        // Check if gradient has non-white color
                        if let Some(ParamValue::ColorGradient(g)) = effect.params.get("gradient") {
                            let c = g.evaluate(0.0);
                            if c.r != 255 || c.g != 255 || c.b != 255 {
                                non_white_gradient += 1;
                            }
                        }
                    }
                    EffectKind::Chase => {
                        chase_count += 1;
                    }
                    _ => {}
                }
            }
        }

        eprintln!("Sequence: {} tracks", seq.tracks.len());
        eprintln!("  Fade effects: {fade_count}");
        eprintln!("    with gradient param: {has_gradient}");
        eprintln!("    with intensity curve param: {has_curve}");
        eprintln!("    with non-white gradient: {non_white_gradient}");
        eprintln!("  Chase effects: {chase_count}");

        // We should find at least some Fade effects (from Pulse) with extracted data
        assert!(
            fade_count > 0,
            "Expected at least one Fade effect from Pulse mapping"
        );
    }

    #[test]
    fn test_real_vixen_chase_curves() {
        let config_path = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3\SystemData\SystemConfig.xml",
        );
        // Use a file known to have Chase effects
        let seq_dir = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3\Sequence",
        );
        if !config_path.exists() || !seq_dir.exists() {
            eprintln!("Skipping: real Vixen files not found");
            return;
        }

        let mut importer = VixenImporter::new();
        importer.parse_system_config(config_path).unwrap();

        // Try AllOn 2016.tim which is known to have Chase effects
        let seq_path = seq_dir.join("AllOn 2016.tim");
        if !seq_path.exists() {
            // Fall back to scanning for any file with Chase effects
            eprintln!("AllOn 2016.tim not found, scanning for Chase effects...");
            for entry in std::fs::read_dir(seq_dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "tim") {
                    let content = std::fs::read_to_string(&path).unwrap_or_default();
                    if content.contains("ChaseMovement") {
                        eprintln!("Found Chase in: {}", path.display());
                        let mut imp2 = VixenImporter::new();
                        imp2.parse_system_config(config_path).unwrap();
                        imp2.parse_sequence(&path).unwrap();
                        let seqs = imp2.into_sequences();
                        for seq in &seqs {
                            for track in &seq.tracks {
                                for effect in &track.effects {
                                    if matches!(effect.kind, EffectKind::Chase) {
                                        let has_move = effect.params.get("movement_curve").is_some();
                                        let has_pulse = effect.params.get("pulse_curve").is_some();
                                        eprintln!(
                                            "  Chase effect: movement_curve={}, pulse_curve={}",
                                            has_move, has_pulse
                                        );
                                    }
                                }
                            }
                        }
                        return;
                    }
                }
            }
            eprintln!("No files with ChaseMovement found");
            return;
        }

        importer.parse_sequence(&seq_path).unwrap();
        let sequences = importer.into_sequences();

        let mut chase_count = 0usize;
        let mut chase_with_movement = 0usize;
        let mut chase_with_pulse = 0usize;

        for seq in &sequences {
            for track in &seq.tracks {
                for effect in &track.effects {
                    if matches!(effect.kind, EffectKind::Chase) {
                        chase_count += 1;
                        if effect.params.get("movement_curve").is_some() {
                            chase_with_movement += 1;
                        }
                        if effect.params.get("pulse_curve").is_some() {
                            chase_with_pulse += 1;
                        }
                    }
                }
            }
        }

        eprintln!("Chase effects: {chase_count}");
        eprintln!("  with movement_curve: {chase_with_movement}");
        eprintln!("  with pulse_curve: {chase_with_pulse}");

        if chase_count > 0 {
            assert!(
                chase_with_movement > 0,
                "Expected at least one Chase with movement_curve (found {chase_count} chases)"
            );
        }
    }
}
