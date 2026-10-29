//! The index: what is known of each meme, in SQLite in the app's data folder. A cache of this
//! device, rebuilt from the meme folder when lost. Migrations by `PRAGMA user_version`, as
//! Notav's store.

use crate::{Result, library};
use rusqlite::Connection;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

const MIGRATIONS: &[&str] = &[r#"
CREATE TABLE files (
  path TEXT PRIMARY KEY,                -- full path
  size INTEGER NOT NULL,
  mtime INTEGER NOT NULL,               -- nanoseconds since 1970
  hash BLOB NOT NULL                    -- BLAKE3, 32 bytes: the meme's identity
);
CREATE INDEX files_hash ON files (hash);
"#];

pub struct Index {
    db: Mutex<Connection>,
}

impl Index {
    /// Opens the index at `path` (a file), made with its folder if missing.
    pub fn open(path: String) -> Result<Index> {
        if let Some(dir) = Path::new(&path).parent() {
            fs::create_dir_all(dir)?;
        }
        let db = Connection::open(&path)?;
        // A cache: losing the last writes in a crash only means hashing those files again.
        db.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
        let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        for (i, m) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            let tx = db.unchecked_transaction()?;
            tx.execute_batch(m)?;
            tx.pragma_update(None, "user_version", i as i64 + 1)?;
            tx.commit()?;
        }
        Ok(Index { db: Mutex::new(db) })
    }

    /// Brings the index up to date with the memes under `root`: new files and those whose size
    /// or modification time changed are hashed, gone ones dropped. Returns how many were
    /// hashed. The first time it reads every file: call it off the UI thread. Each file is
    /// saved as it is hashed, so a scan cut short picks up where it stopped.
    pub fn scan(&self, root: String) -> Result<u64> {
        let mut known: HashMap<String, (i64, i64)> = {
            let db = self.db.lock().unwrap();
            let mut q = db.prepare(
                "SELECT path, size, mtime FROM files WHERE substr(path, 1, length(?1)) = ?1",
            )?;
            q.query_map([format!("{root}/")], |r| {
                Ok((r.get(0)?, (r.get(1)?, r.get(2)?)))
            })?
            .collect::<rusqlite::Result<_>>()?
        };
        let mut hashed = 0;
        for path in library::memes(&root) {
            // Unreadable or gone since the walk: left out, as the walk leaves out what it
            // cannot read.
            let Ok(meta) = fs::metadata(&path) else {
                continue;
            };
            let size = meta.len() as i64;
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos() as i64);
            if known.remove(&path) == Some((size, mtime)) {
                continue;
            }
            // ponytail: one file at a time, as reading the disk is the limit; hash on a few
            // threads if SSDs show it is not.
            let Ok(hash) = hash(&path) else {
                continue;
            };
            self.db.lock().unwrap().execute(
                "INSERT OR REPLACE INTO files (path, size, mtime, hash) VALUES (?1, ?2, ?3, ?4)",
                (&path, size, mtime, &hash[..]),
            )?;
            hashed += 1;
        }
        // What is left was not found.
        let db = self.db.lock().unwrap();
        for path in known.keys() {
            db.execute("DELETE FROM files WHERE path = ?1", [path])?;
        }
        Ok(hashed)
    }
}

/// The BLAKE3 hash of the file at `path`.
fn hash(path: &str) -> Result<[u8; 32]> {
    let mut h = blake3::Hasher::new();
    h.update_reader(fs::File::open(path)?)?;
    Ok(*h.finalize().as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_new_and_changed_files_and_drops_gone_ones() {
        let dir = std::env::temp_dir().join(format!("memeplex-index-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let root = dir.join("memes");
        fs::create_dir_all(root.join("IT")).unwrap();
        fs::write(root.join("a.png"), "a").unwrap();
        fs::write(root.join("IT/b.gif"), "b").unwrap();
        let root_str = root.to_str().unwrap().to_string();
        let index = Index::open(dir.join("db/index.sqlite").to_str().unwrap().into()).unwrap();
        let hashes = || -> Vec<(String, Vec<u8>)> {
            let db = index.db.lock().unwrap();
            let mut q = db
                .prepare("SELECT path, hash FROM files ORDER BY path")
                .unwrap();
            q.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .map(|r| r.unwrap())
                .collect()
        };

        assert_eq!(index.scan(root_str.clone()).unwrap(), 2);
        let a = format!("{root_str}/a.png");
        assert_eq!(
            hashes()[1],
            (a.clone(), blake3::hash(b"a").as_bytes().to_vec())
        );
        assert_eq!(index.scan(root_str.clone()).unwrap(), 0);

        fs::write(&a, "changed").unwrap();
        fs::remove_file(root.join("IT/b.gif")).unwrap();
        assert_eq!(index.scan(root_str.clone()).unwrap(), 1);
        assert_eq!(
            hashes(),
            [(a, blake3::hash(b"changed").as_bytes().to_vec())]
        );

        fs::remove_dir_all(&dir).unwrap();
    }
}
