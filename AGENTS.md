# Agent notes

## Name

- The app is Memeplex (formerly Memento, taken on the AUR): app ID `dev.aperte.Memeplex`,
  lowercase `memeplex` for crates, packages and the gettext domain. The repository is
  `AperteDev/memeplex`.

## Layout

- As in Todav, the apps live under `memeplex/` (Todav roots its bindings' workspace there):
  `memeplex/linux/app` (`memeplex-linux`, GTK4 and libadwaita, Blueprint views in `ui/`),
  `memeplex/linux/data` (desktop file, metainfo, icon), `memeplex/linux/flatpak`,
  `memeplex/core` (`memeplex-core`), and later `memeplex/ffi`, `memeplex/android`,
  `memeplex/windows`.
- `make -C memeplex/linux install` installs for the user, `stage` the system-wide tree the
  tarball and `.deb` are made of, `flatpak` the Flatpak. `sandbox` (Zed task "linux: run in
  sandbox") runs a separate copy next to the normal one (`MEMEPLEX_SANDBOX`), its config, data
  and cache in `target/sandbox`; `sandbox-reset` wipes it. Test there, not in the user's app.
- The app icon is a placeholder until one is designed.

## Git

- Work on `master` only; don't create branches. Commit as you go, one logical change per commit;
  unrelated changes go in separate commits.
- Commit messages are Conventional Commits, one line, no body: `feat(core): add OCR indexing`.
- The branch `python` keeps the old Python app as it was; `src-python/` is a copy for reference
  while rewriting and will be deleted. Don't change or build on it.

## Roadmap

- `docs/ROADMAP.md` holds the planned work only (features, open questions, what was ruled out);
  what is built is described by the code. Read it before working on a planned feature, and
  update it in the same change: remove what is done, add a plan or an open question, move a
  dropped plan under "Ruled out" with why.

## Todav and Notav

- This app follows the conventions of its siblings Todav (`AperteDev/todav`) and Notav
  (`AperteDev/notav`), cloned next to this repository: a Rust core shared by every front-end, a
  GTK4/libadwaita app for Linux, Jetpack Compose for Android, WinUI for Windows. When something
  here is not decided yet, look at how Todav does it (its `AGENTS.md`, `docs/ROADMAP.md` and
  code), and write down code copied from either sibling here, so fixes can be ported.
- Copied from Todav: `memeplex/linux/app/src/i18n.rs` (`init`, `locale_dir`; copy its
  `gettext`, `ngettext`, `pgettext` when Rust code needs them), `build.rs`,
  `memeplex/linux/Makefile`, the Flatpak manifest, `po/its/`, `scripts/i18n.sh`,
  `scripts/release.py`, `scripts/deb.sh`, `.github/` (workflows, issue templates),
  `docs/CONTRIBUTING.md`, and the hiding sidebar (`Adw.OverlaySplitView` with its toggle
  button in `window.blp`, `win.toggle-sidebar` on F9 in `src/window.rs`) with its view
  switcher in the header (`Adw.InlineViewSwitcher`, Tags and Folders).
- Copied from Notav: the sidebar's folder tree in `memeplex/linux/app/src/window.rs`
  (`refresh_tree`, `tree_row` with its dimmed count, `toggle`, `INDENT`; unlike Notav, a click
  on a row picks the folder and opens it, its arrow opens or closes it; the counts come from
  the core's `library::counts`) and `sorted` (Notav's `by_name`); View Options in
  `window.blp`, a `Gtk.PopoverMenu` whose custom row (`size_box`, the thumbnail size slider) is
  added in code, as Notav's `zoom_box`; the index's migrations by `PRAGMA user_version` in
  `memeplex/core/src/index.rs` (Notav's `store::open`).
- Device settings of the Linux app (the meme folder, the thumbnail size) are in
  `memeplex/linux/app/src/settings.rs`, a key file in the config folder, until the core keeps
  settings.
- Never read or write the siblings' configuration, credentials, settings or databases.

## Shared code

- Logic a front-end needs that is not about widgets goes in the core, not in a front-end:
  indexing, search, tags, AI inference, sync, settings with their defaults. It returns no text
  (enums, ids, numbers); each front-end words and formats it.
- Android and Windows use the core through UniFFI bindings (Kotlin, C#), as in Todav, so keep
  its API exportable: synchronous, no closures or generics in what front-ends call.
- Decoding video and drawing the UI belong to the platform (GStreamer and GTK, Android's media
  APIs, Windows'); the core gets decoded frames and pixels.

## Translations

- All text a user sees is translatable; one gettext domain, `memeplex`, with catalogs in `po/`
  (languages in `po/LINGUAS`). Blueprint: `_("…")` / `C_("context", "…")`; Rust as in Todav's
  `AGENTS.md`. Run `scripts/i18n.sh` after changing text and commit `po/` with the change.

## Releases

- Add user-visible changes to `docs/CHANGELOG.md` under `## [Unreleased]`, in the same commit
  as the change ([Keep a Changelog](https://keepachangelog.com/en/1.1.0/)).
- Releases are cut by the manual Release workflow (`.github/workflows/release.yml`):
  `scripts/release.py` sets the version in `Cargo.toml`, `Cargo.lock` and the metainfo (with
  the changes as its notes) and moves the `[Unreleased]` changes under it; CI builds the
  packages, then it commits `chore(release): X`, tags `vX` and publishes them. Don't bump
  versions by hand.
- CI (`.github/workflows/ci.yml`) runs on every push: `cargo fmt --check`, clippy with warnings
  as errors and the tests on Fedora (GNOME 51), plus the Flatpak bundle, the tarball (built on
  Fedora 43, the oldest supported GNOME 49) and the `.deb` made from it on Ubuntu 26.04. Run
  the checks before committing.
- The metainfo must pass `appstreamcli validate` without warnings (Flathub treats them as
  errors).

## Dependencies

- Standard library and platform SDKs first; a third-party crate only where it replaces real
  work, with the reason in its commit.
