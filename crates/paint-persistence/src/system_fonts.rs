//! Installed system fonts (DirectWrite on Windows, fontconfig on Linux).

use std::fmt;
use std::sync::Mutex;

use fontique::{Collection, CollectionOptions, SourceCache};
use paint_domain::ports::{FontData, FontSource};

/// [`FontSource`] backed by the operating system's font collection.
pub struct SystemFontSource {
    inner: Mutex<(Collection, SourceCache)>,
}

impl fmt::Debug for SystemFontSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SystemFontSource").finish_non_exhaustive()
    }
}

impl Default for SystemFontSource {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemFontSource {
    /// Font enumeration is deferred until the first query.
    pub fn new() -> Self {
        let collection = Collection::new(CollectionOptions {
            shared: false,
            system_fonts: true,
        });
        Self {
            inner: Mutex::new((collection, SourceCache::default())),
        }
    }
}

impl FontSource for SystemFontSource {
    fn family_names(&self) -> Vec<String> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let (collection, cache) = &mut *inner;
        let mut names: Vec<String> = collection
            .family_names()
            // "@Font" entries are vertical-writing aliases on Windows.
            .filter(|n| !n.starts_with('@') && !n.is_empty())
            .map(str::to_owned)
            .collect();
        // Keep only fonts that can actually be loaded (drops e.g. Type 1).
        names.retain(|n| load_family(collection, cache, n).is_some());
        names.sort_by_key(|n| n.to_lowercase());
        names.dedup();
        names
    }

    fn load(&self, family: &str) -> Option<FontData> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let (collection, cache) = &mut *inner;
        load_family(collection, cache, family)
    }
}

fn load_family(
    collection: &mut Collection,
    cache: &mut SourceCache,
    family: &str,
) -> Option<FontData> {
    {
        let info = collection.family_by_name(family)?;
        let font = info.default_font()?;
        let blob = font.load(Some(cache))?;
        let data = blob.as_ref();
        // TrueType ("\0\1\0\0", "true"), OpenType ("OTTO"), collection ("ttcf").
        let scalable = [b"\0\x01\0\0", b"true", b"OTTO", b"ttcf"]
            .iter()
            .any(|magic| data.starts_with(*magic));
        scalable.then(|| FontData {
            bytes: data.into(),
            index: font.index(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_family_is_none_and_listing_does_not_panic() {
        let fonts = SystemFontSource::new();
        assert!(fonts.load("No Such Font Family 12345").is_none());
        let names = fonts.family_names();
        assert!(
            names
                .windows(2)
                .all(|w| w[0].to_lowercase() <= w[1].to_lowercase())
        );
        // Every listed family can be loaded (checked on a few to stay fast).
        for name in names.iter().take(3) {
            assert!(fonts.load(name).is_some(), "{name}");
        }
    }
}
