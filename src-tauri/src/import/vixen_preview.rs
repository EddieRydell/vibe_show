use std::fs;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;

use super::vixen::ImportError;

// ── Internal types ──────────────────────────────────────────────────

/// Raw preview data extracted from a Vixen preview XML file.
pub struct VixenPreviewData {
    pub width: u32,
    pub height: u32,
    pub display_items: Vec<VixenDisplayItem>,
}

/// A single display item (fixture shape) from the Vixen preview.
pub struct VixenDisplayItem {
    pub name: String,
    pub shape_type: String,
    pub pixels: Vec<VixenPreviewPixel>,
}

/// A single pixel within a display item, with its GUID link to the element node tree.
pub struct VixenPreviewPixel {
    pub node_id: String,
    pub x: f64,
    pub y: f64,
}

// ── File discovery ──────────────────────────────────────────────────

/// Find the Vixen preview data file within a Vixen data directory.
///
/// Vixen 3 stores preview/display data in `SystemData/ModuleStore.xml` inside
/// a `<VixenPreviewData>` section. Falls back to scanning `Module Data Files/`
/// for standalone preview XML files.
pub fn find_preview_file(vixen_dir: &Path) -> Option<std::path::PathBuf> {
    // Primary location: SystemData/ModuleStore.xml
    let module_store = vixen_dir.join("SystemData").join("ModuleStore.xml");
    if module_store.exists() && file_contains_preview_marker(&module_store) {
        return Some(module_store);
    }

    // Fallback: scan Module Data Files/ for standalone preview XML
    let module_dir = vixen_dir.join("Module Data Files");
    if module_dir.exists() {
        // Check common known paths
        let candidates = [
            module_dir.join("VixenPreviewData.xml"),
            module_dir.join("VixenPreview"),
        ];
        for candidate in &candidates {
            if candidate.is_file() && file_contains_preview_marker(candidate) {
                return Some(candidate.clone());
            } else if candidate.is_dir() {
                if let Some(found) = scan_dir_for_preview(candidate) {
                    return Some(found);
                }
            }
        }

        // Fall back to recursive scan
        if let Some(found) = scan_dir_for_preview(&module_dir) {
            return Some(found);
        }
    }

    None
}

/// Recursively scan a directory for XML files containing preview data markers.
fn scan_dir_for_preview(dir: &Path) -> Option<std::path::PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    let mut subdirs = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if ext == "xml" && file_contains_preview_marker(&path) {
                return Some(path);
            }
        } else if path.is_dir() {
            subdirs.push(path);
        }
    }

    for subdir in subdirs {
        if let Some(found) = scan_dir_for_preview(&subdir) {
            return Some(found);
        }
    }

    None
}

/// Quick check: read a portion of a file looking for preview-related XML tags.
fn file_contains_preview_marker(path: &Path) -> bool {
    // For large files (like ModuleStore.xml at 100MB+), we can't read the whole thing.
    // Read in chunks and check for markers.
    let Ok(file) = fs::File::open(path) else {
        return false;
    };
    let metadata = file.metadata().ok();
    let file_size = metadata.map(|m| m.len()).unwrap_or(0);

    // For small files, read the whole thing
    if file_size < 64 * 1024 {
        let Ok(data) = fs::read(path) else {
            return false;
        };
        let text = String::from_utf8_lossy(&data);
        return text.contains("DisplayItem") || text.contains("VixenPreviewData");
    }

    // For large files, read the first 64KB. ModuleStore.xml typically has the
    // preview section further in, but the Module dataModelType string appears early
    // in the Module element's attributes. Also check a middle chunk.
    use std::io::Read;
    let mut file = file;
    let mut buf = vec![0u8; 64 * 1024];

    // Check beginning
    if let Ok(n) = file.read(&mut buf) {
        let text = String::from_utf8_lossy(&buf[..n]);
        if text.contains("VixenPreviewData") || text.contains("DisplayItem") {
            return true;
        }
    }

    // For ModuleStore.xml, the VixenPreviewData section can appear anywhere in
    // the file (often near the end of 100MB+ files). Scan the entire file in
    // chunks, with overlap to avoid missing markers split across boundaries.
    use std::io::Seek;
    let chunk_size = 256 * 1024usize;
    let overlap = 64usize; // enough to catch "VixenPreviewData" split across chunks
    let mut large_buf = vec![0u8; chunk_size];
    let _ = file.seek(std::io::SeekFrom::Start(0));
    let mut carry = Vec::<u8>::new();
    loop {
        match file.read(&mut large_buf) {
            Ok(0) => break,
            Ok(n) => {
                // Prepend carry from previous chunk boundary
                let search_buf = if carry.is_empty() {
                    &large_buf[..n]
                } else {
                    carry.extend_from_slice(&large_buf[..n]);
                    &carry
                };
                let text = String::from_utf8_lossy(search_buf);
                if text.contains("VixenPreviewData") || text.contains("DisplayItem") {
                    return true;
                }
                // Keep the last `overlap` bytes to catch split markers
                carry.clear();
                if n > overlap {
                    carry.extend_from_slice(&large_buf[n - overlap..n]);
                } else {
                    carry.extend_from_slice(&large_buf[..n]);
                }
            }
            Err(_) => break,
        }
    }

    false
}

