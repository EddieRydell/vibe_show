use serde::Serialize;

/// Get the serde-serialized name of a unit enum variant.
/// Uses serde's own rules (rename_all, rename, etc.) as the single source of truth.
/// Returns `None` for non-string variants (data-carrying variants).
pub fn serde_variant_name<T: Serialize>(val: &T) -> Option<String> {
    match serde_json::to_value(val) {
        Ok(serde_json::Value::String(s)) => Some(s),
        _ => None,
    }
}

/// Get serde-serialized names for all variants of an enum.
/// Filters out any non-string variants (data-carrying variants).
pub fn serde_variant_names<T: Serialize>(variants: &[T]) -> Vec<String> {
    variants.iter().filter_map(serde_variant_name).collect()
}

/// Deserialize a string into an enum variant using serde's own rules.
/// Single source of truth: uses the same rename/rename_all config as normal deserialization.
pub fn from_serde_str<T: for<'de> serde::Deserialize<'de>>(s: &str) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(s.to_string())).ok()
}

/// Encode raw bytes as base64.
#[allow(clippy::indexing_slicing)] // chunk[0] is safe: chunks(3) guarantees >= 1 element.
                                   // CHARS[..] indices are masked to 0..63 and CHARS has 64 elements.
pub fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(chunk.get(1).copied().unwrap_or(0));
        let b2 = u32::from(chunk.get(2).copied().unwrap_or(0));
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[(triple >> 18 & 0x3F) as usize] as char);
        result.push(CHARS[(triple >> 12 & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[(triple >> 6 & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

/// Decode base64-encoded data into raw bytes.
#[allow(clippy::indexing_slicing)] // chunk[0] and chunk[1] are safe: we break when chunk.len() < 2.
                                   // chunk[2] and chunk[3] are guarded by explicit length checks.
pub fn base64_decode(s: &str) -> Vec<u8> {
    fn val(c: u8) -> u8 {
        match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => 0,
        }
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        if chunk.len() < 2 {
            break;
        }
        let a = u32::from(val(chunk[0]));
        let b = u32::from(val(chunk[1]));
        let c = if chunk.len() > 2 && chunk[2] != b'=' { u32::from(val(chunk[2])) } else { 0 };
        let d = if chunk.len() > 3 && chunk[3] != b'=' { u32::from(val(chunk[3])) } else { 0 };
        let triple = (a << 18) | (b << 12) | (c << 6) | d;
        #[allow(clippy::cast_possible_truncation)] // masked to 8 bits
        {
            out.push((triple >> 16) as u8);
            if chunk.len() > 2 && chunk[2] != b'=' {
                out.push((triple >> 8 & 0xFF) as u8);
            }
            if chunk.len() > 3 && chunk[3] != b'=' {
                out.push((triple & 0xFF) as u8);
            }
        }
    }
    out
}
