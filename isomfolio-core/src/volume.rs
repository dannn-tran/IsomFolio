//! Cross-platform volume identity. Maps a filesystem path to the **stable**
//! identifier of the volume it lives on, so a file's identity can survive a
//! removable drive remounting under a different mount point (macOS/Linux) or
//! drive letter (Windows). Best-effort: when the platform or filesystem can't
//! supply a stable id, [`resolve`] returns `None` and callers fall back to
//! path-based identity (the historical behaviour).
//!
//! Resolution is snapshot-based: the set of mounted volumes is enumerated once
//! and cached briefly, so resolving thousands of files during a scan is a string
//! prefix-match, not a syscall/subprocess per file.

use std::path::MAIN_SEPARATOR;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VolumeInfo {
    /// Stable volume identifier (filesystem UUID on macOS/Linux, the volume GUID
    /// path on Windows). Survives remounts and drive-letter reassignment.
    pub uuid: String,
    /// Where the volume is currently mounted.
    pub mount_point: String,
}

const SNAPSHOT_TTL: Duration = Duration::from_secs(2);

fn cache() -> &'static Mutex<Option<(Instant, Vec<VolumeInfo>)>> {
    static C: OnceLock<Mutex<Option<(Instant, Vec<VolumeInfo>)>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}

fn volumes() -> Vec<VolumeInfo> {
    let mut guard = cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, vols)) = guard.as_ref() {
        if at.elapsed() < SNAPSHOT_TTL {
            return vols.clone();
        }
    }
    let vols = platform::enumerate();
    *guard = Some((Instant::now(), vols.clone()));
    vols
}

