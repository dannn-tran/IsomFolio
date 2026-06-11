use iced::{Point, Task};

use super::super::{loupe, App, LoupeLoadError, Msg};

impl App {
    pub(crate) fn load_loupe_full_res(&self) -> Task<Msg> {
        let idx = self.loupe.idx;
        let Some(file) = self.files.get(idx) else { return Task::none() };
        let path = file.disk_path();
        let filename = file.name.clone();
        let fallback_name = filename.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || match decode_image_for_display(&path, false) {
                    Some(handle) => Ok(handle),
                    None => Err(diagnose_load_failure(&path, &filename)),
                })
                .await
                .unwrap_or_else(|_| {
                    Err(LoupeLoadError {
                        filename: fallback_name,
                        message: "Decoding the image crashed.".into(),
                        permission: false,
                    })
                })
            },
            move |result| match result {
                Ok(handle) => Msg::LoupeFullResLoaded { idx, handle },
                Err(error) => Msg::LoupeFullResFailed { idx, error },
            },
        )
    }

    /// Live loupe geometry from the last hover-reported sizes, if known. Used
    /// for centre-anchored button/key zoom; the widget builds its own from the
    /// current layout for pointer-anchored gestures.
    fn loupe_geometry(&self) -> Option<loupe::LoupeGeometry> {
        match (self.loupe.viewport, self.loupe.native) {
            (Some(viewport), Some(native))
                if viewport.width > 0.0 && viewport.height > 0.0 && native.width > 0.0 =>
            {
                Some(loupe::LoupeGeometry { viewport, native })
            }
            _ => None,
        }
    }

    pub(super) fn loupe_center(&self) -> Point {
        self.loupe_geometry().map(|g| g.center()).unwrap_or(Point::ORIGIN)
    }

    /// Apply a loupe intent app-side (buttons / keys) through the shared reducer,
    /// then load the hi-res decode if we ended up zoomed in. When geometry isn't
    /// known yet, falls back to a geometry-free approximation the widget will
    /// re-clamp on its next draw.
    pub(super) fn apply_loupe_intent(&mut self, intent: loupe::LoupeIntent) -> Task<Msg> {
        let prev = self.loupe.zoom;
        let cur = loupe::LoupeZoom { zoom: self.loupe.zoom, offset: self.loupe.pan };
        let next = match self.loupe_geometry() {
            Some(geo) => geo.apply(cur, intent),
            None => fallback_apply(cur, intent),
        };
        self.loupe.zoom = next.zoom;
        self.loupe.pan = next.offset;
        if next.zoom <= super::super::LOUPE_ZOOM_MIN {
            // Back at fit: the next zoom-in must re-decode the hi-res image.
            self.loupe.hires_loaded = false;
        }
        if next.zoom > super::super::LOUPE_ZOOM_MIN && next.zoom != prev {
            return self.load_loupe_hires();
        }
        Task::none()
    }

    /// Full-demosaic decode for the current RAW, swapped in when the user zooms
    /// to 100% so the focus check is pixel-accurate. No-op for non-RAW (already
    /// full quality) or once already loaded for this photo.
    pub(crate) fn load_loupe_hires(&self) -> Task<Msg> {
        if self.loupe.hires_loaded {
            return Task::none();
        }
        let idx = self.loupe.idx;
        let Some(file) = self.files.get(idx) else { return Task::none() };
        if !is_raw_path(&file.path) {
            return Task::none();
        }
        let path = file.disk_path();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || decode_image_for_display(&path, true))
                    .await
                    .ok()
                    .flatten()
            },
            move |handle_opt| match handle_opt {
                Some(handle) => Msg::LoupeHiresLoaded { idx, handle },
                None => Msg::NoOp,
            },
        )
    }

    pub(crate) fn load_compare_slot(&self, slot: usize) -> Task<Msg> {
        let Some(file) = self.compare.files[slot].as_ref() else { return Task::none() };
        let path = file.disk_path();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || decode_image_for_display(&path, false))
                    .await
                    .ok()
                    .flatten()
            },
            move |handle_opt| match handle_opt {
                Some(handle) => Msg::CompareFullResLoaded { slot, handle },
                None => Msg::NoOp,
            },
        )
    }

    pub(crate) fn load_loupe_prefetch(&self) -> Task<Msg> {
        let total = self.files.len();
        if total == 0 {
            return Task::none();
        }
        let current = self.loupe.idx;
        let mut tasks = Vec::new();
        for delta in [-1i32, 1] {
            let idx = (current as i32 + delta).rem_euclid(total as i32) as usize;
            if self.loupe.prefetch.contains_key(&idx) {
                continue;
            }
            if self.loupe.full_res.as_ref().map_or(false, |(i, _)| *i == idx) {
                continue;
            }
            if let Some(file) = self.files.get(idx) {
                let path = file.disk_path();
                tasks.push(Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || decode_image_for_display(&path, false))
                            .await
                            .ok()
                            .flatten()
                    },
                    move |handle_opt| match handle_opt {
                        Some(handle) => Msg::LoupePrefetchLoaded { idx, handle },
                        None => Msg::NoOp,
                    },
                ));
            }
        }
        Task::batch(tasks)
    }
}

