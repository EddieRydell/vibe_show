use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::import::ImportError;
use crate::model::color::Color;
use crate::model::fixture::{
    BulbShape, ChannelOrder, ColorModel, Controller, ControllerId, ControllerProtocol,
    EffectTarget, FixtureDef, FixtureGroup, FixtureId, GroupId, GroupMember, PixelType,
};
use crate::model::show::{FixtureLayout, Layout, Show};
use crate::model::timeline::{
    BlendMode, EffectInstance, Sequence, TimeRange, Track,
};

use super::effects::map_vixen_effect;
use super::types::{CurveKind, VixenEffect, VixenNode};
use crate::import::parse_iso_duration;
use crate::import::xyz_to_srgb;

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
    #[must_use]
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

    /// Reconstruct importer state from an existing setup + saved GUID mapping.
    /// This allows importing sequences against a previously-imported setup.
    #[must_use]
    pub fn from_setup(
        fixtures: Vec<FixtureDef>,
        groups: Vec<FixtureGroup>,
        controllers: Vec<Controller>,
        patches: Vec<crate::model::fixture::Patch>,
        guid_map: HashMap<String, u32>,
    ) -> Self {
        let next_id = guid_map.values().copied().max().map_or(0, |m| m + 1);
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

    /// Return the GUID → ID mapping (for persisting after setup import).
    #[must_use]
    pub fn guid_map(&self) -> &HashMap<String, u32> {
        &self.guid_to_id
    }

    /// Return warnings accumulated during import.
    #[must_use]
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Count of parsed fixtures.
    #[must_use]
    pub fn fixture_count(&self) -> usize {
        self.fixtures.len()
    }

    /// Count of parsed groups.
    #[must_use]
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Count of parsed controllers.
    #[must_use]
    pub fn controller_count(&self) -> usize {
        self.controllers.len()
    }

    /// Parse Vixen preview layout data and produce `FixtureLayout` entries.
    ///
    /// # Errors
    ///
    /// Returns `ImportError` if the preview file cannot be found or parsed.
    pub fn parse_preview(
        &mut self,
        vixen_dir: &Path,
        preview_file_override: Option<&Path>,
    ) -> Result<Vec<FixtureLayout>, ImportError> {
        let preview_path = if let Some(override_path) = preview_file_override {
            override_path.to_path_buf()
        } else {
            super::preview::find_preview_file(vixen_dir).ok_or_else(|| {
                ImportError::Parse("No preview data file found".into())
            })?
        };

        let preview_data = super::preview::parse_preview_file(&preview_path)?;

        // Build pixel count map from current fixtures
        let pixel_counts: HashMap<u32, u32> = self
            .fixtures
            .iter()
            .map(|f| (f.id.0, f.pixel_count))
            .collect();

        let layouts = super::preview::build_fixture_layouts(
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
    ///
    /// # Errors
    ///
    /// Returns `ImportError` on I/O or XML parsing failures.
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
                            let Some(node) = node_stack.pop() else {
                                continue;
                            };
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
                                    ip = Some(val);
                                }
                                "universe" | "Universe" => {
                                    universe = val.parse().ok();
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
                                    name: format!("{current_name} ({ip})"),
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
            .flat_map(|n| n.children_guids.iter().map(String::as_str))
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
    /// Returns either a `FixtureId` or `GroupId` if successfully created.
    fn build_node(&mut self, guid: &str) -> Option<GroupMember> {
        // Already processed?
        if let Some(&id) = self.guid_to_id.get(guid) {
            let node = self.nodes.get(guid)?;
            if node.children_guids.is_empty() {
                return Some(GroupMember::Fixture(FixtureId(id)));
            }
            return Some(GroupMember::Group(GroupId(id)));
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
                pixel_type: PixelType::default(),
                bulb_shape: BulbShape::default(),
                display_radius_override: None,
                channel_order: ChannelOrder::default(),
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
                GroupMember::Group(_) => false,
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
                        GroupMember::Group(_) => None,
                    })
                    .collect();

                // Remove individual fixtures
                self.fixtures.retain(|f| !fixture_ids.contains(&f.id));

                // Create one multi-pixel fixture for this group of leaves
                #[allow(clippy::cast_possible_truncation)]
                let pixel_count = child_count as u32;
                self.fixtures.push(FixtureDef {
                    id: FixtureId(id),
                    name: node.name.clone(),
                    color_model: ColorModel::Rgb,
                    pixel_count,
                    pixel_type: PixelType::default(),
                    bulb_shape: BulbShape::default(),
                    display_radius_override: None,
                    channel_order: ChannelOrder::default(),
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
    ///
    /// An optional `progress_cb` receives a fraction (0.0–1.0) based on bytes
    /// read, allowing callers to report granular progress during large files.
    ///
    /// # Errors
    ///
    /// Returns `ImportError` on I/O or XML parsing failures.
    #[allow(clippy::too_many_lines)]
    pub fn parse_sequence(
        &mut self,
        path: &Path,
        progress_cb: Option<&dyn Fn(f64)>,
    ) -> Result<(), ImportError> {
        #[allow(clippy::cast_precision_loss)]
        let file_size = std::fs::metadata(path)?.len().max(1) as f64;
        let file = File::open(path)?;
        let reader = BufReader::with_capacity(64 * 1024, file);
        let mut xml = Reader::from_reader(reader);
        xml.config_mut().trim_text(true);

        let mut buf = Vec::with_capacity(4096);
        let mut event_count = 0u64;

        let seq_name = path
            .file_stem()
            .map_or_else(|| "Untitled".to_string(), |s| s.to_string_lossy().to_string());

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
        let mut current_curve_kind = CurveKind::Intensity;
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
        // Direction data keyed by ModuleInstanceId (for Wipe, Butterfly, etc.)
        let mut data_model_direction: HashMap<String, String> = HashMap::new();

        // For effect node surrogates
        let mut in_effect_node_entry = false;
        let mut current_module_id = String::new();
        let mut current_effect_instance_id = String::new();
        let mut effect_node_depth = 0u32;

        let mut depth = 0u32;

        loop {
            // Report sub-progress based on bytes parsed
            event_count += 1;
            if let Some(cb) = &progress_cb {
                if event_count.is_multiple_of(5000) {
                    #[allow(clippy::cast_precision_loss)]
                    let pos = xml.buffer_position() as f64;
                    cb((pos / file_size).min(1.0));
                }
            }

            match xml.read_event_into(&mut buf) {
                Ok(Event::Eof) => break,
                Ok(Event::Start(ref e)) => {
                    depth += 1;
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    current_element.clone_from(&tag);

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
                            "ChaseMovement" | "MovementCurve" | "WipeMovement" => {
                                in_curve_element = true;
                                current_curve_kind = CurveKind::Movement;
                                current_curve_points.clear();
                            }
                            "PulseCurve" => {
                                in_curve_element = true;
                                current_curve_kind = CurveKind::Pulse;
                                current_curve_points.clear();
                            }
                            "LevelCurve" | "IntensityCurve" | "Curve"
                            | "DissolveCurve" | "AccelerationCurve"
                            | "SpeedCurve" | "Height" => {
                                in_curve_element = true;
                                current_curve_kind = CurveKind::Intensity;
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
                                // Direction for Wipe, Butterfly, etc.
                                "Direction" | "WipeDirection" => {
                                    data_model_direction.insert(id.clone(), text.clone());
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
                                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
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
                            "ChaseMovement" | "MovementCurve" | "WipeMovement"
                            | "PulseCurve" | "LevelCurve"
                            | "IntensityCurve" | "Curve"
                            | "DissolveCurve" | "AccelerationCurve"
                            | "SpeedCurve" | "Height"
                                if in_curve_element =>
                            {
                                if !current_curve_points.is_empty()
                                    && !current_data_model_id.is_empty()
                                {
                                    let target_map = match current_curve_kind {
                                        CurveKind::Movement => &mut data_model_movement_curves,
                                        CurveKind::Pulse => &mut data_model_pulse_curves,
                                        CurveKind::Intensity => &mut data_model_intensity_curves,
                                    };
                                    target_map.insert(
                                        current_data_model_id.clone(),
                                        current_curve_points.clone(),
                                    );
                                }
                                in_curve_element = false;
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
                        let resolved_direction = data_model_direction
                            .get(&current_effect_instance_id)
                            .or_else(|| data_model_direction.get(&current_module_id))
                            .cloned();

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
                                direction: resolved_direction,
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
            motion_paths: std::collections::HashMap::new(),
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
        let Some(first) = effects.first() else {
            return;
        };
        let mut current = first.clone();

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
    #[allow(clippy::too_many_lines)]
    fn build_tracks(&self, effects: Vec<VixenEffect>) -> Vec<Track> {
        const MAX_TOTAL_EFFECTS: usize = 10_000;

        // Group effects by their primary target
        let mut effects_by_target: HashMap<String, Vec<VixenEffect>> = HashMap::new();

        for effect in effects {
            if effect.target_node_guids.is_empty() {
                effects_by_target
                    .entry("_all_".to_string())
                    .or_default()
                    .push(effect);
            } else {
                let Some(target_guid) = effect.target_node_guids.first() else {
                    continue;
                };
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
                        .map_or(0.0, |e| e.start_time + e.duration);
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
                    .map_or_else(|| format!("Track {}", tracks.len() + 1), |n| n.name.clone())
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
                            blend_mode: BlendMode::Override,
                            opacity: 1.0,
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
                        name: format!("{target_name}{lane_suffix}"),
                        target: target.clone(),
                        effects: effect_instances,
                    });
                }
            }
        }

        // Cap total effects
        if total_effects > MAX_TOTAL_EFFECTS {
            eprintln!(
                "[VibeLights] Warning: {total_effects} effects exceed cap of {MAX_TOTAL_EFFECTS}. Truncating tracks.",
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
    #[must_use]
    pub fn into_sequences(self) -> Vec<Sequence> {
        self.sequences
    }

    /// Consume the importer and produce a Show.
    #[must_use]
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
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::uninlined_format_args,
    clippy::bool_assert_comparison,
    clippy::match_same_arms,
    clippy::option_map_or_none,
)]
mod tests {
    use super::*;
    use crate::import::{parse_iso_duration, xyz_to_srgb};

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
}
