# Contributing

Thanks for your interest in Memeplex!

## Ideas, bugs and questions

They are very welcome: open an [issue](https://github.com/AperteDev/memeplex/issues) for a bug, a
feature request, an idea or a question. For bugs, mention your distribution and GNOME versions
and how to reproduce the problem.

## Pull requests

Memeplex is developed along a plan, and I prefer to make feature changes myself: for a feature,
open an issue describing it rather than a pull request. A pull request for a bug fix or another
small change may be accepted, but please **don't open one before it has been agreed** in an
issue; an unannounced pull request may clash with work in progress or go in a different
direction, so it might not be merged.

When one has been agreed:

- Keep it to one change, and follow the style of the surrounding code.
- Run `cargo fmt`, `cargo clippy` and `cargo test`.
- Use [Conventional Commits](https://www.conventionalcommits.org/) for commit messages, e.g.
  `fix(linux): keep the scroll position when a folder is rescanned`.

## Translations

Translations live in [`po/`](../po), one gettext catalog per language, shared by the app, its
UI files, the desktop file and the metainfo. Corrections to an existing language and new
languages are welcome; as for code, open a
[translation issue](https://github.com/AperteDev/memeplex/issues/new?template=translation.yml)
first, so two people don't translate the same thing.

- **Fix a language:** edit `po/<lang>.po` with any PO editor (such as
  [Translation Editor](https://flathub.org/apps/org.gnome.Gtranslator)) and open a pull request.
- **Add a language:** add its code to `po/LINGUAS`, run `scripts/i18n.sh` (it creates
  `po/<lang>.po` from the template), translate it and open a pull request.
- Keep placeholders such as `{name}` or `%d` as they are, and fill every plural form.
- To try it, build the app (`cargo build -p memeplex-linux` compiles the catalogs) and run it
  with your language, e.g. `LANGUAGE=pl target/debug/memeplex-linux`.

By contributing, you agree that your contribution is licensed under the
[GPL-3.0-or-later](../LICENSE).
