use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::model::color::Color;
use crate::model::fixture::{
    ColorModel, Controller, ControllerId, ControllerProtocol, EffectTarget, FixtureDef,
    FixtureGroup, FixtureId, GroupId, GroupMember,
};
use crate::model::show::{Layout, Show};
use crate::model::timeline::{
    BlendMode, EffectInstance, EffectKind, EffectParams, ParamValue, Sequence, TimeRange, Track,
};

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

/// Map a Vixen effect type name to a VibeShow EffectKind + default params.
pub fn map_vixen_effect(type_name: &str, color: Option<Color>) -> (EffectKind, EffectParams) {
    let base_color = color.unwrap_or(Color::WHITE);
    let color_param = ParamValue::Color(base_color);

    match type_name {
        "Pulse" | "SetLevel" => (
            EffectKind::Solid,
            EffectParams::new().set("color", color_param),
        ),
        "Chase" => (
            EffectKind::Chase,
            EffectParams::new()
                .set("color", color_param)
                .set("speed", ParamValue::Float(2.0))
                .set("tail_length", ParamValue::Float(0.3)),
        ),
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
                .set("color", color_param)
                .set("density", ParamValue::Float(0.4))
                .set("speed", ParamValue::Float(6.0)),
        ),
        "Strobe" => (
            EffectKind::Strobe,
            EffectParams::new()
                .set("color", color_param)
                .set("rate", ParamValue::Float(10.0))
                .set("duty_cycle", ParamValue::Float(0.5)),
        ),
        "Alternating" => (
            EffectKind::Chase,
            EffectParams::new()
                .set("color", color_param)
                .set("speed", ParamValue::Float(1.0))
                .set("tail_length", ParamValue::Float(0.5)),
        ),
        "PinWheel" => (
            EffectKind::Rainbow,
            EffectParams::new()
                .set("speed", ParamValue::Float(1.5))
                .set("spread", ParamValue::Float(1.0)),
        ),
        "Wipe" | "Spin" => (
            EffectKind::Chase,
            EffectParams::new()
                .set("color", color_param)
                .set("speed", ParamValue::Float(2.0))
                .set("tail_length", ParamValue::Float(0.6)),
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
        }
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
        // Track depth and state for collecting node data.
        // Vixen SystemConfig stores nodes in nested XML, typically:
        // <Nodes> contains <Node> elements with id=GUID, name=..., children, and Properties
        // We look for patterns like:
        //   <Node> with attribute id="GUID" name="name"
        //     <Children> ... child node references </Children>
        //     <Properties> ... channel references </Properties>
        //   </Node>

        let mut in_nodes_section = false;
        let mut current_node: Option<VixenNode> = None;
        let mut in_children = false;
        let mut in_properties = false;
        let mut depth = 0u32;
        let mut collecting_text = false;
        let mut text_target: Option<String> = None;

        loop {
            match xml.read_event_into(buf) {
                Ok(Event::Eof) => break,
                Ok(Event::Start(ref e)) => {
                    depth += 1;
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    match name.as_str() {
                        "Nodes" | "SystemNodes" => {
                            in_nodes_section = true;
                        }
                        "Node" | "ElementNode" | "ChannelNode" => {
                            if in_nodes_section || depth <= 5 {
                                let mut node_id = String::new();
                                let mut node_name = String::new();

                                for attr in e.attributes().flatten() {
                                    let key =
                                        String::from_utf8_lossy(attr.key.as_ref()).to_string();
                                    let val =
                                        String::from_utf8_lossy(&attr.value).to_string();
                                    match key.as_str() {
                                        "id" | "Id" => node_id = val,
                                        "name" | "Name" => node_name = val,
                                        _ => {}
                                    }
                                }

                                if !node_id.is_empty() {
                                    current_node = Some(VixenNode {
                                        name: node_name,
                                        guid: node_id,
                                        children_guids: Vec::new(),
                                        channel_id: None,
                                    });
                                }
                            }
                        }
                        "Children" => {
                            if current_node.is_some() {
                                in_children = true;
                            }
                        }
                        "Properties" | "ChannelId" | "Property" => {
                            if current_node.is_some() {
                                in_properties = true;
                            }
                        }
                        "ChildId" | "NodeId" | "ElementId" => {
                            if in_children && current_node.is_some() {
                                collecting_text = true;
                                text_target = Some("child".to_string());
                            }
                        }
                        "ChannelReference" | "OutputChannel" => {
                            // Sometimes channel refs are in attributes
                            if let Some(ref mut node) = current_node {
                                for attr in e.attributes().flatten() {
                                    let key =
                                        String::from_utf8_lossy(attr.key.as_ref()).to_string();
                                    if key == "id" || key == "Id" || key == "channelId" {
                                        let val =
                                            String::from_utf8_lossy(&attr.value).to_string();
                                        node.channel_id = Some(val);
                                    }
                                }
                            }
                        }
                        _ => {
                            // Check for id/name attributes on generic elements within node context
                            if in_properties {
                                if let Some(ref mut node) = current_node {
                                    for attr in e.attributes().flatten() {
                                        let key = String::from_utf8_lossy(attr.key.as_ref())
                                            .to_string();
                                        if key == "channelId" || key == "ChannelId" {
                                            let val =
                                                String::from_utf8_lossy(&attr.value).to_string();
                                            node.channel_id = Some(val);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(Event::Text(ref e)) => {
                    if collecting_text {
                        let text = e.unescape().unwrap_or_default().trim().to_string();
                        if !text.is_empty() {
                            if let Some(ref target) = text_target {
                                if let Some(ref mut node) = current_node {
                                    match target.as_str() {
                                        "child" => node.children_guids.push(text),
                                        "channel" => node.channel_id = Some(text),
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                    // Also check for standalone text that might be a channel ID
                    if in_properties && !collecting_text {
                        if let Some(ref mut node) = current_node {
                            let text = e.unescape().unwrap_or_default().trim().to_string();
                            // GUIDs are typically 36 chars with hyphens
                            if text.len() == 36 && text.contains('-') && node.channel_id.is_none()
                            {
                                node.channel_id = Some(text);
                            }
                        }
                    }
                }
                Ok(Event::End(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    depth = depth.saturating_sub(1);

                    match name.as_str() {
                        "Node" | "ElementNode" | "ChannelNode" => {
                            if let Some(node) = current_node.take() {
                                if !node.guid.is_empty() {
                                    self.nodes.insert(node.guid.clone(), node);
                                }
                            }
                            in_children = false;
                            in_properties = false;
                        }
                        "Children" => {
                            in_children = false;
                        }
                        "Properties" | "ChannelId" | "Property" => {
                            in_properties = false;
                        }
                        "ChildId" | "NodeId" | "ElementId" => {
                            collecting_text = false;
                            text_target = None;
                        }
                        "Nodes" | "SystemNodes" => {
                            in_nodes_section = false;
                        }
                        _ => {}
                    }
                }
                Ok(Event::Empty(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    // Handle self-closing Node elements
                    if (name == "Node" || name == "ElementNode" || name == "ChannelNode")
                        && (in_nodes_section || depth <= 5)
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
                            self.nodes.insert(
                                node_id.clone(),
                                VixenNode {
                                    name: node_name,
                                    guid: node_id,
                                    children_guids: Vec::new(),
                                    channel_id,
                                },
                            );
                        }
                    }

                    // Handle self-closing child references
                    if (name == "ChildId" || name == "NodeId" || name == "ElementId")
                        && in_children
                    {
                        if let Some(ref mut node) = current_node {
                            for attr in e.attributes().flatten() {
                                let key =
                                    String::from_utf8_lossy(attr.key.as_ref()).to_string();
                                if key == "id" || key == "Id" || key == "value" {
                                    let val =
                                        String::from_utf8_lossy(&attr.value).to_string();
                                    node.children_guids.push(val);
                                }
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

            // Check if all children are leaf fixtures — if so, merge into one multi-pixel fixture
            let all_fixtures = members.iter().all(|m| matches!(m, GroupMember::Fixture(_)));
            let child_count = members.len();

            if all_fixtures && child_count > 1 {
                // Merge: remove individual fixtures, create one multi-pixel fixture
                let fixture_ids: Vec<FixtureId> = members
                    .iter()
                    .filter_map(|m| match m {
                        GroupMember::Fixture(fid) => Some(*fid),
                        _ => None,
                    })
                    .collect();

                // Remove individual fixtures
                self.fixtures.retain(|f| !fixture_ids.contains(&f.id));

                // Overwrite this node's ID as a fixture
                let pixel_count = child_count as u32;
                // Determine channels per pixel from child count heuristics
                // If the child names suggest RGB (3 children per pixel), adjust
                self.fixtures.push(FixtureDef {
                    id: FixtureId(id),
                    name: node.name.clone(),
                    color_model: ColorModel::Rgb,
                    pixel_count: pixel_count / 3, // Vixen typically has 3 channels per RGB pixel
                    pixel_type: Default::default(),
                    bulb_shape: Default::default(),
                    display_radius_override: None,
                    channel_order: Default::default(),
                });

                // Adjust: if pixel_count / 3 == 0, treat as single-channel fixtures
                if pixel_count / 3 == 0 {
                    if let Some(f) = self.fixtures.last_mut() {
                        f.pixel_count = pixel_count;
                        f.color_model = ColorModel::Single;
                    }
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
        let mut depth = 0u32;

        // For effect data models, we store type_name keyed by some ID
        let mut data_model_types: HashMap<String, String> = HashMap::new();
        let mut data_model_colors: HashMap<String, Color> = HashMap::new();
        let mut current_data_model_id = String::new();
        let mut in_data_model_entry = false;

        // For effect node surrogates
        let mut in_effect_node_entry = false;
        let mut current_module_id = String::new();

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

                    // Inside _dataModels, each entry has a type and possibly color data
                    if in_data_models
                        && (tag.contains("DataModel")
                            || tag.contains("Effect")
                            || tag.contains("Module"))
                    {
                        in_data_model_entry = true;
                        current_data_model_id.clear();
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            match key.as_str() {
                                "type" | "Type" | "typeName" => {
                                    // Extract just the class name from fully qualified name
                                    let type_name = val
                                        .rsplit('.')
                                        .next()
                                        .unwrap_or(&val)
                                        .to_string();
                                    // Remove "Module" or "Data" suffix
                                    let type_name = type_name
                                        .strip_suffix("Module")
                                        .or_else(|| type_name.strip_suffix("Data"))
                                        .unwrap_or(&type_name)
                                        .to_string();
                                    if !current_data_model_id.is_empty() {
                                        data_model_types.insert(
                                            current_data_model_id.clone(),
                                            type_name,
                                        );
                                    }
                                    effect_type = val
                                        .rsplit('.')
                                        .next()
                                        .unwrap_or(&val)
                                        .to_string();
                                    effect_type = effect_type
                                        .strip_suffix("Module")
                                        .or_else(|| effect_type.strip_suffix("Data"))
                                        .unwrap_or(&effect_type)
                                        .to_string();
                                }
                                "id" | "Id" | "ModuleInstanceId" => {
                                    current_data_model_id = val.clone();
                                }
                                _ => {}
                            }
                        }
                    }

                    // Inside _effectNodeSurrogates, entries have timing and target info
                    if in_effect_nodes
                        && (tag.contains("Surrogate")
                            || tag.contains("EffectNode")
                            || tag.contains("Effect"))
                    {
                        in_effect_node_entry = true;
                        effect_start = 0.0;
                        effect_duration = 0.0;
                        effect_targets.clear();
                        effect_color = None;
                        current_module_id.clear();

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
                                "typeId" | "TypeId" | "moduleInstanceId"
                                | "ModuleInstanceId" => {
                                    current_module_id = val;
                                }
                                _ => {}
                            }
                        }
                    }

                    // Look for XYZ color values
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
                    } else if current_element == "Length" || current_element == "Duration" {
                        if let Some(dur) = parse_iso_duration(&text) {
                            duration = dur;
                        }
                    } else if current_element == "StartTime" || current_element == "startTime" {
                        if in_effect_node_entry {
                            if let Some(t) = parse_iso_duration(&text) {
                                effect_start = t;
                            }
                        }
                    } else if current_element == "TimeSpan"
                        || current_element == "timeSpan"
                        || current_element == "Duration"
                    {
                        if in_effect_node_entry {
                            if let Some(d) = parse_iso_duration(&text) {
                                effect_duration = d;
                            }
                        }
                    } else if current_element == "TargetNodes"
                        || current_element == "TargetNodeId"
                        || current_element == "NodeId"
                    {
                        if in_effect_node_entry {
                            // May be a semicolon-separated list of GUIDs
                            for guid in text.split(';') {
                                let guid = guid.trim();
                                if !guid.is_empty() {
                                    effect_targets.push(guid.to_string());
                                }
                            }
                        }
                    } else if (current_element == "TypeId"
                        || current_element == "ModuleInstanceId"
                        || current_element == "typeId")
                        && in_effect_node_entry
                    {
                        current_module_id = text;
                    } else if (current_element == "FilePath"
                        || current_element == "MediaFilePath")
                        && in_media
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

                    if in_data_model_entry
                        && (tag.contains("DataModel")
                            || tag.contains("Effect")
                            || tag.contains("Module"))
                    {
                        // Store the type for this data model
                        if !current_data_model_id.is_empty() && !effect_type.is_empty() {
                            data_model_types.insert(
                                current_data_model_id.clone(),
                                effect_type.clone(),
                            );
                        }
                        in_data_model_entry = false;
                    }

                    if in_effect_node_entry
                        && (tag.contains("Surrogate")
                            || tag.contains("EffectNode")
                            || tag.contains("Effect"))
                    {
                        // Finalize this effect
                        let resolved_type = if !current_module_id.is_empty() {
                            data_model_types
                                .get(&current_module_id)
                                .cloned()
                                .unwrap_or_else(|| effect_type.clone())
                        } else {
                            effect_type.clone()
                        };

                        let resolved_color = effect_color.or_else(|| {
                            if !current_module_id.is_empty() {
                                data_model_colors.get(&current_module_id).copied()
                            } else {
                                None
                            }
                        });

                        if effect_duration > 0.0 {
                            effects.push(VixenEffect {
                                type_name: resolved_type,
                                start_time: effect_start,
                                duration: effect_duration,
                                target_node_guids: effect_targets.clone(),
                                color: resolved_color,
                            });
                        }

                        in_effect_node_entry = false;
                        effect_targets.clear();
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

    /// Build tracks from parsed Vixen effects, grouped by target node.
    fn build_tracks(&self, effects: Vec<VixenEffect>) -> Vec<Track> {
        // Group effects by their primary target
        let mut effects_by_target: HashMap<String, Vec<VixenEffect>> = HashMap::new();

        for effect in effects {
            let target_key = if effect.target_node_guids.is_empty() {
                "_all_".to_string()
            } else {
                effect.target_node_guids[0].clone()
            };
            effects_by_target
                .entry(target_key)
                .or_default()
                .push(effect);
        }

        let mut tracks = Vec::new();

        for (target_guid, mut target_effects) in effects_by_target {
            // Sort by start time
            target_effects.sort_by(|a, b| {
                a.start_time
                    .partial_cmp(&b.start_time)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Assign effects to lanes (non-overlapping within each lane)
            let mut lanes: Vec<Vec<&VixenEffect>> = Vec::new();

            for effect in &target_effects {
                let end_time = effect.start_time + effect.duration;
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

                let _ = end_time; // used in lane_end calculation
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
                    EffectTarget::All
                }
            } else {
                EffectTarget::All
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

                let effect_instances: Vec<EffectInstance> = lane
                    .iter()
                    .filter_map(|e| {
                        let end = e.start_time + e.duration;
                        let time_range = TimeRange::new(e.start_time, end)?;
                        let (kind, params) = map_vixen_effect(&e.type_name, e.color);
                        Some(EffectInstance {
                            kind,
                            params,
                            time_range,
                        })
                    })
                    .collect();

                if !effect_instances.is_empty() {
                    tracks.push(Track {
                        name: format!("{}{}", target_name, lane_suffix),
                        target: target.clone(),
                        blend_mode: BlendMode::Override,
                        effects: effect_instances,
                    });
                }
            }
        }

        tracks
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

    #[test]
    fn test_effect_mapping() {
        let (kind, _) = map_vixen_effect("Pulse", None);
        assert!(matches!(kind, EffectKind::Solid));

        let (kind, _) = map_vixen_effect("SetLevel", None);
        assert!(matches!(kind, EffectKind::Solid));

        let (kind, _) = map_vixen_effect("Chase", Some(Color::rgb(255, 0, 0)));
        assert!(matches!(kind, EffectKind::Chase));

        let (kind, _) = map_vixen_effect("ColorWash", None);
        assert!(matches!(kind, EffectKind::Gradient));

        let (kind, _) = map_vixen_effect("Twinkle", None);
        assert!(matches!(kind, EffectKind::Twinkle));

        let (kind, _) = map_vixen_effect("Strobe", None);
        assert!(matches!(kind, EffectKind::Strobe));

        let (kind, _) = map_vixen_effect("PinWheel", None);
        assert!(matches!(kind, EffectKind::Rainbow));

        let (kind, _) = map_vixen_effect("Alternating", None);
        assert!(matches!(kind, EffectKind::Chase));

        let (kind, _) = map_vixen_effect("Wipe", None);
        assert!(matches!(kind, EffectKind::Chase));

        let (kind, _) = map_vixen_effect("Rainbow", None);
        assert!(matches!(kind, EffectKind::Rainbow));

        // Unknown effect falls back to Solid
        let (kind, _) = map_vixen_effect("SomeUnknownEffect", None);
        assert!(matches!(kind, EffectKind::Solid));
    }
}
