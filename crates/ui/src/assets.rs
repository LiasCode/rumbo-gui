//! The files the components draw: Lucide's icons, those they use and no
//! more, under the ISC license in `assets/icons/LICENSE`.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

/// Every file, by the path an element asks for.
const FILES: [(&str, &[u8]); 12] = [
    (
        "icons/arrow-up.svg",
        include_bytes!("../assets/icons/arrow-up.svg"),
    ),
    (
        "icons/check.svg",
        include_bytes!("../assets/icons/check.svg"),
    ),
    (
        "icons/chevron-down.svg",
        include_bytes!("../assets/icons/chevron-down.svg"),
    ),
    (
        "icons/chevron-up.svg",
        include_bytes!("../assets/icons/chevron-up.svg"),
    ),
    ("icons/copy.svg", include_bytes!("../assets/icons/copy.svg")),
    ("icons/info.svg", include_bytes!("../assets/icons/info.svg")),
    (
        "icons/minus.svg",
        include_bytes!("../assets/icons/minus.svg"),
    ),
    ("icons/plus.svg", include_bytes!("../assets/icons/plus.svg")),
    (
        "icons/search.svg",
        include_bytes!("../assets/icons/search.svg"),
    ),
    (
        "icons/settings.svg",
        include_bytes!("../assets/icons/settings.svg"),
    ),
    (
        "icons/trash.svg",
        include_bytes!("../assets/icons/trash.svg"),
    ),
    ("icons/x.svg", include_bytes!("../assets/icons/x.svg")),
];

/// What an application gives GPUI for the components to find their files.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(FILES
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(FILES
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| SharedString::new_static(name))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icon::IconName;

    #[test]
    fn holds_every_icon_the_components_name() {
        for icon in IconName::ALL {
            let loaded = Assets.load(icon.path()).unwrap();

            assert!(loaded.is_some(), "{} is missing", icon.path());
        }
    }

    #[test]
    fn lists_the_files_under_a_folder() {
        let listed = Assets.list("icons/").unwrap();

        assert_eq!(listed.len(), FILES.len());
        assert!(Assets.list("fonts/").unwrap().is_empty());
    }
}
