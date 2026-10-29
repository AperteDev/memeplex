//! Translations through glibc's gettext with the `memeplex` domain (catalogs in `po/`, refreshed
//! by `scripts/i18n.sh`). GtkBuilder translates the UI files through the domain set here.

use std::ffi::{CString, c_char};

const DOMAIN: &str = "memeplex";

unsafe extern "C" {
    fn bindtextdomain(domain: *const c_char, dir: *const c_char) -> *mut c_char;
    fn bind_textdomain_codeset(domain: *const c_char, codeset: *const c_char) -> *mut c_char;
    fn textdomain(domain: *const c_char) -> *mut c_char;
}

fn c(s: &str) -> CString {
    CString::new(s).expect("no NUL in text")
}

/// Use the user's locale and this app's catalogs; before any text is shown or built.
pub fn init() {
    unsafe { libc::setlocale(libc::LC_ALL, c"".as_ptr()) };
    let dir = locale_dir();
    let (domain, dir) = (c(DOMAIN), c(&dir.to_string_lossy()));
    unsafe {
        bindtextdomain(domain.as_ptr(), dir.as_ptr());
        bind_textdomain_codeset(domain.as_ptr(), c"UTF-8".as_ptr());
        textdomain(domain.as_ptr());
    }
}

/// Installed (`make install-data`), the catalogs are in `../share/locale` next to the binary's
/// directory (/usr, ~/.local, /app); a build run uses the ones build.rs compiled.
fn locale_dir() -> std::path::PathBuf {
    let installed = std::env::current_exe()
        .ok()
        .and_then(|e| Some(e.parent()?.parent()?.join("share/locale")));
    let has_catalogs = |d: &std::path::Path| {
        std::fs::read_dir(d).is_ok_and(|mut langs| {
            langs.any(|l| l.is_ok_and(|l| l.path().join("LC_MESSAGES/memeplex.mo").exists()))
        })
    };
    match installed {
        Some(d) if has_catalogs(&d) => d,
        _ => concat!(env!("OUT_DIR"), "/locale").into(),
    }
}
