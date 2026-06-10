//! Non-destructive file copy helpers used when exporting albums/groups to a
//! folder on disk. "Non-destructive" means: existing directories are merged
//! into (never cleared), and a name collision never overwrites — a numeric
//! suffix is added instead.

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};

/// Copy `src` into directory `dest_dir`, creating `dest_dir` and any missing
/// parents first. If a file of the same name already exists there, a numeric
/// suffix is inserted before the extension — `photo.jpg` → `photo (1).jpg` →
/// `photo (2).jpg` … — so an existing file is never overwritten. Returns the
/// path actually written.
pub fn copy_into_dir(src: &Path, dest_dir: &Path) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dest_dir)?;
    let file_name = src.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, format!("no file name in {}", src.display()))
    })?;
    let target = non_colliding_path(dest_dir, file_name);
    std::fs::copy(src, &target)?;
    Ok(target)
}

/// First non-existing path of the form `dir/<name>`, `dir/<stem> (1).<ext>`,
/// `dir/<stem> (2).<ext>`, … Pure filesystem probing; no I/O beyond `exists`.
pub fn non_colliding_path(dir: &Path, file_name: &OsStr) -> PathBuf {
    let candidate = dir.join(file_name);
    if !candidate.exists() {
        return candidate;
    }
    let name = Path::new(file_name);
    let stem = name.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = name.extension().map(|e| e.to_string_lossy().into_owned());
    for n in 1u32.. {
        let candidate = match &ext {
            Some(ext) => format!("{stem} ({n}).{ext}"),
            None => format!("{stem} ({n})"),
        };
        let p = dir.join(candidate);
        if !p.exists() {
            return p;
        }
    }
    unreachable!("exhausted u32 suffixes for {}", dir.display())
}

/// Sanitise an album/group name into a single safe folder-name component:
/// path separators and characters illegal on common filesystems are replaced
/// with `-`, surrounding whitespace/dots trimmed, and an empty result falls
/// back to `Untitled`.
pub fn sanitize_component(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').trim();
    if trimmed.is_empty() {
        "Untitled".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmpdir() -> PathBuf {
        let p = std::env::temp_dir().join(format!("isomfolio-fileops-{}", uuid()));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn uuid() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static C: AtomicU64 = AtomicU64::new(0);
        let n = C.fetch_add(1, Ordering::Relaxed);
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("{t:x}-{n:x}")
    }

    mod non_colliding_path {
        use super::*;

        #[test]
        fn returns_plain_path_when_free() {
            let dir = tmpdir();
            let p = non_colliding_path(&dir, OsStr::new("a.jpg"));
            assert_eq!(p, dir.join("a.jpg"));
        }

        #[test]
        fn suffixes_before_extension_on_collision() {
            let dir = tmpdir();
            fs::write(dir.join("a.jpg"), b"x").unwrap();
            let p = non_colliding_path(&dir, OsStr::new("a.jpg"));
            assert_eq!(p, dir.join("a (1).jpg"));
        }

        #[test]
        fn increments_suffix_past_existing_suffixed() {
            let dir = tmpdir();
            fs::write(dir.join("a.jpg"), b"x").unwrap();
            fs::write(dir.join("a (1).jpg"), b"x").unwrap();
            let p = non_colliding_path(&dir, OsStr::new("a.jpg"));
            assert_eq!(p, dir.join("a (2).jpg"));
        }

        #[test]
        fn handles_extensionless_names() {
            let dir = tmpdir();
            fs::write(dir.join("README"), b"x").unwrap();
            let p = non_colliding_path(&dir, OsStr::new("README"));
            assert_eq!(p, dir.join("README (1)"));
        }
    }

    mod copy_into_dir {
        use super::*;

        #[test]
        fn creates_missing_dirs_and_copies() {
            let root = tmpdir();
            let src = root.join("src.txt");
            fs::write(&src, b"hello").unwrap();
            let nested = root.join("Group").join("Album");
            let written = copy_into_dir(&src, &nested).unwrap();
            assert_eq!(written, nested.join("src.txt"));
            assert_eq!(fs::read(&written).unwrap(), b"hello");
        }

        #[test]
        fn never_overwrites_existing() {
            let root = tmpdir();
            let src = root.join("src.txt");
            fs::write(&src, b"new").unwrap();
            let dest = root.join("out");
            fs::create_dir_all(&dest).unwrap();
            fs::write(dest.join("src.txt"), b"original").unwrap();
            let written = copy_into_dir(&src, &dest).unwrap();
            assert_eq!(written, dest.join("src (1).txt"));
            assert_eq!(fs::read(dest.join("src.txt")).unwrap(), b"original");
            assert_eq!(fs::read(&written).unwrap(), b"new");
        }
    }

    mod sanitize_component {
        use super::*;

        #[test]
        fn replaces_separators_and_illegal_chars() {
            assert_eq!(sanitize_component("2024/Best: \"picks\""), "2024-Best- -picks-");
        }

        #[test]
        fn falls_back_for_empty_or_dots() {
            assert_eq!(sanitize_component("   "), "Untitled");
            assert_eq!(sanitize_component("..."), "Untitled");
        }

        #[test]
        fn keeps_ordinary_names() {
            assert_eq!(sanitize_component("Wedding 2024"), "Wedding 2024");
        }
    }
}