/// Drop the cached volume snapshot so the next [`resolve`] re-enumerates. Call
/// after a known mount change (e.g. a drive came back online).
pub fn invalidate_cache() {
    *cache().lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// The volume containing `path` — the mounted volume whose mount point is the
/// longest prefix of `path`. `None` when no known volume contains it.
pub fn resolve(path: &str) -> Option<VolumeInfo> {
    let mut best: Option<VolumeInfo> = None;
    for v in volumes() {
        if path_under(path, &v.mount_point)
            && best
                .as_ref()
                .map_or(true, |b| v.mount_point.len() > b.mount_point.len())
        {
            best = Some(v);
        }
    }
    best
}

/// Whether a path on this mount should be keyed by volume rather than by its
/// absolute path. The boot volume's mount point is stable, so keying it buys
/// nothing on Unix; on Windows even the system drive letter can move, so every
/// volume is keyed.
pub fn should_key_volume(mount_point: &str) -> bool {
    if cfg!(windows) {
        true
    } else {
        mount_point != "/"
    }
}

fn path_under(path: &str, mount: &str) -> bool {
    if mount == "/" {
        return path.starts_with('/');
    }
    let m = mount.trim_end_matches(MAIN_SEPARATOR);
    path == m || path.starts_with(&format!("{m}{MAIN_SEPARATOR}"))
}

/// Directories where volumes mount, to watch (non-recursively) for drives
/// appearing/disappearing — the event source for removable-drive detection.
/// Only existing directories are returned. Empty on platforms without a mount
/// container (Windows: drive letters, no directory to watch — poll instead).
pub fn mount_watch_dirs() -> Vec<String> {
    let mut candidates: Vec<String> = Vec::new();
    if cfg!(target_os = "macos") {
        candidates.push("/Volumes".into());
    } else if cfg!(target_os = "linux") {
        candidates.push("/media".into());
        candidates.push("/mnt".into());
        // udisks auto-mounts removable media under /run/media/<user>/<label> and
        // some distros under /media/<user>/<label>; watch the per-user dir so the
        // label-level mount is a direct child (non-recursive watch).
        if let Some(user) = std::env::var_os("USER").and_then(|u| u.into_string().ok()) {
            candidates.push(format!("/run/media/{user}"));
            candidates.push(format!("/media/{user}"));
        }
    }
    candidates
        .into_iter()
        .filter(|d| std::path::Path::new(d).is_dir())
        .collect()
}

/// The volume-relative remainder of `path` under `mount` (no leading separator).
pub fn relative_to_mount(path: &str, mount: &str) -> String {
    let m = mount.trim_end_matches(MAIN_SEPARATOR);
    path.strip_prefix(m)
        .unwrap_or(path)
        .trim_start_matches(MAIN_SEPARATOR)
        .to_string()
}

#[cfg(target_os = "macos")]
mod platform {
    use super::VolumeInfo;

    pub fn enumerate() -> Vec<VolumeInfo> {
        let mut out = Vec::new();
        if let Some(uuid) = volume_uuid("/") {
            out.push(VolumeInfo { uuid, mount_point: "/".into() });
        }
        // Every mounted volume (including the boot volume's alias) appears under
        // /Volumes; each entry is a mount point.
        if let Ok(entries) = std::fs::read_dir("/Volumes") {
            for entry in entries.flatten() {
                let mp = entry.path().to_string_lossy().into_owned();
                if let Some(uuid) = volume_uuid(&mp) {
                    out.push(VolumeInfo { uuid, mount_point: mp });
                }
            }
        }
        out
    }

    fn volume_uuid(mount: &str) -> Option<String> {
        let output = std::process::Command::new("diskutil")
            .args(["info", "-plist", mount])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let value: plist::Value = plist::from_bytes(&output.stdout).ok()?;
        value
            .as_dictionary()
            .and_then(|d| d.get("VolumeUUID"))
            .and_then(|v| v.as_string())
            .map(str::to_string)
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::VolumeInfo;
    use std::collections::HashMap;

    pub fn enumerate() -> Vec<VolumeInfo> {
        let dev_to_uuid = uuid_map();
        let mut out = Vec::new();
        let Ok(mounts) = std::fs::read_to_string("/proc/mounts") else {
            return out;
        };
        for line in mounts.lines() {
            let mut fields = line.split_whitespace();
            let (Some(dev), Some(mp)) = (fields.next(), fields.next()) else {
                continue;
            };
            if !dev.starts_with("/dev/") {
                continue;
            }
            let canon = std::fs::canonicalize(dev)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| dev.to_string());
            if let Some(uuid) = dev_to_uuid.get(&canon) {
                out.push(VolumeInfo { uuid: uuid.clone(), mount_point: unescape(mp) });
            }
        }
        out
    }

    /// Map canonical device node → filesystem UUID via `/dev/disk/by-uuid`.
    fn uuid_map() -> HashMap<String, String> {
        let mut map = HashMap::new();
        if let Ok(entries) = std::fs::read_dir("/dev/disk/by-uuid") {
            for entry in entries.flatten() {
                let uuid = entry.file_name().to_string_lossy().into_owned();
                if let Ok(target) = std::fs::canonicalize(entry.path()) {
                    map.insert(target.to_string_lossy().into_owned(), uuid);
                }
            }
        }
        map
    }

    /// `/proc/mounts` octal-escapes space/tab/newline/backslash in mount points.
    fn unescape(s: &str) -> String {
        s.replace("\\040", " ")
            .replace("\\011", "\t")
            .replace("\\012", "\n")
            .replace("\\134", "\\")
    }
}

#[cfg(windows)]
mod platform {
    use super::VolumeInfo;
    use windows_sys::Win32::Storage::FileSystem::{
        GetLogicalDrives, GetVolumeNameForVolumeMountPointW,
    };

    pub fn enumerate() -> Vec<VolumeInfo> {
        let mut out = Vec::new();
        let mask = unsafe { GetLogicalDrives() };
        for i in 0..26u32 {
            if mask & (1 << i) == 0 {
                continue;
            }
            let root = format!("{}:\\", (b'A' + i as u8) as char);
            if let Some(guid) = volume_guid(&root) {
                out.push(VolumeInfo { uuid: guid, mount_point: root });
            }
        }
        out
    }

    /// The `\\?\Volume{GUID}\` name — stable across drive-letter reassignment.
    fn volume_guid(root: &str) -> Option<String> {
        let wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
        let mut buf = [0u16; 64];
        let ok = unsafe {
            GetVolumeNameForVolumeMountPointW(wide.as_ptr(), buf.as_mut_ptr(), buf.len() as u32)
        };
        if ok == 0 {
            return None;
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..len]))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
mod platform {
    pub fn enumerate() -> Vec<super::VolumeInfo> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longest_mount_prefix_wins() {
        *cache().lock().unwrap() = Some((
            Instant::now(),
            vec![
                VolumeInfo { uuid: "boot".into(), mount_point: "/".into() },
                VolumeInfo { uuid: "ext".into(), mount_point: "/Volumes/SD".into() },
            ],
        ));
        assert_eq!(resolve("/Volumes/SD/DCIM/x.jpg").unwrap().uuid, "ext");
        assert_eq!(resolve("/Users/me/p.jpg").unwrap().uuid, "boot");
        invalidate_cache();
    }

    #[test]
    fn relative_strips_mount_and_separator() {
        assert_eq!(relative_to_mount("/Volumes/SD/DCIM/x.jpg", "/Volumes/SD"), "DCIM/x.jpg");
        assert_eq!(relative_to_mount("/Volumes/SD", "/Volumes/SD"), "");
    }

    #[test]
    fn boot_volume_is_not_keyed_on_unix() {
        // Platform-independent guard for the keying policy on Unix targets.
        #[cfg(not(windows))]
        {
            assert!(!should_key_volume("/"));
            assert!(should_key_volume("/Volumes/SD"));
        }
    }
}