/// Geometry-free intent application for the brief window after the loupe opens
/// but before the widget has reported its size. Anchoring is impossible without
/// geometry, so this only sets the zoom and scales the pan toward centre; the
/// widget re-clamps to the image edges on its next draw.
fn fallback_apply(cur: loupe::LoupeZoom, intent: loupe::LoupeIntent) -> loupe::LoupeZoom {
    use loupe::{LoupeIntent, LoupeZoom, ZoomLevel, ZOOM_MAX, ZOOM_MIN};
    let scale_to = |to: f32, cur: LoupeZoom| {
        let to = to.clamp(ZOOM_MIN, ZOOM_MAX);
        if to <= ZOOM_MIN {
            LoupeZoom { zoom: ZOOM_MIN, offset: iced::Vector::ZERO }
        } else {
            LoupeZoom { zoom: to, offset: cur.offset * (to / cur.zoom) }
        }
    };
    match intent {
        LoupeIntent::ZoomAround { factor, .. } => scale_to(cur.zoom * factor, cur),
        // No geometry → no true 1:1; fall back to 2× (matches the old behaviour).
        LoupeIntent::ZoomTo { level: ZoomLevel::Actual, .. } => {
            LoupeZoom { zoom: 2.0_f32.clamp(ZOOM_MIN, ZOOM_MAX), offset: iced::Vector::ZERO }
        }
        LoupeIntent::ZoomTo { level: ZoomLevel::Fit, .. } | LoupeIntent::Reset => {
            LoupeZoom { zoom: ZOOM_MIN, offset: iced::Vector::ZERO }
        }
        LoupeIntent::PanTo(offset) => LoupeZoom { zoom: cur.zoom, offset },
    }
}

pub(crate) fn is_raw_path(path: &str) -> bool {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    isomfolio_core::indexing::thumbnail::is_raw_extension(ext)
}

pub(crate) fn decode_image_for_display(path: &str, prefer_full: bool) -> Option<iced::widget::image::Handle> {
    let img = open_image(path, prefer_full)?;
    let rgba = img.into_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    Some(iced::widget::image::Handle::from_rgba(w, h, rgba.into_raw()))
}