// ── Preview parsing ─────────────────────────────────────────────────

/// Parse a Vixen preview data file into structured preview data.
/// Handles both standalone preview XML and ModuleStore.xml containing VixenPreviewData.
pub fn parse_preview_file(path: &Path) -> Result<VixenPreviewData, ImportError> {
    let data = fs::read(path)?;
    parse_preview_xml(&data)
}

/// Strip XML namespace prefix: "a:Foo" → "Foo", "Foo" → "Foo"
fn strip_ns(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// Parse Vixen preview XML from bytes.
///
/// Handles the Vixen 3 DataContract XML format where elements use namespace prefixes
/// (e.g., `a:DisplayItem`, `a:PreviewPixel`). Pixel positions are derived from the
/// shape's `<_points>` control points and interpolated across the pixel count.
pub fn parse_preview_xml(data: &[u8]) -> Result<VixenPreviewData, ImportError> {
    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::with_capacity(8192);

    // Track each VixenPreviewData section independently so we can pick the best one.
    // Merging sections is wrong: the same fixtures appear in each section with
    // coordinates designed for different canvas sizes, causing duplicates.
    struct PreviewSection {
        width: u32,
        height: u32,
        display_items: Vec<VixenDisplayItem>,
    }
    let mut sections: Vec<PreviewSection> = Vec::new();
    let mut current_section_width: u32 = 0;
    let mut current_section_height: u32 = 0;
    let mut display_items: Vec<VixenDisplayItem> = Vec::new();

    // Whether we're inside a VixenPreviewData section
    let mut in_preview_data = false;
    let mut preview_data_depth = 0u32;

    // Display item state
    let mut in_display_item = false;
    let mut display_item_depth = 0u32;
    let mut current_shape_type = String::new();
    let mut current_node_ids: Vec<String> = Vec::new();

    // Shape control points (_points > PreviewPoint)
    let mut in_points = false;
    let mut in_preview_point = false;
    let mut current_points: Vec<(f64, f64)> = Vec::new();
    let mut point_x: f64 = 0.0;
    let mut point_y: f64 = 0.0;

    // Pixel state (collecting NodeIds)
    let mut in_pixel = false;
    let mut current_node_id = String::new();
    // For formats where pixel has direct X/Y
    let mut pixel_has_position = false;
    let mut pixel_x: f64 = 0.0;
    let mut pixel_y: f64 = 0.0;
    let mut pixel_positions: Vec<(String, f64, f64)> = Vec::new();

    let mut current_element = String::new();
    let mut depth = 0u32;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(ref e)) => {
                depth += 1;
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let local = strip_ns(&tag);
                current_element = local.to_string();

                match local {
                    "VixenPreviewData" | "Preview" => {
                        if !in_preview_data {
                            in_preview_data = true;
                            preview_data_depth = depth;
                        }
                    }
                    "DisplayItems" if !in_preview_data => {
                        // Standalone format: <DisplayItems> without VixenPreviewData wrapper
                        in_preview_data = true;
                        preview_data_depth = depth;
                    }
                    "DisplayItem" if in_preview_data => {
                        in_display_item = true;
                        display_item_depth = depth;
                        current_shape_type.clear();
                        current_node_ids.clear();
                        current_points.clear();
                        pixel_positions.clear();
                    }
                    "Shape" if in_display_item => {
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            if strip_ns(&key) == "type" {
                                current_shape_type =
                                    strip_ns(&val).to_string();
                            }
                        }
                    }
                    "PreviewPixel" | "Pixel" | "LightNode" if in_display_item => {
                        in_pixel = true;
                        current_node_id.clear();
                        pixel_has_position = false;
                        pixel_x = 0.0;
                        pixel_y = 0.0;

                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            match strip_ns(&key) {
                                "NodeId" | "nodeId" | "Node" => current_node_id = val,
                                "X" | "x" => {
                                    pixel_x = val.parse().unwrap_or(0.0);
                                    pixel_has_position = true;
                                }
                                "Y" | "y" => {
                                    pixel_y = val.parse().unwrap_or(0.0);
                                    pixel_has_position = true;
                                }
                                _ => {}
                            }
                        }
                    }
                    "_points" | "Points" if in_display_item => {
                        in_points = true;
                    }
                    "PreviewPoint" | "Point" if in_points => {
                        in_preview_point = true;
                        point_x = 0.0;
                        point_y = 0.0;

                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            match strip_ns(&key) {
                                "X" | "x" => point_x = val.parse().unwrap_or(0.0),
                                "Y" | "y" => point_y = val.parse().unwrap_or(0.0),
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().trim().to_string();
                if text.is_empty() {
                    continue;
                }

                let el = current_element.as_str();

                // Canvas dimensions (inside VixenPreviewData, outside DisplayItems)
                if in_preview_data && !in_display_item {
                    match el {
                        "Width" | "SetupWidth" => {
                            if let Ok(w) = text.parse::<u32>() {
                                if w > current_section_width {
                                    current_section_width = w;
                                }
                            }
                        }
                        "Height" | "SetupHeight" => {
                            if let Ok(h) = text.parse::<u32>() {
                                if h > current_section_height {
                                    current_section_height = h;
                                }
                            }
                        }
                        _ => {}
                    }
                }

                // Pixel NodeId as text content
                if in_pixel {
                    match el {
                        "NodeId" | "nodeId" | "Node" => current_node_id = text,
                        "X" | "x" => {
                            pixel_x = text.parse().unwrap_or(pixel_x);
                            pixel_has_position = true;
                        }
                        "Y" | "y" => {
                            pixel_y = text.parse().unwrap_or(pixel_y);
                            pixel_has_position = true;
                        }
                        _ => {}
                    }
                } else if in_preview_point {
                    // Shape control point coordinates
                    match el {
                        "X" | "x" => point_x = text.parse().unwrap_or(point_x),
                        "Y" | "y" => point_y = text.parse().unwrap_or(point_y),
                        _ => {}
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let local = strip_ns(&tag);
                depth = depth.saturating_sub(1);

                match local {
                    "VixenPreviewData" | "Preview"
                        if in_preview_data && depth < preview_data_depth =>
                    {
                        // Save this section and reset for the next one
                        if !display_items.is_empty() {
                            sections.push(PreviewSection {
                                width: current_section_width,
                                height: current_section_height,
                                display_items: std::mem::take(&mut display_items),
                            });
                        }
                        current_section_width = 0;
                        current_section_height = 0;
                        in_preview_data = false;
                    }
                    "PreviewPixel" | "Pixel" | "LightNode" if in_pixel => {
                        if !current_node_id.is_empty() {
                            if pixel_has_position {
                                pixel_positions.push((
                                    current_node_id.clone(),
                                    pixel_x,
                                    pixel_y,
                                ));
                            } else {
                                current_node_ids.push(current_node_id.clone());
                            }
                        }
                        in_pixel = false;
                    }
                    "PreviewPoint" | "Point" if in_preview_point => {
                        current_points.push((point_x, point_y));
                        in_preview_point = false;
                    }
                    "_points" | "Points" if in_points => {
                        in_points = false;
                    }
                    "DisplayItem" if in_display_item && depth < display_item_depth => {
                        // Build pixels for this display item
                        let pixels = if !pixel_positions.is_empty() {
                            // Pixels have direct positions (standalone format)
                            pixel_positions
                                .iter()
                                .map(|(nid, x, y)| VixenPreviewPixel {
                                    node_id: nid.clone(),
                                    x: *x,
                                    y: *y,
                                })
                                .collect()
                        } else if !current_node_ids.is_empty() {
                            // Interpolate positions from shape control points
                            interpolate_pixels(&current_node_ids, &current_points)
                        } else {
                            Vec::new()
                        };

                        if !pixels.is_empty() {
                            display_items.push(VixenDisplayItem {
                                name: format!("Item {}", display_items.len() + 1),
                                shape_type: current_shape_type.clone(),
                                pixels,
                            });
                        }
                        in_display_item = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let local = strip_ns(&tag);

                // Self-closing pixel elements
                if (local == "PreviewPixel" || local == "Pixel" || local == "LightNode")
                    && in_display_item
                {
                    let mut node_id = String::new();
                    let mut x = 0.0f64;
                    let mut y = 0.0f64;
                    let mut has_pos = false;

                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let val = String::from_utf8_lossy(&attr.value).to_string();
                        match strip_ns(&key) {
                            "NodeId" | "nodeId" | "Node" => node_id = val,
                            "X" | "x" => {
                                x = val.parse().unwrap_or(0.0);
                                has_pos = true;
                            }
                            "Y" | "y" => {
                                y = val.parse().unwrap_or(0.0);
                                has_pos = true;
                            }
                            _ => {}
                        }
                    }

                    if !node_id.is_empty() {
                        if has_pos {
                            pixel_positions.push((node_id, x, y));
                        } else {
                            current_node_ids.push(node_id);
                        }
                    }
                }

                // Self-closing PreviewPoint
                if (local == "PreviewPoint" || local == "Point") && in_points {
                    let mut px = 0.0f64;
                    let mut py = 0.0f64;
                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let val = String::from_utf8_lossy(&attr.value).to_string();
                        match strip_ns(&key) {
                            "X" | "x" => px = val.parse().unwrap_or(0.0),
                            "Y" | "y" => py = val.parse().unwrap_or(0.0),
                            _ => {}
                        }
                    }
                    current_points.push((px, py));
                }
            }
            Err(e) => return Err(ImportError::Xml(e)),
            _ => {}
        }
        buf.clear();
    }

    // Flush any remaining items (e.g. standalone format without VixenPreviewData wrapper,
    // or if the closing tag wasn't matched).
    if !display_items.is_empty() {
        sections.push(PreviewSection {
            width: current_section_width,
            height: current_section_height,
            display_items,
        });
    }

    // Pick the section with the most display items. Multiple VixenPreviewData sections
    // represent different display configurations (e.g. multiple monitors); each contains
    // the full fixture set positioned for its own canvas size. Merging them would duplicate
    // every fixture with mis-scaled coordinates.
    let best = sections
        .into_iter()
        .max_by_key(|s| s.display_items.len())
        .unwrap_or(PreviewSection {
            width: 0,
            height: 0,
            display_items: Vec::new(),
        });

    let mut width = best.width;
    let mut height = best.height;

    // Default canvas if nothing found
    if width == 0 {
        width = 1920;
    }
    if height == 0 {
        height = 1080;
    }

    Ok(VixenPreviewData {
        width,
        height,
        display_items: best.display_items,
    })
}

/// Interpolate pixel positions from shape control points.
///
/// In Vixen 3, each DisplayItem has N pixels (with NodeIds) and M control points.
/// Pixels are distributed evenly along the polyline defined by the control points.
fn interpolate_pixels(
    node_ids: &[String],
    points: &[(f64, f64)],
) -> Vec<VixenPreviewPixel> {
    if node_ids.is_empty() {
        return Vec::new();
    }

    // If no control points, we can't determine positions — use (0,0) as fallback
    if points.is_empty() {
        return node_ids
            .iter()
            .map(|nid| VixenPreviewPixel {
                node_id: nid.clone(),
                x: 0.0,
                y: 0.0,
            })
            .collect();
    }

    // Single control point: all pixels at that location
    if points.len() == 1 {
        return node_ids
            .iter()
            .map(|nid| VixenPreviewPixel {
                node_id: nid.clone(),
                x: points[0].0,
                y: points[0].1,
            })
            .collect();
    }

    // Calculate segment lengths along the polyline
    let mut segment_lengths = Vec::with_capacity(points.len() - 1);
    let mut total_length = 0.0f64;
    for i in 0..points.len() - 1 {
        let dx = points[i + 1].0 - points[i].0;
        let dy = points[i + 1].1 - points[i].1;
        let len = (dx * dx + dy * dy).sqrt();
        segment_lengths.push(len);
        total_length += len;
    }

    if total_length < 0.001 {
        // Degenerate case: all control points at same location
        return node_ids
            .iter()
            .map(|nid| VixenPreviewPixel {
                node_id: nid.clone(),
                x: points[0].0,
                y: points[0].1,
            })
            .collect();
    }

    // Distribute pixels evenly along total length
    let pixel_count = node_ids.len();
    let mut pixels = Vec::with_capacity(pixel_count);

    for (i, nid) in node_ids.iter().enumerate() {
        let t = if pixel_count > 1 {
            i as f64 / (pixel_count - 1) as f64
        } else {
            0.5
        };
        let target_dist = t * total_length;

        // Walk along segments to find position
        let mut accumulated = 0.0f64;
        let mut x = points[0].0;
        let mut y = points[0].1;

        for (seg_idx, &seg_len) in segment_lengths.iter().enumerate() {
            if accumulated + seg_len >= target_dist || seg_idx == segment_lengths.len() - 1 {
                let remaining = target_dist - accumulated;
                let seg_t = if seg_len > 0.001 {
                    (remaining / seg_len).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                x = points[seg_idx].0 + (points[seg_idx + 1].0 - points[seg_idx].0) * seg_t;
                y = points[seg_idx].1 + (points[seg_idx + 1].1 - points[seg_idx].1) * seg_t;
                break;
            }
            accumulated += seg_len;
        }

        pixels.push(VixenPreviewPixel {
            node_id: nid.clone(),
            x,
            y,
        });
    }

    pixels
}

// ── Position normalization and fixture mapping ──────────────────────

use std::collections::{HashMap, HashSet};
use crate::model::fixture::FixtureId;
use crate::model::show::{FixtureLayout, LayoutShape, Position2D};

/// Map parsed preview data to FixtureLayout entries.
///
/// Groups pixels by fixture using the guid_to_id mapping, normalizes positions
/// to 0.0-1.0 using the canvas dimensions, and infers shape types where possible.
pub fn build_fixture_layouts(
    preview: &VixenPreviewData,
    guid_to_id: &HashMap<String, u32>,
    fixture_pixel_counts: &HashMap<u32, u32>,
    warnings: &mut Vec<String>,
) -> Vec<FixtureLayout> {
    // Group all pixels by their resolved fixture ID, collecting raw coordinates.
    // Track seen node IDs to avoid duplicates — the same node can appear in
    // multiple DisplayItems (e.g. PixelGrid sub-shapes repeat node IDs 50x,
    // and standalone items duplicate PixelGrid entries). Only the first
    // occurrence of each node ID is used.
    let mut fixture_pixels: HashMap<u32, Vec<(f64, f64)>> = HashMap::new();
    let mut seen_node_ids: HashSet<&str> = HashSet::new();
    let mut unresolved_count = 0usize;

    let known_shapes = [
        "PreviewLine",
        "PreviewArch",
        "PreviewNet",
        "PreviewPixelGrid",
        "PreviewRectangle",
        "PreviewMegaTree",
        "PreviewStar",
        "PreviewCustomProp",
        "PreviewEllipse",
        "PreviewPolyLine",
        "PreviewMultiString",
        "PreviewTriangle",
    ];

    // Collect raw pixel positions first (no normalization yet)
    let mut all_raw: Vec<(f64, f64)> = Vec::new();

    for item in &preview.display_items {
        if !item.shape_type.is_empty() && !known_shapes.contains(&item.shape_type.as_str()) {
            warnings.push(format!(
                "Unsupported preview shape type '{}' for '{}' — imported as Custom",
                item.shape_type, item.name
            ));
        }

        for pixel in &item.pixels {
            // Skip if this node was already seen in a PREVIOUS display item.
            // The same fixture often appears in multiple items (e.g. once in a
            // PixelGrid and once as a standalone shape), producing duplicate
            // positions at different scales. Only use positions from the first
            // display item that references each node ID.
            if seen_node_ids.contains(pixel.node_id.as_str()) {
                continue;
            }
            if let Some(&id) = guid_to_id.get(&pixel.node_id) {
                fixture_pixels.entry(id).or_default().push((pixel.x, pixel.y));
                all_raw.push((pixel.x, pixel.y));
            } else {
                unresolved_count += 1;
            }
        }

        // Mark all node IDs from this item as globally seen
        for pixel in &item.pixels {
            seen_node_ids.insert(&pixel.node_id);
        }
    }

    // Normalize against the actual bounding box of all pixel coordinates.
    // This is more robust than using the canvas width/height from XML, which
    // may not be parsed correctly or may not match the coordinate system used
    // by pixel positions (different Vixen versions use different scales).
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for &(x, y) in &all_raw {
        if x < min_x { min_x = x; }
        if x > max_x { max_x = x; }
        if y < min_y { min_y = y; }
        if y > max_y { max_y = y; }
    }
    let range_x = (max_x - min_x).max(1.0);
    let range_y = (max_y - min_y).max(1.0);

    // Remap all collected positions to 0.0-1.0 based on actual bounding box
    for positions in fixture_pixels.values_mut() {
        for (x, y) in positions.iter_mut() {
            *x = (*x - min_x) / range_x;
            *y = (*y - min_y) / range_y;
        }
    }

    if unresolved_count > 0 {
        warnings.push(format!(
            "{} preview pixels could not be mapped to imported fixtures (orphan node IDs)",
            unresolved_count
        ));
    }

    // Build FixtureLayout for each fixture that has pixels
    let mut layouts: Vec<FixtureLayout> = Vec::new();

    for (&fixture_id, positions) in &fixture_pixels {
        let expected_count = fixture_pixel_counts
            .get(&fixture_id)
            .copied()
            .unwrap_or(1) as usize;

        // If we have more positions than pixels (e.g. individual RGB channels mapped),
        // take every Nth to match pixel count
        let pixel_positions: Vec<Position2D> = if positions.len() > expected_count
            && expected_count > 0
        {
            let step = positions.len() as f64 / expected_count as f64;
            (0..expected_count)
                .map(|i| {
                    let idx = (i as f64 * step) as usize;
                    let (x, y) = positions[idx.min(positions.len() - 1)];
                    Position2D {
                        x: x as f32,
                        y: y as f32,
                    }
                })
                .collect()
        } else {
            positions
                .iter()
                .map(|&(x, y)| Position2D {
                    x: x as f32,
                    y: y as f32,
                })
                .collect()
        };

        let shape = if pixel_positions.len() >= 2 {
            infer_shape(&pixel_positions)
        } else {
            LayoutShape::Custom
        };

        layouts.push(FixtureLayout {
            fixture_id: FixtureId(fixture_id),
            pixel_positions,
            shape,
        });
    }

    // Sort by fixture ID for deterministic output
    layouts.sort_by_key(|l| l.fixture_id.0);

    layouts
}

/// Try to infer a LayoutShape from a set of pixel positions.
fn infer_shape(positions: &[Position2D]) -> LayoutShape {
    if positions.len() < 2 {
        return LayoutShape::Custom;
    }

    let first = positions[0];
    let last = positions[positions.len() - 1];

    // Check if roughly collinear (line)
    if positions.len() >= 3 {
        let dx = last.x - first.x;
        let dy = last.y - first.y;
        let length = (dx * dx + dy * dy).sqrt();

        if length > 0.001 {
            let max_deviation = positions
                .iter()
                .map(|p| {
                    let t = ((p.x - first.x) * dx + (p.y - first.y) * dy) / (length * length);
                    let proj_x = first.x + t * dx;
                    let proj_y = first.y + t * dy;
                    let dev_x = p.x - proj_x;
                    let dev_y = p.y - proj_y;
                    (dev_x * dev_x + dev_y * dev_y).sqrt()
                })
                .fold(0.0f32, f32::max);

            if max_deviation < 0.02 {
                return LayoutShape::Line {
                    start: first,
                    end: last,
                };
            }
        }
    }

    LayoutShape::Custom
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_vixen3_module_store_format() {
        // This mimics the real Vixen 3 ModuleStore.xml format with namespace prefixes
        let xml = br#"<?xml version="1.0" encoding="utf-8"?>
<ModuleStore version="4">
  <ModuleData>
    <Module dataModelType="VixenModules.Preview.VixenPreview.VixenPreviewData, VixenPreview">
      <VixenPreviewData xmlns="http://schemas.datacontract.org/2004/07/VixenModules.Preview.VixenPreview" xmlns:i="http://www.w3.org/2001/XMLSchema-instance">
        <ModuleInstanceId xmlns="">test-id</ModuleInstanceId>
        <Height>1080</Height>
        <SetupWidth>1920</SetupWidth>
        <Width>1920</Width>
        <DisplayItems xmlns:a="http://schemas.datacontract.org/2004/07/VixenModules.Preview.VixenPreview.Shapes">
          <a:DisplayItem>
            <a:Shape i:type="a:PreviewLine">
              <a:PixelSize>3</a:PixelSize>
              <a:Pixels>
                <a:PreviewPixel>
                  <a:NodeId>guid-aaa</a:NodeId>
                </a:PreviewPixel>
                <a:PreviewPixel>
                  <a:NodeId>guid-bbb</a:NodeId>
                </a:PreviewPixel>
                <a:PreviewPixel>
                  <a:NodeId>guid-ccc</a:NodeId>
                </a:PreviewPixel>
              </a:Pixels>
              <a:_points>
                <a:PreviewPoint>
                  <a:X>100</a:X>
                  <a:Y>200</a:Y>
                </a:PreviewPoint>
                <a:PreviewPoint>
                  <a:X>500</a:X>
                  <a:Y>200</a:Y>
                </a:PreviewPoint>
              </a:_points>
            </a:Shape>
          </a:DisplayItem>
        </DisplayItems>
      </VixenPreviewData>
    </Module>
  </ModuleData>
</ModuleStore>"#;

        let preview = parse_preview_xml(xml).unwrap();
        assert_eq!(preview.width, 1920);
        assert_eq!(preview.height, 1080);
        assert_eq!(preview.display_items.len(), 1);

        let item = &preview.display_items[0];
        assert_eq!(item.shape_type, "PreviewLine");
        assert_eq!(item.pixels.len(), 3);

        // Pixels should be interpolated along the control points (100,200) → (500,200)
        assert_eq!(item.pixels[0].node_id, "guid-aaa");
        assert!((item.pixels[0].x - 100.0).abs() < 0.01);
        assert!((item.pixels[0].y - 200.0).abs() < 0.01);

        // Middle pixel at t=0.5: (300, 200)
        assert_eq!(item.pixels[1].node_id, "guid-bbb");
        assert!((item.pixels[1].x - 300.0).abs() < 0.01);

        // Last pixel at endpoint
        assert_eq!(item.pixels[2].node_id, "guid-ccc");
        assert!((item.pixels[2].x - 500.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_polyline_shape() {
        let xml = br#"<?xml version="1.0"?>
<VixenPreviewData>
    <Width>1000</Width>
    <Height>1000</Height>
    <DisplayItems>
        <DisplayItem>
            <Shape i:type="PreviewPolyLine">
                <Pixels>
                    <PreviewPixel><NodeId>p1</NodeId></PreviewPixel>
                    <PreviewPixel><NodeId>p2</NodeId></PreviewPixel>
                    <PreviewPixel><NodeId>p3</NodeId></PreviewPixel>
                </Pixels>
                <_points>
                    <PreviewPoint><X>0</X><Y>0</Y></PreviewPoint>
                    <PreviewPoint><X>500</X><Y>0</Y></PreviewPoint>
                    <PreviewPoint><X>500</X><Y>500</Y></PreviewPoint>
                </_points>
            </Shape>
        </DisplayItem>
    </DisplayItems>
</VixenPreviewData>"#;

        let preview = parse_preview_xml(xml).unwrap();
        assert_eq!(preview.display_items.len(), 1);
        let item = &preview.display_items[0];
        assert_eq!(item.pixels.len(), 3);

        // First pixel at start of polyline
        assert!((item.pixels[0].x).abs() < 0.01);
        assert!((item.pixels[0].y).abs() < 0.01);

        // Last pixel at end of polyline
        assert!((item.pixels[2].x - 500.0).abs() < 0.01);
        assert!((item.pixels[2].y - 500.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_standalone_format_with_positions() {
        // Standalone format where pixels have direct X/Y attributes
        let xml = br#"<?xml version="1.0"?>
<Preview>
    <Width>1920</Width>
    <Height>1080</Height>
    <DisplayItems>
        <DisplayItem>
            <Name>Roofline</Name>
            <Shape i:type="PreviewLine">
                <PreviewPixel NodeId="aaa-111" X="100" Y="200"/>
                <PreviewPixel NodeId="bbb-222" X="300" Y="200"/>
                <PreviewPixel NodeId="ccc-333" X="500" Y="200"/>
            </Shape>
        </DisplayItem>
    </DisplayItems>
</Preview>"#;

        let preview = parse_preview_xml(xml).unwrap();
        assert_eq!(preview.width, 1920);
        assert_eq!(preview.display_items.len(), 1);

        let item = &preview.display_items[0];
        assert_eq!(item.shape_type, "PreviewLine");
        assert_eq!(item.pixels.len(), 3);
        assert_eq!(item.pixels[0].node_id, "aaa-111");
        assert!((item.pixels[0].x - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_interpolate_pixels_two_points() {
        let ids = vec!["a".into(), "b".into(), "c".into()];
        let points = vec![(0.0, 0.0), (100.0, 0.0)];
        let pixels = interpolate_pixels(&ids, &points);

        assert_eq!(pixels.len(), 3);
        assert!((pixels[0].x).abs() < 0.01); // 0.0
        assert!((pixels[1].x - 50.0).abs() < 0.01); // 50.0
        assert!((pixels[2].x - 100.0).abs() < 0.01); // 100.0
    }

    #[test]
    fn test_interpolate_pixels_polyline() {
        // L-shape: (0,0) → (100,0) → (100,100), total length = 200
        let ids: Vec<String> = (0..5).map(|i| format!("n{}", i)).collect();
        let points = vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0)];
        let pixels = interpolate_pixels(&ids, &points);

        assert_eq!(pixels.len(), 5);
        // First at (0,0)
        assert!((pixels[0].x).abs() < 0.01);
        assert!((pixels[0].y).abs() < 0.01);
        // t=0.25 → dist=50 along first segment → (50, 0)
        assert!((pixels[1].x - 50.0).abs() < 0.01);
        assert!((pixels[1].y).abs() < 0.01);
        // t=0.5 → dist=100 → corner at (100, 0)
        assert!((pixels[2].x - 100.0).abs() < 0.01);
        assert!((pixels[2].y).abs() < 0.01);
        // t=0.75 → dist=150 → (100, 50)
        assert!((pixels[3].x - 100.0).abs() < 0.01);
        assert!((pixels[3].y - 50.0).abs() < 0.01);
        // Last at (100, 100)
        assert!((pixels[4].x - 100.0).abs() < 0.01);
        assert!((pixels[4].y - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_interpolate_pixels_no_points() {
        let ids = vec!["a".into(), "b".into()];
        let points: Vec<(f64, f64)> = Vec::new();
        let pixels = interpolate_pixels(&ids, &points);

        assert_eq!(pixels.len(), 2);
        // Fallback to (0,0)
        assert!((pixels[0].x).abs() < 0.01);
    }

    #[test]
    fn test_normalize_positions() {
        let preview = VixenPreviewData {
            width: 1920,
            height: 1080,
            display_items: vec![VixenDisplayItem {
                name: "Test".into(),
                shape_type: "PreviewLine".into(),
                pixels: vec![
                    VixenPreviewPixel { node_id: "guid-1".into(), x: 0.0, y: 0.0 },
                    VixenPreviewPixel { node_id: "guid-2".into(), x: 960.0, y: 540.0 },
                    VixenPreviewPixel { node_id: "guid-3".into(), x: 1920.0, y: 1080.0 },
                ],
            }],
        };

        let mut guid_to_id = HashMap::new();
        guid_to_id.insert("guid-1".to_string(), 10);
        guid_to_id.insert("guid-2".to_string(), 10);
        guid_to_id.insert("guid-3".to_string(), 10);

        let mut pixel_counts = HashMap::new();
        pixel_counts.insert(10, 3);

        let mut warnings = Vec::new();
        let layouts = build_fixture_layouts(&preview, &guid_to_id, &pixel_counts, &mut warnings);

        assert_eq!(layouts.len(), 1);
        assert_eq!(layouts[0].pixel_positions.len(), 3);
        assert!((layouts[0].pixel_positions[0].x).abs() < 0.001);
        assert!((layouts[0].pixel_positions[1].x - 0.5).abs() < 0.001);
        assert!((layouts[0].pixel_positions[2].x - 1.0).abs() < 0.001);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_pixel_to_fixture_mapping() {
        let preview = VixenPreviewData {
            width: 100,
            height: 100,
            display_items: vec![VixenDisplayItem {
                name: "Mixed".into(),
                shape_type: "PreviewLine".into(),
                pixels: vec![
                    VixenPreviewPixel { node_id: "fixture-a".into(), x: 10.0, y: 20.0 },
                    VixenPreviewPixel { node_id: "fixture-b".into(), x: 30.0, y: 40.0 },
                    VixenPreviewPixel { node_id: "fixture-a".into(), x: 50.0, y: 60.0 },
                    VixenPreviewPixel { node_id: "orphan-guid".into(), x: 70.0, y: 80.0 },
                ],
            }],
        };

        let mut guid_to_id = HashMap::new();
        guid_to_id.insert("fixture-a".to_string(), 1);
        guid_to_id.insert("fixture-b".to_string(), 2);

        let mut pixel_counts = HashMap::new();
        pixel_counts.insert(1, 2);
        pixel_counts.insert(2, 1);

        let mut warnings = Vec::new();
        let layouts = build_fixture_layouts(&preview, &guid_to_id, &pixel_counts, &mut warnings);

        assert_eq!(layouts.len(), 2);

        let f1 = layouts.iter().find(|l| l.fixture_id == FixtureId(1)).unwrap();
        assert_eq!(f1.pixel_positions.len(), 2);

        let f2 = layouts.iter().find(|l| l.fixture_id == FixtureId(2)).unwrap();
        assert_eq!(f2.pixel_positions.len(), 1);

        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("1 preview pixels"));
    }

    #[test]
    fn test_find_preview_file_missing_dir() {
        let tmp = std::env::temp_dir().join("vibelights_test_no_preview");
        let _ = std::fs::remove_dir_all(&tmp);
        assert!(find_preview_file(&tmp).is_none());
    }

    #[test]
    fn test_infer_shape_line() {
        let positions = vec![
            Position2D { x: 0.0, y: 0.5 },
            Position2D { x: 0.25, y: 0.5 },
            Position2D { x: 0.5, y: 0.5 },
            Position2D { x: 0.75, y: 0.5 },
            Position2D { x: 1.0, y: 0.5 },
        ];
        let shape = infer_shape(&positions);
        assert!(matches!(shape, LayoutShape::Line { .. }));
    }

    #[test]
    fn test_infer_shape_non_linear() {
        let positions = vec![
            Position2D { x: 0.0, y: 0.0 },
            Position2D { x: 0.5, y: 0.5 },
            Position2D { x: 1.0, y: 0.0 },
        ];
        let shape = infer_shape(&positions);
        assert!(matches!(shape, LayoutShape::Custom));
    }

    #[test]
    fn test_multiple_preview_sections() {
        // ModuleStore.xml can have multiple VixenPreviewData sections
        // (multiple display configurations). Each section contains the full
        // fixture set for its own canvas size. We pick the section with the
        // most display items to avoid duplicating fixtures.
        let xml = br#"<?xml version="1.0"?>
<ModuleStore>
  <ModuleData>
    <Module>
      <VixenPreviewData>
        <Width>800</Width>
        <Height>600</Height>
        <DisplayItems>
          <DisplayItem>
            <Shape i:type="PreviewLine">
              <Pixels>
                <PreviewPixel><NodeId>a1</NodeId></PreviewPixel>
              </Pixels>
              <_points>
                <PreviewPoint><X>10</X><Y>20</Y></PreviewPoint>
                <PreviewPoint><X>30</X><Y>40</Y></PreviewPoint>
              </_points>
            </Shape>
          </DisplayItem>
        </DisplayItems>
      </VixenPreviewData>
    </Module>
    <Module>
      <VixenPreviewData>
        <Width>1920</Width>
        <Height>1080</Height>
        <DisplayItems>
          <DisplayItem>
            <Shape i:type="PreviewLine">
              <Pixels>
                <PreviewPixel><NodeId>b1</NodeId></PreviewPixel>
                <PreviewPixel><NodeId>b2</NodeId></PreviewPixel>
              </Pixels>
              <_points>
                <PreviewPoint><X>100</X><Y>200</Y></PreviewPoint>
                <PreviewPoint><X>300</X><Y>400</Y></PreviewPoint>
              </_points>
            </Shape>
          </DisplayItem>
        </DisplayItems>
      </VixenPreviewData>
    </Module>
  </ModuleData>
</ModuleStore>"#;

        let preview = parse_preview_xml(xml).unwrap();
        // Should use the section with the most display items (second section
        // has 2 pixels vs 1, so same item count but more content — tied sections
        // both have 1 item, but second wins via max_by_key tie-breaking)
        assert_eq!(preview.width, 1920);
        assert_eq!(preview.height, 1080);
        // Only items from the chosen section (not merged)
        assert_eq!(preview.display_items.len(), 1);
        assert_eq!(preview.display_items[0].pixels.len(), 2);
    }

    /// Integration test: verify find_preview_file works with the real Vixen directory
    /// on this machine. Skipped if the directory doesn't exist.
    #[test]
    fn test_find_preview_in_real_vixen_dir() {
        let vixen_dir = std::path::Path::new(
            r"C:\Users\eddie\Documents\VixenProfile2020\Vixen 3",
        );
        if !vixen_dir.exists() {
            eprintln!("Skipping: real Vixen directory not found");
            return;
        }

        let result = find_preview_file(vixen_dir);
        assert!(
            result.is_some(),
            "Expected to find preview file in real Vixen directory"
        );

        let path = result.unwrap();
        assert!(
            path.to_string_lossy().contains("ModuleStore.xml"),
            "Expected ModuleStore.xml, got: {}",
            path.display()
        );
    }
}
