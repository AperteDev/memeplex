# Agent notes

## Name

- The app is Memeplex (formerly Memento, taken on the AUR): app ID `dev.aperte.Memeplex`,
  lowercase `memeplex` for crates, packages and the gettext domain. The repository moves to
  `AperteDev` later.

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
- Never read or write the siblings' configuration, credentials, settings or databases.

## Shared code

- Logic a front-end needs that is not about widgets goes in the core, not in a front-end:
  indexing, search, tags, AI inference, sync, settings with their defaults. It returns no text
  (enums, ids, numbers); each front-end words and formats it.
- Android and Windows use the core through UniFFI bindings (Kotlin, C#), as in Todav, so keep
  its API exportable: synchronous, no closures or generics in what front-ends call.
- Decoding video and drawing the UI belong to the platform (GStreamer and GTK, Android's media
  APIs, Windows'); the core gets decoded frames and pixels.

## Dependencies

- Standard library and platform SDKs first; a third-party crate only where it replaces real
  work, with the reason in its commit.
