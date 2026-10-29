//! Memeplex for Linux: the GTK app.

mod i18n;
mod window;

use adw::prelude::*;
use gtk::{gio, glib};

pub const APP_ID: &str = "dev.aperte.Memeplex";

fn main() -> glib::ExitCode {
    i18n::init();
    let app = adw::Application::builder().application_id(APP_ID).build();
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
