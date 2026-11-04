//! The Preferences dialog (ui/preferences.blp), as Notav's: what is kept on this device.

use crate::settings::{self, ThumbInfo, TypeLabels};
use adw::prelude::*;
use gtk::glib;

/// Show it over `window`; `changed` runs after a preference changes, to apply it.
pub fn present(window: &adw::ApplicationWindow, changed: impl Fn() + 'static) {
    let b = gtk::Builder::from_string(include_str!(concat!(env!("OUT_DIR"), "/preferences.ui")));
    let get = |name: &str| {
        b.object::<glib::Object>(name)
            .unwrap_or_else(|| panic!("{name} in preferences.ui"))
    };
    let dialog: adw::PreferencesDialog = get("dialog").downcast().unwrap();
    let labels: adw::ToggleGroup = get("type_labels").downcast().unwrap();
    labels.set_active_name(Some(TypeLabels::get().name()));
    let changed = std::rc::Rc::new(changed);
    let apply = changed.clone();
    labels.connect_active_name_notify(move |labels| {
        if let Some(name) = labels.active_name() {
            TypeLabels::save(&name);
            apply();
        }
    });
    let saved = ThumbInfo::get();
    for (key, on) in ThumbInfo::KEYS
        .into_iter()
        .zip([saved.size, saved.dimensions, saved.length])
    {
        let row: adw::SwitchRow = get(key).downcast().unwrap();
        row.set_active(on);
        let apply = changed.clone();
        row.connect_active_notify(move |row| {
            settings::set(key, &row.is_active().to_string());
            apply();
        });
    }
    dialog.present(Some(window));
}
