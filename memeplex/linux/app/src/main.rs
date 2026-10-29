//! Memeplex for Linux: the GTK app.

mod i18n;
mod window;

use adw::prelude::*;
use gtk::{gio, glib};

pub const APP_ID: &str = "dev.aperte.Memeplex";

fn main() -> glib::ExitCode {
    i18n::init();
    let app = adw::Application::builder().application_id(APP_ID).build();
    if sandbox() {
        // Its own process, not a window of the normal app's.
        app.set_flags(gio::ApplicationFlags::NON_UNIQUE);
    }
    app.add_action_entries([
        gio::ActionEntry::builder("about")
            .activate(|app: &adw::Application, _, _| about(app))
            .build(),
        gio::ActionEntry::builder("quit")
            .activate(|app: &adw::Application, _, _| app.quit())
            .build(),
    ]);
    app.set_accels_for_action("app.quit", &["<Control>q"]);
    app.connect_activate(window::present);
    app.run()
}

/// `MEMEPLEX_SANDBOX` set: a separate copy for testing (`make -C memeplex/linux sandbox`), next
/// to the normal app, with its files where the XDG variables say.
pub fn sandbox() -> bool {
    std::env::var_os("MEMEPLEX_SANDBOX").is_some()
}

fn about(app: &adw::Application) {
    adw::AboutDialog::builder()
        .application_name("Memeplex")
        .application_icon(APP_ID)
        .developer_name("Aperte")
        .version(env!("CARGO_PKG_VERSION"))
        .license_type(gtk::License::Gpl30)
        .website("https://github.com/AperteDev/memeplex")
        .issue_url("https://github.com/AperteDev/memeplex/issues")
        .build()
        .present(app.active_window().as_ref());
}
