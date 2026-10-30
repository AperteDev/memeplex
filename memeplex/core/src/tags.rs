//! Tags, kept by hash in tag files in the meme folder (`Memeplex/tags-<device>.tsv`), one per
//! device so syncing never makes two devices write the same file, and merged into the index.
//! A file holds a version line, then one line per meme its device tagged: the hash (hex), the
//! time of the change (milliseconds since 1970) and the tags, separated by tabs. A meme's newest
//! line among every device's file wins; a meme whose tags were all removed keeps its line, with
//! none, so the removal wins over older lines.

use crate::Result;
use crate::index::{Index, stat};
use crate::library::TAGS_DIR;
use rusqlite::{Connection, OptionalExtension};
use std::collections::HashMap;
use std::fs;
use std::hash::{BuildHasher, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

const HEADER: &str = "# memeplex tags 1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagCount {
    pub tag: String,
    /// Memes under the root with the tag, a meme in two places counted twice, as the grid shows
    /// it.
    pub count: u64,
}

impl Index {
    /// Reads the tag files under `root` that changed since they were last read (every one the
    /// first time), and forgets those gone. Returns whether any changed.
    pub fn load_tags(&self, root: String) -> Result<bool> {
        let dir = format!("{root}/{TAGS_DIR}/");
        let mut found = Vec::new();
        for e in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("tags-") && name.ends_with(".tsv") {
                found.push(format!("{dir}{name}"));
            }
        }
        let db = self.db.lock().unwrap();
        let mut changed = false;
        let known: Vec<String> = db
            .prepare("SELECT path FROM tag_files WHERE substr(path, 1, length(?1)) = ?1")?
            .query_map([&dir], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        for gone in known.iter().filter(|k| !found.contains(k)) {
            db.execute("DELETE FROM tag_lines WHERE file = ?1", [gone])?;
            db.execute("DELETE FROM tag_files WHERE path = ?1", [gone])?;
            changed = true;
        }
        for path in found {
            let Some((size, mtime)) = stat(&path) else {
                continue;
            };
            let last: Option<(i64, i64)> = db
                .query_row(
                    "SELECT size, mtime FROM tag_files WHERE path = ?1",
                    [&path],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            if last == Some((size, mtime)) {
                continue;
            }
            // Unreadable, or written by a newer version: left as it is.
            let Some(lines) = fs::read_to_string(&path).ok().and_then(|t| parse(&t)) else {
                continue;
            };
            let tx = db.unchecked_transaction()?;
            tx.execute("DELETE FROM tag_lines WHERE file = ?1", [&path])?;
            for (hash, time, tags) in lines {
                tx.execute(
                    "INSERT OR REPLACE INTO tag_lines (file, hash, time, tags) VALUES (?1, ?2, ?3, ?4)",
                    (&path, hash, time, tags),
                )?;
            }
            tx.execute(
                "INSERT OR REPLACE INTO tag_files (path, size, mtime) VALUES (?1, ?2, ?3)",
                (&path, size, mtime),
            )?;
            tx.commit()?;
            changed = true;
        }
        if changed {
            merge(&db, None)?;
        }
        Ok(changed)
    }

    /// The tags of the meme at `path`, none if it is not hashed yet.
    pub fn tags(&self, path: String) -> Result<Vec<String>> {
        let db = self.db.lock().unwrap();
        let mut q = db.prepare(
            "SELECT tag FROM meme_tags JOIN files USING (hash) WHERE path = ?1 ORDER BY tag",
        )?;
        Ok(q.query_map([path], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Sets the tags of the meme at `path` (under `root`) as this device's change; `save_tags`
    /// writes it to the tag file. Tabs and line breaks in a tag become spaces, and empty or
    /// repeated tags are left out. Returns `false` when the meme is not hashed yet.
    pub fn set_tags(&self, root: String, path: String, tags: Vec<String>) -> Result<bool> {
        let db = self.db.lock().unwrap();
        let hash: Option<Vec<u8>> = db
            .query_row("SELECT hash FROM files WHERE path = ?1", [path], |r| {
                r.get(0)
            })
            .optional()?;
        let Some(hash) = hash else {
            return Ok(false);
        };
        let file = own_file(&db, &root)?;
        set_line(&db, &file, &hash, &clean(tags))?;
        merge(&db, Some(&hash))?;
        Ok(true)
    }

    /// Adds the names of the folders each hashed meme under `root` is in to its tags. Returns how
    /// many memes got a tag.
    pub fn tags_from_folders(&self, root: String) -> Result<u64> {
        let db = self.db.lock().unwrap();
        let prefix = format!("{root}/");
        let mut wanted: HashMap<Vec<u8>, Vec<String>> = HashMap::new();
        let files: Vec<(String, Vec<u8>)> = db
            .prepare("SELECT path, hash FROM files WHERE substr(path, 1, length(?1)) = ?1")?
            .query_map([&prefix], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (path, hash) in files {
            let mut folders: Vec<&str> = path[prefix.len()..].split('/').collect();
            folders.pop();
            wanted
                .entry(hash)
                .or_default()
                .extend(folders.into_iter().map(String::from));
        }
        let file = own_file(&db, &root)?;
        let tx = db.unchecked_transaction()?;
        let mut tagged = 0;
        for (hash, folders) in wanted {
            let mut tags: Vec<String> = tx
                .prepare_cached("SELECT tag FROM meme_tags WHERE hash = ?1")?
                .query_map([&hash], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            let had = tags.len();
            tags = clean(tags.into_iter().chain(folders).collect());
            if tags.len() > had {
                set_line(&tx, &file, &hash, &tags)?;
                tagged += 1;
            }
        }
        tx.commit()?;
        merge(&db, None)?;
        Ok(tagged)
    }

    /// Writes this device's tag file under `root` from the index, through a temporary file
    /// renamed over it, so a sync client never sees it half-written. Call it a few seconds after
    /// the last change and on quit, as the whole file is uploaded again. ponytail: one file per
    /// device, about 2 MB at 20,000 memes; split it in 16 by the hash's first digit if uploading
    /// it costs too much.
    pub fn save_tags(&self, root: String) -> Result<()> {
        let db = self.db.lock().unwrap();
        let file = own_file(&db, &root)?;
        let lines: Vec<(Vec<u8>, i64, String)> = db
            .prepare("SELECT hash, time, tags FROM tag_lines WHERE file = ?1 ORDER BY hash")?
            .query_map([&file], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        if lines.is_empty() && stat(&file).is_none() {
            return Ok(());
        }
        let mut text = format!("{HEADER}\n");
        for (hash, time, tags) in lines {
            text += &hex(&hash);
            text += &format!("\t{time}");
            if !tags.is_empty() {
                text += "\t";
                text += &tags;
            }
            text += "\n";
        }
        fs::create_dir_all(format!("{root}/{TAGS_DIR}"))?;
        // Hidden: not synced by default, not read as a tag file.
        let temp = format!("{root}/{TAGS_DIR}/.{}.part", device(&db)?);
        fs::write(&temp, text)?;
        fs::rename(&temp, &file)?;
        if let Some((size, mtime)) = stat(&file) {
            db.execute(
                "INSERT OR REPLACE INTO tag_files (path, size, mtime) VALUES (?1, ?2, ?3)",
                (&file, size, mtime),
            )?;
        }
        Ok(())
    }

    /// The tags of the memes under `root`, with how many memes have each.
    pub fn tag_counts(&self, root: String) -> Result<Vec<TagCount>> {
        let db = self.db.lock().unwrap();
        let mut q = db.prepare(
            "SELECT tag, count(*) FROM meme_tags JOIN files USING (hash)
             WHERE substr(path, 1, length(?1)) = ?1 GROUP BY tag",
        )?;
        Ok(q.query_map([format!("{root}/")], |r| {
            Ok(TagCount {
                tag: r.get(0)?,
                count: r.get::<_, i64>(1)? as u64,
            })
        })?
        .collect::<rusqlite::Result<_>>()?)
    }

    /// The memes under `root` with `tag`, as full paths.
    pub fn tagged(&self, root: String, tag: String) -> Result<Vec<String>> {
        let db = self.db.lock().unwrap();
        let mut q = db.prepare(
            "SELECT path FROM files JOIN meme_tags USING (hash)
             WHERE substr(path, 1, length(?1)) = ?1 AND tag = ?2",
        )?;
        Ok(q.query_map((format!("{root}/"), tag), |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// The hashed memes under `root` without a tag, as full paths.
    pub fn untagged(&self, root: String) -> Result<Vec<String>> {
        let db = self.db.lock().unwrap();
        let mut q = db.prepare(
            "SELECT path FROM files WHERE substr(path, 1, length(?1)) = ?1
             AND hash NOT IN (SELECT hash FROM meme_tags)",
        )?;
        Ok(q.query_map([format!("{root}/")], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }
}

/// Records `tags` as this device's line for `hash`, newer than every line of it: a device whose
/// clock is behind still changes what it was shown.
fn set_line(db: &Connection, file: &str, hash: &[u8], tags: &[String]) -> Result<()> {
    let newest: Option<i64> = db.query_row(
        "SELECT max(time) FROM tag_lines WHERE hash = ?1",
        [hash],
        |r| r.get(0),
    )?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64);
    db.execute(
        "INSERT OR REPLACE INTO tag_lines (file, hash, time, tags) VALUES (?1, ?2, ?3, ?4)",
        (
            file,
            hash,
            now.max(newest.map_or(0, |t| t + 1)),
            tags.join("\t"),
        ),
    )?;
    Ok(())
}

/// Rebuilds the tags of `hash` (of every meme with `None`) from the newest line of each; on equal
/// times, the file whose path sorts last wins, the same on every device.
fn merge(db: &Connection, hash: Option<&[u8]>) -> Result<()> {
    let tx = db.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM meme_tags WHERE ?1 IS NULL OR hash = ?1",
        [hash],
    )?;
    let newest: Vec<(Vec<u8>, String)> = tx
        .prepare(
            "SELECT hash, tags FROM tag_lines t WHERE (?1 IS NULL OR hash = ?1) AND NOT EXISTS (
               SELECT 1 FROM tag_lines u WHERE u.hash = t.hash
               AND (u.time > t.time OR (u.time = t.time AND u.file > t.file)))",
        )?
        .query_map([hash], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    {
        let mut insert =
            tx.prepare("INSERT OR IGNORE INTO meme_tags (hash, tag) VALUES (?1, ?2)")?;
        for (hash, tags) in &newest {
            for tag in tags.split('\t').filter(|t| !t.is_empty()) {
                insert.execute((hash, tag))?;
            }
        }
    }
    tx.commit()?;
    Ok(())
}

/// The lines of a tag file: hash, time and tab-separated tags; `None` for a file of another
/// version. Lines that do not parse are left out.
fn parse(text: &str) -> Option<Vec<(Vec<u8>, i64, String)>> {
    let mut lines = text.lines();
    if lines.next()? != HEADER {
        return None;
    }
    Some(
        lines
            .filter_map(|line| {
                let mut fields = line.splitn(3, '\t');
                let hash = unhex(fields.next()?)?;
                let time = fields.next()?.parse().ok()?;
                let tags = clean(
                    fields
                        .next()
                        .unwrap_or("")
                        .split('\t')
                        .map(String::from)
                        .collect(),
                );
                Some((hash, time, tags.join("\t")))
            })
            .collect(),
    )
}

/// Tags as a tag file can hold them: tabs and line breaks as spaces, trimmed, no empty or
/// repeated ones.
fn clean(tags: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for tag in tags {
        let tag = tag.replace(['\t', '\n', '\r'], " ").trim().to_string();
        if !tag.is_empty() && !out.contains(&tag) {
            out.push(tag);
        }
    }
    out
}

/// This device's tag file under `root`.
fn own_file(db: &Connection, root: &str) -> Result<String> {
    Ok(format!("{root}/{TAGS_DIR}/tags-{}.tsv", device(db)?))
}

/// This device's id, made at random the first time and kept in the index.
fn device(db: &Connection) -> Result<String> {
    let id: Option<String> = db
        .query_row("SELECT value FROM kv WHERE key = 'device'", [], |r| {
            r.get(0)
        })
        .optional()?;
    if let Some(id) = id {
        return Ok(id);
    }
    // ponytail: the standard library's per-process random keys; a crate if ids must be longer.
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    // Another connection may have made one meanwhile: that one stays.
    db.execute(
        "INSERT OR IGNORE INTO kv (key, value) VALUES ('device', ?1)",
        [format!("{:016x}", h.finish())],
    )?;
    Ok(
        db.query_row("SELECT value FROM kv WHERE key = 'device'", [], |r| {
            r.get(0)
        })?,
    )
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() != 64 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two devices tagging the same folder, each with its index.
    #[test]
    fn devices_merge_their_tag_files() {
        let dir = std::env::temp_dir().join(format!("memeplex-tags-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let root = dir.join("memes");
        fs::create_dir_all(root.join("IT/Linux")).unwrap();
        fs::write(root.join("a.png"), "a").unwrap();
        fs::write(root.join("IT/Linux/b.gif"), "b").unwrap();
        let root = root.to_str().unwrap().to_string();
        let a = format!("{root}/a.png");
        let b = format!("{root}/IT/Linux/b.gif");
        let open = |name: &str| {
            let i = Index::open(dir.join(name).to_str().unwrap().into()).unwrap();
            i.scan(root.clone()).unwrap();
            i
        };
        let (one, two) = (open("one.sqlite"), open("two.sqlite"));
        let tags = |s: &[&str]| s.iter().map(|t| t.to_string()).collect::<Vec<_>>();

        assert!(
            one.set_tags(
                root.clone(),
                a.clone(),
                tags(&["Linux", " IT\t", "Linux", ""])
            )
            .unwrap()
        );
        one.save_tags(root.clone()).unwrap();
        assert!(two.load_tags(root.clone()).unwrap());
        assert_eq!(two.tags(a.clone()).unwrap(), tags(&["IT", "Linux"]));
        assert!(!two.load_tags(root.clone()).unwrap());

        // The newer line wins, removals too.
        two.set_tags(root.clone(), a.clone(), tags(&["Linux"]))
            .unwrap();
        two.save_tags(root.clone()).unwrap();
        one.load_tags(root.clone()).unwrap();
        assert_eq!(one.tags(a.clone()).unwrap(), tags(&["Linux"]));
        one.set_tags(root.clone(), a.clone(), vec![]).unwrap();
        one.save_tags(root.clone()).unwrap();
        two.load_tags(root.clone()).unwrap();
        assert!(two.tags(a.clone()).unwrap().is_empty());
        assert_eq!(two.untagged(root.clone()).unwrap().len(), 2);

        assert_eq!(two.tags_from_folders(root.clone()).unwrap(), 1);
        assert_eq!(two.tags(b.clone()).unwrap(), tags(&["IT", "Linux"]));
        assert_eq!(two.tagged(root.clone(), "Linux".into()).unwrap(), [b]);
        assert_eq!(two.untagged(root.clone()).unwrap(), [a]);
        let mut counts = two.tag_counts(root.clone()).unwrap();
        counts.sort_by(|x, y| x.tag.cmp(&y.tag));
        let count = |tag: &str| TagCount {
            tag: tag.into(),
            count: 1,
        };
        assert_eq!(counts, [count("IT"), count("Linux")]);

        let files = fs::read_dir(format!("{root}/{TAGS_DIR}")).unwrap().count();
        assert_eq!(files, 2); // one per device, no temporary file left
        fs::remove_dir_all(&dir).unwrap();
    }
}
