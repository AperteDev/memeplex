# Memeplex — Roadmap

Planned work, not yet done; the order is not fixed. A rewrite of the Python app (branch
`python`, `src-python/`) as a meme and reaction library: find the right meme fast by what is in
it, then copy, drag or share it. Issue numbers refer to GitHub issues.

## Library (core)

- **Folders**: one or more meme folders, scanned recursively (hidden folders skipped) and
  watched for changes; nothing is written into them.
- **Identity by content**: each file is known by its hash (BLAKE3, as in `sifter`), so renames
  and moves keep tags and index data; duplicates show up as one hash in several places.
- **Index**: SQLite in the app's data folder; thumbnails in the cache folder (XDG on Linux).
- **Formats**: PNG, JPEG, WebP, animated GIF (#3) and WebP, videos (MP4, WebM, MOV; #4) such as
  saved reels.
- **Search** over file names, tags and OCR text, ignoring case and accents (as the Python app's
  `unidecode`), combined with smart search (see [AI](#ai)).
- **Tags**: added by hand or accepted from suggestions; favorites; recently used.
- **Background indexing**: one queue for thumbnails, OCR and embeddings, all files at once
  (#5), resumable, with progress; new files indexed as they appear.

## AI

All on the device, through ONNX Runtime (the `ort` crate) in the core, so every platform runs
the same models and code.

- **Embeddings**: a CLIP-family model gives each image a vector; the text encoder turns the
  search into one. Stored per file hash and model id, so a model change only means re-indexing.
- **Smart search**: by description ("cat screaming at the dinner table"), in Polish and English,
  by cosine similarity, brute force over all vectors.
- **Tag suggestions**: zero-shot, the image's vector against a tag vocabulary; the user accepts
  or rejects them.
- **Similar memes**: "more like this", near-duplicates, grouping by meme template.
- **OCR**: text in images and video frames, searchable; Latin scripts with Polish diacritics at
  least.
- **Video and GIFs**: a few frames sampled per file, each embedded and OCR'd; a file matches as
  well as its best frame.
- **Speech**, later: transcribing reels' audio (Whisper) for search.
- **Models**: downloaded on first use or bundled (see open questions), chosen in Preferences.

## Linux app

- GTK4 and libadwaita, Blueprint UI files, as in Todav and Notav.
- A grid of thumbnails with search on top; a preview panel with the file's details, tags and
  text; folders as a sidebar (#1).
- GIF and video playback in the preview and, muted, on hover in the grid.
- Copy to the clipboard (the image itself, not its path), drag and drop out into other apps,
  open with, show in folder.
- Packaging: Flatpak on Flathub (#6), AUR, and the others Todav ships.

## Android app

- Jetpack Compose over the core's Kotlin bindings, as Todav's.
- The same grid, search and preview; share to other apps; receive shared images and videos into
  the library; answer other apps' requests for an image (`GET_CONTENT`), so memes can be picked
  from a chat app.
- Indexing on the phone, as a background job while charging.

## Windows app

- WinUI over the core's C# bindings, as Todav's.
- The same grid, search, preview, clipboard and drag and drop as on Linux.

## Sync

- The memes stay in a Nextcloud folder as plain files (WebDAV), as Notav's notes; the app works
  from a local copy.
- What the user made (tags, favorites) goes into a small file in that folder, keyed by hash, so
  every device sees it; what can be computed (thumbnails, OCR, embeddings) stays local.
- Maybe: syncing embeddings too, so a phone searches what a desktop indexed and only runs the
  text encoder.
- Later, maybe: a web app or a Nextcloud app, if the native ones are not enough.

## Meme editor

- Making a meme from a template or any image in the library (#2): captions in the classic style
  (Impact, white with a black outline) and others, top, bottom or free placement, crop, resize.
- Captions over GIFs and videos.
- Templates found through similar memes; the result saved into the library.

## Open questions and what needs testing

- **Embedding model**: test on our own memes with a set of about 50 queries in Polish and
  English, each with a known answer; measure recall, indexing speed on a desktop CPU and on a
  phone, and size. Candidates: NLLB-CLIP base SigLIP (multilingual; Immich ships ONNX exports
  with separate image and text encoders), SigLIP 2 B/16 (multilingual), MobileCLIP2-S2 (fastest
  on phones, English only). One model on every device, or embeddings are not comparable when
  synced.
- **OCR engine**: PaddleOCR (PP-OCRv5, ONNX, through `ort`) or `ocrs` (pure Rust); test both on
  memes with Polish text, stylized fonts and text over busy pictures.
- **Tag suggestions**: the vocabulary (fixed, the user's own tags, or both), the prompt
  wording, and the similarity threshold; test how many suggestions are right.
- **`ort` on Android**: still 2.0 release candidates; Android builds exist since rc.11, Ente
  packages its own (`ente/ort-packaging`). Test the build, its size and the speed of the CPU
  (XNNPACK) and NPU execution providers.
- **GPU**: whether indexing on the CPU is fast enough for a large library; measure before adding
  an execution provider.
- **Models' distribution**: bundling makes the packages large (Flathub, F-Droid); downloading
  needs a source we trust and checksums, and F-Droid may flag it.
- **Video frames**: how many and which (fixed interval, scene changes), and the cost for long
  videos.
- **The sync file's format**: JSON in the folder (conflicts when two devices tag at once) or XMP
  sidecars next to each file (other apps read them, but they clutter the folder).

## Ruled out

- **A photo manager first** (Shotwell, digiKam style): albums by date, RAW, editing and faces
  are a different job and well served (Shotwell, digiKam, Immich, Nextcloud Memories; `sifter`
  sorts photos by date). The core stays media-agnostic, so a photo browser could reuse it later.
- **A separate app for the meme editor**: it needs the library and its templates; one app.
- **A vector database**: brute force over 50,000 vectors of 768 numbers (about 75 MB as f16)
  takes milliseconds; SQLite holds them.
- **PyTorch**: too large for phones and not embeddable in the core; ONNX Runtime replaces it.
