# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Linux: open a meme folder (Open in the sidebar, Ctrl+O); its folders show as a tree in the
  sidebar (a click opens a folder, its arrow closes it), and the memes of the folder picked
  there, with those of its folders, as a grid of thumbnails. The folder opens again at the next
  start. Each folder in the tree shows how many memes it holds, with those of its folders,
  dimmed after its name. View Options, at the right of the grid's header, has a slider for the
  thumbnails' size, kept on this device. The sidebar hides and shows again with the button at
  the left of the grid's header (F9).
- Linux: a click on a thumbnail (or Enter) opens the meme over the whole panel, growing out of
  the thumbnail as the grid fades behind it: pictures at full size, GIFs playing in a loop,
  videos with their controls. The X button (on the side of the window's own close button) or
  Esc shrinks it back into its thumbnail.
- Tags: the main way to group memes, any number per meme, kept with the meme's content so they
  stay through renames and moves. The sidebar shows All Memes, Untagged (the memes still to
  sort) and the tags, each with its count; the folder tree moved to its second view, Folders.
  The preview shows the meme's tags above it: Enter adds the ones typed in (several between
  commas), a click on one removes it. "Use Folder Names as Tags…" in the main menu tags every
  meme with the folders it is in. A thumbnail dragged onto a tag in the sidebar gets that tag
  (and leaves Untagged, if that is shown); dragged elsewhere, it is the meme's file, for other
  apps to take. Tags are saved in a `Memeplex` folder in the meme folder,
  one file for each device, so a folder synced to other devices (Nextcloud) brings its tags
  along without conflicts.

[Unreleased]: https://github.com/AperteDev/memeplex/commits/HEAD
