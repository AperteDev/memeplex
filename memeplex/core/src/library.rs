//! A meme folder on disk: its folders and the memes in them. Hidden files and folders (a name
//! starting with `.`) and symbolic links are skipped; nothing is written. Paths are strings, as
//! the bindings pass them, and come unsorted: each front-end sorts them in the user's locale.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A still or animated picture: PNG, JPEG, WebP, GIF.
    Image,
    /// MP4, WebM, MOV.
    Video,
}

/// A file's kind by its extension, `None` for what is not a meme.
pub fn kind(path: &str) -> Option<Kind> {
    let ext = Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "jfif" | "webp" | "gif" => Some(Kind::Image),
        "mp4" | "webm" | "mov" => Some(Kind::Video),
        _ => None,
    }
}

/// The folders under `root`, at any depth, relative to it with `/` between names.
pub fn folders(root: &str) -> Vec<String> {
    let mut out = Vec::new();
    walk(Path::new(root), "", &mut |rel, is_dir| {
        if is_dir {
            out.push(rel.to_string());
        }
    });
    out
}

/// How many memes each folder under `root` holds with its folders, by its path relative to
/// `root` ("" for `root` itself); a folder without any is left out.
pub fn counts(root: &str) -> HashMap<String, u64> {
    let mut out = HashMap::new();
    walk(Path::new(root), "", &mut |rel, is_dir| {
        if is_dir || kind(rel).is_none() {
            return;
        }
        let mut folder = rel;
        while !folder.is_empty() {
            folder = folder.rsplit_once('/').map_or("", |(parent, _)| parent);
            *out.entry(folder.to_string()).or_insert(0) += 1;
        }
    });
    out
}

/// The memes in `folder` and the folders under it, as full paths.
pub fn memes(folder: &str) -> Vec<String> {
    let mut out = Vec::new();
    walk(Path::new(folder), "", &mut |rel, is_dir| {
        if !is_dir && kind(rel).is_some() {
            out.push(format!("{folder}/{rel}"));
        }
    });
    out
}

/// Calls `found` with each visible entry under `dir` (its path relative to the walk's start, and
/// whether it is a folder), going into the folders. Unreadable folders are left out.
fn walk(dir: &Path, rel: &str, found: &mut dyn FnMut(&str, bool)) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let Ok(name) = e.file_name().into_string() else {
            continue;
        };
        let Ok(t) = e.file_type() else { continue };
        if name.starts_with('.') || t.is_symlink() {
            continue;
        }
        let path = if rel.is_empty() {
            name
        } else {
            format!("{rel}/{name}")
        };
        found(&path, t.is_dir());
        if t.is_dir() {
            walk(&e.path(), &path, found);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_visible_folders_and_memes() {
        let root = std::env::temp_dir().join(format!("memeplex-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for dir in ["IT/Linux", ".hidden"] {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        for file in [
            "a.JPG",
            "IT/b.gif",
            "IT/Linux/c.mp4",
            "IT/notes.txt",
            ".hidden/d.png",
        ] {
            fs::write(root.join(file), "").unwrap();
        }
        let root_str = root.to_str().unwrap();

        let mut f = folders(root_str);
        f.sort();
        assert_eq!(f, ["IT", "IT/Linux"]);

        let mut m = memes(root_str);
        m.sort();
        let full = |p: &str| format!("{root_str}/{p}");
        assert_eq!(m, [full("IT/Linux/c.mp4"), full("IT/b.gif"), full("a.JPG")]);
        assert_eq!(memes(&full("IT/Linux")), [full("IT/Linux/c.mp4")]);
        let counts = counts(root_str);
        let count = |f: &str| counts.get(f).copied();
        assert_eq!(
            [count(""), count("IT"), count("IT/Linux")],
            [Some(3), Some(2), Some(1)]
        );
        assert_eq!(counts.len(), 3); // not the hidden folder
        assert_eq!(kind("x.mp4"), Some(Kind::Video));
        assert_eq!(kind("x"), None);

        fs::remove_dir_all(&root).unwrap();
    }
}
