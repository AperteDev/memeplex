//! Compiles the Blueprint views in `ui/` to GtkBuilder XML and the translations in `po/` (for a
//! build run; installs compile their own, see the Makefile), in OUT_DIR.

use std::process::Command;

const PO: &str = "../../../po";

fn main() {
    let out = std::env::var("OUT_DIR").unwrap();
    println!("cargo::rerun-if-changed={PO}");
    let linguas = std::fs::read_to_string(format!("{PO}/LINGUAS")).unwrap_or_default();
    for lang in linguas
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let dir = format!("{out}/locale/{lang}/LC_MESSAGES");
        std::fs::create_dir_all(&dir).unwrap();
        let ok = Command::new("msgfmt")
            .args(["--check", "-o", &format!("{dir}/memeplex.mo")])
            .arg(format!("{PO}/{lang}.po"))
            .status()
            .expect("msgfmt not found; install gettext to build the Linux app")
            .success();
        assert!(ok, "msgfmt failed for {lang}");
    }

    println!("cargo::rerun-if-changed=ui");
    let blps: Vec<_> = std::fs::read_dir("ui")
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "blp"))
        .collect();
    let ok = Command::new("blueprint-compiler")
        .args(["batch-compile", &out, "ui"])
        .args(&blps)
        .status()
        .expect("blueprint-compiler not found; install it to build the Linux app")
        .success();
    assert!(ok, "blueprint-compiler failed");
}
