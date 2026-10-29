#!/usr/bin/env python3
"""Prepare a release: set the version in Cargo.toml and Cargo.lock, move the Unreleased changelog
section under the new version, add it to the app's metainfo as the release's notes, and print
them for the GitHub release. The Release workflow runs it.

    scripts/release.py 0.2.0 > notes.md
"""

import datetime
import html
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent


def fail(message: str) -> None:
    print(message, file=sys.stderr)
    sys.exit(1)


def edit(path: str, pattern: str, replacement: str) -> None:
    """Replace the first match of `pattern` in place, so the file keeps its formatting."""
    file = ROOT / path
    text = file.read_text()
    new, count = re.subn(pattern, replacement, text, count=1, flags=re.M | re.S)
    if count == 0:
        fail(f"{path}: nothing matches {pattern!r}")
    file.write_text(new)


def inline(text: str) -> str:
    """Changelog Markdown within a line as AppStream markup: `code` kept, links as their text,
    images left out (AppStream has none in release notes)."""
    text = re.sub(r"!\[[^\]]*\]\([^)]+\)", "", text).strip()
    text = re.sub(r"\[([^\]]+)\]\([^)]+\)", r"\1", text)
    return re.sub(r"`([^`]+)`", r"<code>\1</code>", html.escape(text, quote=False))


def release_xml(version: str, date: str, notes: str, repo: str, indent: str) -> str:
    """A metainfo <release> with a changelog section's notes, untranslated: each `### Added` and
    the like a paragraph, its items a list."""
    lines = [f'<release version="{version}" date="{date}">']
    lines.append(f'  <url type="details">{repo}/releases/tag/v{version}</url>')
    lines.append('  <description translate="no">')  # English, as the changelog
    items: list[str] = []

    def close_list() -> None:
        if items:
            lines.append("    <ul>")
            lines.extend(f"      <li>{inline(i)}</li>" for i in items)
            lines.append("    </ul>")
            items.clear()

    for line in notes.splitlines():
        if line.startswith("### "):
            close_list()
            lines.append(f"    <p>{inline(line[4:].strip())}</p>")
        elif line.startswith("- "):
            items.append(line[2:].strip())
        elif line.strip() and items:
            items[-1] += " " + line.strip()  # an item's next line
        elif line.strip():
            close_list()
            lines.append(f"    <p>{inline(line.strip())}</p>")
    close_list()
    lines += ["  </description>", "</release>"]
    return "\n".join(indent + l for l in lines) + "\n"


def set_release(metainfo: str, version: str, date: str, notes: str, repo: str) -> None:
    """Put the release first in the metainfo's <releases>, or replace it when listed."""
    file = ROOT / metainfo
    text = file.read_text()
    listed = re.compile(
        rf'^([ \t]*)<release version="{re.escape(version)}"[^>]*?(?:/>|>.*?</release>)\n',
        re.M | re.S,
    )
    if match := listed.search(text):
        new = release_xml(version, date, notes, repo, match[1])
        text = text[: match.start()] + new + text[match.end() :]
    else:
        match = re.search(r"^([ \t]*)<releases>\n", text, re.M)
        if not match:
            fail(f"{metainfo}: no <releases>")
        new = release_xml(version, date, notes, repo, match[1] + "  ")
        text = text[: match.end()] + new + text[match.end() :]
    file.write_text(text)


def main() -> None:
    version = sys.argv[1] if len(sys.argv) > 1 else ""
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        fail(f'Expected a version like 1.2.3, got "{version}"')
    date = datetime.date.today().isoformat()

    # Keep a Changelog: `## [Unreleased]` and `## [x.y.z] - date` sections, links at the end.
    changelog = ROOT / "docs" / "CHANGELOG.md"
    text = changelog.read_text()
    if f"\n## [{version}]" in text:
        fail(f"CHANGELOG.md already has a {version} section")
    heading = "## [Unreleased]\n"
    start = text.find(heading)
    if start == -1:
        fail("CHANGELOG.md has no [Unreleased] section")
    body_start = start + len(heading)
    # The next version's section, else the link definitions, end it.
    ends = [i for i in (text.find("\n## [", body_start), text.find("\n[Unreleased]:")) if i != -1]
    body_end = min(ends) + 1 if ends else len(text)
    notes = text[body_start:body_end].strip()
    if not notes:
        fail("The Unreleased section of CHANGELOG.md is empty")
    # [Unreleased] compares the last version with HEAD, and that comparison becomes the new
    # version's link; before the first release it points at the commits, and the first version
    # links to its tag.
    link = re.compile(r"^\[Unreleased\]: (\S+?)/(?:compare/(\S+)\.\.\.HEAD|commits/HEAD)$", re.M)
    match = link.search(text)
    if not match:
        fail("CHANGELOG.md has no [Unreleased] link")
    repo, previous = match[1], match[2]
    this = (
        f"{repo}/compare/{previous}...v{version}" if previous else f"{repo}/releases/tag/v{version}"
    )
    links = f"[Unreleased]: {repo}/compare/v{version}...HEAD\n[{version}]: {this}"
    rest = link.sub(lambda _: links, text[body_end:], count=1)
    changelog.write_text(f"{text[:body_start]}\n## [{version}] - {date}\n\n{notes}\n\n{rest}")

    edit("Cargo.toml", r'(\[workspace\.package\].*?^version = )"[^"]*"', rf'\g<1>"{version}"')
    # The workspace's own crates, the packages without a `source`, so `--locked` builds still work.
    lock = ROOT / "Cargo.lock"
    lock.write_text(
        re.sub(
            r'^(name = "[^"]+"\nversion = )"[^"]*"$(?!\nsource)',
            rf'\g<1>"{version}"',
            lock.read_text(),
            flags=re.M,
        )
    )
    set_release("memeplex/linux/data/dev.aperte.Memeplex.metainfo.xml", version, date, notes, repo)

    # The GitHub release's notes: images (relative to docs/ in the changelog) from the new tag.
    raw = repo.replace("https://github.com/", "https://raw.githubusercontent.com/")
    print(notes.replace("](screenshots/", f"]({raw}/v{version}/docs/screenshots/"))
    print(f"Prepared release {version}", file=sys.stderr)


if __name__ == "__main__":
    main()
