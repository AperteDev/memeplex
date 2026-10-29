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
