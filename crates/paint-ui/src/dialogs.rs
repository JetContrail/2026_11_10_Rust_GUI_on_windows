use crate::i18n::tr;
use std::fmt;
use std::path::PathBuf;

/// File dialogs used by the UI (replaced by a fake in E2E tests).
pub trait FileDialogs: fmt::Debug {
    fn open_image(&self) -> Option<PathBuf>;
    fn open_project(&self) -> Option<PathBuf>;
    fn save_project(&self) -> Option<PathBuf>;
    fn export_png(&self) -> Option<PathBuf>;
}

/// Native Windows dialogs (via `rfd`).
#[derive(Debug, Default, Clone, Copy)]
pub struct NativeDialogs;

impl FileDialogs for NativeDialogs {
    fn open_image(&self) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter(tr().filter_images, &["png", "jpg", "jpeg", "bmp"])
            .pick_file()
    }

    fn open_project(&self) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter(tr().filter_project, &[paint_persistence::PROJECT_EXTENSION])
            .pick_file()
    }

    fn save_project(&self) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter(tr().filter_project, &[paint_persistence::PROJECT_EXTENSION])
            .set_file_name("untitled.paint")
            .save_file()
            .map(|p| with_extension(p, paint_persistence::PROJECT_EXTENSION))
    }

    fn export_png(&self) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter(tr().filter_png, &["png"])
            .set_file_name("image.png")
            .save_file()
            .map(|p| with_extension(p, "png"))
    }
}

/// Appends `ext` when the user typed a name without it.
pub(crate) fn with_extension(path: PathBuf, ext: &str) -> PathBuf {
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
    {
        path
    } else {
        let mut s = path.into_os_string();
        s.push(".");
        s.push(ext);
        s.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_missing_extension() {
        assert_eq!(
            with_extension("a".into(), "paint"),
            PathBuf::from("a.paint")
        );
        assert_eq!(
            with_extension("a.PAINT".into(), "paint"),
            PathBuf::from("a.PAINT")
        );
        assert_eq!(
            with_extension("a.txt".into(), "png"),
            PathBuf::from("a.txt.png")
        );
    }
}
