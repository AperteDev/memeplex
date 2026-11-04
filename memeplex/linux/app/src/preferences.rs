//! The Preferences dialog (ui/preferences.blp), as Notav's: what is kept on this device.

use crate::settings::TypeLabels;
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
    labels.connect_active_name_notify(move |labels| {
        if let Some(name) = labels.active_name() {
            TypeLabels::save(&name);
            changed();
        }
    });
    dialog.present(Some(window));
}
