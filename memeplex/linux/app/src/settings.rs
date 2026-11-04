//! What the app keeps on this device: a key file in its config folder (`settings.ini`, group
//! `linux`). ponytail: until the core keeps settings, with the library's folders.

use gtk::glib;
use std::path::PathBuf;

const GROUP: &str = "linux";

fn file() -> PathBuf {
    glib::user_config_dir().join("memeplex/settings.ini")
}

pub fn get(key: &str) -> Option<String> {
    let keys = glib::KeyFile::new();
    keys.load_from_file(file(), glib::KeyFileFlags::NONE).ok()?;
    keys.string(GROUP, key).ok().map(Into::into)
}

pub fn set(key: &str, value: &str) {
    let keys = glib::KeyFile::new();
    let file = file();
    // Missing on the first save.
    let _ = keys.load_from_file(&file, glib::KeyFileFlags::KEEP_COMMENTS);
    keys.set_string(GROUP, key, value);
    let saved = std::fs::create_dir_all(file.parent().unwrap())
        .map_err(|e| e.to_string())
        .and_then(|_| keys.save_to_file(&file).map_err(|e| e.to_string()));
    if let Err(e) = saved {
        glib::g_warning!("memeplex", "Could not save the setting {key}: {e}");
    }
}

/// Which thumbnails show their format in a corner (Preferences): none, the ones that move, or
/// all.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TypeLabels {
    None,
    Animated,
    All,
}

impl TypeLabels {
    const KEY: &'static str = "type_labels";

    /// As saved on this device, the animated ones by default.
    pub fn get() -> Self {
        match get(Self::KEY).as_deref() {
            Some("none") => Self::None,
            Some("all") => Self::All,
            _ => Self::Animated,
        }
    }

    /// The name saved, and the one of its toggle in Preferences.
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Animated => "animated",
            Self::All => "all",
        }
    }

    pub fn save(name: &str) {
        set(Self::KEY, name);
    }
}

/// What thumbnails say about their file in a corner (Preferences), each off by default.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct ThumbInfo {
    pub size: bool,
    pub dimensions: bool,
    pub length: bool,
}

impl ThumbInfo {
    /// The keys of the three, as the switches in Preferences are named.
    pub const KEYS: [&'static str; 3] = ["info_size", "info_dimensions", "info_length"];

    /// As saved on this device.
    pub fn get() -> Self {
        let on = |key| get(key).is_some_and(|v| v == "true");
        Self {
            size: on(Self::KEYS[0]),
            dimensions: on(Self::KEYS[1]),
            length: on(Self::KEYS[2]),
        }
    }

    pub fn any(self) -> bool {
        self.size || self.dimensions || self.length
    }
}
