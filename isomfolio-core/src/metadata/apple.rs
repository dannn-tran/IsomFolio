use super::{AppleMetadata, AppleTag};

const ATTR_USER_TAGS: &str = "com.apple.metadata:_kMDItemUserTags";

pub fn read_apple_metadata(file_path: &str) -> Option<AppleMetadata> {
    let bytes = read_xattr(file_path, ATTR_USER_TAGS)?;
    let tags = parse_tags_bplist(&bytes);
    if tags.is_empty() {
        None
    } else {
        Some(AppleMetadata { user_tags: tags })
    }
}

fn read_xattr(path: &str, attr: &str) -> Option<Vec<u8>> {
    use std::ffi::CString;
    use std::os::raw::c_int;

    let c_path = CString::new(path).ok()?;
    let c_attr = CString::new(attr).ok()?;

    extern "C" {
        fn getxattr(
            path: *const libc::c_char,
            name: *const libc::c_char,
            value: *mut libc::c_void,
            size: libc::size_t,
            position: u32,
            options: c_int,
        ) -> libc::ssize_t;
    }

    let size = unsafe {
        getxattr(
            c_path.as_ptr(),
            c_attr.as_ptr(),
            std::ptr::null_mut(),
            0,
            0,
            0,
        )
    };

    if size < 0 {
        return None;
    }

    let buf_size = size as usize + 16;
    let mut buf = vec![0u8; buf_size];
    let read = unsafe {
        getxattr(
            c_path.as_ptr(),
            c_attr.as_ptr(),
            buf.as_mut_ptr() as *mut libc::c_void,
            buf_size,
            0,
            0,
        )
    };

    if read < 0 {
        return None;
    }

    buf.truncate(read as usize);
    Some(buf)
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
