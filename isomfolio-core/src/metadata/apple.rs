use super::{AppleMetadata, AppleTag};

const ATTR_USER_TAGS: &str = "com.apple.metadata:_kMDItemUserTags";

pub fn read_apple_metadata(file_path: &str) -> Option<AppleMetadata> {
    let bytes = xattr::get(file_path, ATTR_USER_TAGS).ok().flatten()?;
    let tags = parse_tags_bplist(&bytes);
    if tags.is_empty() { None } else { Some(AppleMetadata { user_tags: tags }) }
}

fn parse_tags_bplist(data: &[u8]) -> Vec<AppleTag> {
    // Use the `plist` crate to decode the binary plist
    match plist::Value::from_reader(std::io::Cursor::new(data)) {
        Ok(plist::Value::Array(arr)) => arr
            .into_iter()
            .filter_map(|v| {
                if let plist::Value::String(s) = v {
                    Some(parse_tag(&s))
                } else {
                    None
                }
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_tag(s: &str) -> AppleTag {
    // macOS format: "TagName\nColorIndex"
    match s.split_once('\n') {
        Some((text, color_str)) => AppleTag {
            text: text.to_string(),
            color_idx: color_str.parse().unwrap_or(0),
        },
        None => AppleTag { text: s.to_string(), color_idx: 0 },
    }
}