/// Classify why a full-res decode produced nothing, into a user-facing reason +
/// a `permission` flag that drives the resolution action. Distinguishes a
/// permission denial (macOS TCC on a protected folder) from a missing file or an
/// unsupported/corrupt image by probing the raw file open.
fn diagnose_load_failure(path: &str, filename: &str) -> LoupeLoadError {
    use std::io::ErrorKind;
    let (message, permission) = match std::fs::File::open(path) {
        Err(e) if e.kind() == ErrorKind::PermissionDenied => (
            "macOS blocked access to this file. It's in a protected folder \
             (Downloads, Desktop, Documents). Grant the app Full Disk Access, \
             then reopen the photo."
                .to_string(),
            true,
        ),
        Err(e) if e.kind() == ErrorKind::NotFound => {
            ("The file is no longer at its expected location.".to_string(), false)
        }
        Err(e) => (format!("Couldn't open the file: {e}."), false),
        // Opened fine, so the decoder rejected the contents.
        Ok(_) => ("The image data is unsupported or corrupt.".to_string(), false),
    };
    LoupeLoadError { filename: filename.to_string(), message, permission }
}

/// Open the OS privacy pane where file-access is granted. macOS deep-links to
/// Full Disk Access; other platforms have no equivalent one-click target.
pub(super) fn open_privacy_settings() {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
            .spawn();
    }
}

/// Decode an image for on-screen display. For RAW, `prefer_full = false` returns
/// the embedded preview first (fast — used for fit-to-window browsing and
/// prefetch), and only falls back to a full demosaic if no preview exists.
/// `prefer_full = true` does the full demosaic (used when zoomed to 100% for a
/// pixel-accurate focus check). Non-RAW formats ignore the flag.
fn open_image(path: &str, prefer_full: bool) -> Option<image::DynamicImage> {
    use rawler::decoders::RawDecodeParams;
    use rawler::rawsource::RawSource;
    use std::path::Path;

    if is_raw_path(path) {
        let source = RawSource::new(Path::new(path)).ok()?;
        let decoder = rawler::get_decoder(&source).ok()?;
        let params = RawDecodeParams::default();
        let full = || decoder.full_image(&source, &params).ok().flatten();
        let preview = || decoder.preview_image(&source, &params).ok().flatten();
        return if prefer_full {
            full().or_else(preview)
        } else {
            preview().or_else(full)
        };
    }

    match image::open(path) {
        Ok(img) => Some(img),
        Err(e) => {
            // Common cause on macOS: the file is in a TCC-protected folder
            // (~/Downloads, ~/Desktop, ~/Documents) the app lacks access to —
            // the read fails with "Operation not permitted". The loupe then
            // falls back to the cached thumbnail (looks pixelated) and can't zoom.
            eprintln!("[loupe] cannot read {path}: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod diagnose_load_failure_fn {
        use super::*;

        fn temp_path(name: &str) -> std::path::PathBuf {
            let p = std::env::temp_dir().join(format!(
                "isomfolio-test-{}-{name}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            p
        }

        #[test]
        fn missing_file_is_not_a_permission_error() {
            let err = diagnose_load_failure("/no/such/file.jpg", "file.jpg");
            assert!(!err.permission);
            assert_eq!(err.filename, "file.jpg");
            assert!(err.message.to_lowercase().contains("location"));
        }

        #[test]
        fn readable_but_undecodable_file_reports_corrupt_not_permission() {
            let path = temp_path("corrupt.jpg");
            std::fs::write(&path, b"definitely not an image").unwrap();
            let err = diagnose_load_failure(path.to_str().unwrap(), "x.jpg");
            let _ = std::fs::remove_file(&path);
            assert!(!err.permission);
            let m = err.message.to_lowercase();
            assert!(m.contains("unsupported") || m.contains("corrupt"), "got: {}", err.message);
        }

        #[cfg(unix)]
        #[test]
        fn unreadable_file_is_flagged_as_permission() {
            use std::os::unix::fs::PermissionsExt;
            let path = temp_path("denied.jpg");
            std::fs::write(&path, b"x").unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
            // Running as root bypasses the mode bits; only assert when truly denied.
            let denied = std::fs::File::open(&path).is_err();
            let err = diagnose_load_failure(path.to_str().unwrap(), "x.jpg");
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644));
            let _ = std::fs::remove_file(&path);
            if denied {
                assert!(err.permission, "expected permission flag, got: {}", err.message);
            }
        }
    }
}
