// SPDX-FileCopyrightText: 2026  Harish
// SPDX-License-Identifier: GPL-3.0-or-later

//! Folder grouping for the music library.
//!
//! A library is a root directory whose first-level subfolders are albums. Songs are queued
//! folder by folder (see `utils::load_files_from_folder`), so consecutive songs that share a
//! folder form a group. This module answers "which folder is this song in" and "where am I in
//! the play order", without touching GTK, so it can be unit tested.

use std::path::{Path, PathBuf};

/// The folder a song belongs to.
///
/// Inside the library this is the first directory below `root`, so `Album/Album/01.mp3`
/// and `Album/02.mp3` both belong to `Album`. A file directly in `root` belongs to the root.
/// Outside the library, or with no library set, a song belongs to its own parent directory.
pub fn folder_key(root: Option<&Path>, song: &Path) -> Option<PathBuf> {
    if let Some(root) = root {
        if let Ok(rel) = song.strip_prefix(root) {
            let mut parts = rel.components();
            let first = parts.next()?;
            return if parts.next().is_some() {
                Some(root.join(first))
            } else {
                Some(root.to_path_buf())
            };
        }
    }
    song.parent().map(Path::to_path_buf)
}

/// One folder in play order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderInfo {
    pub key: PathBuf,
    pub name: String,
    pub n_songs: u32,
    pub first_index: u32,
}

/// Group consecutive songs that share a folder, keeping play order.
pub fn group_folders(root: Option<&Path>, songs: &[PathBuf]) -> Vec<FolderInfo> {
    let mut folders: Vec<FolderInfo> = Vec::new();
    for (i, song) in songs.iter().enumerate() {
        let Some(key) = folder_key(root, song) else {
            continue;
        };
        match folders.last_mut() {
            Some(last) if last.key == key => last.n_songs += 1,
            _ => {
                let name = key
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| key.to_string_lossy().into_owned());
                folders.push(FolderInfo {
                    key,
                    name,
                    n_songs: 1,
                    first_index: i as u32,
                });
            }
        }
    }
    folders
}

/// Where a song sits in the play order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    /// 1-based number of the song's folder
    pub folder: usize,
    pub n_folders: usize,
    /// 1-based number of the song inside its folder
    pub song_in_folder: u32,
    pub folder_size: u32,
    /// Name and size of the next folder, if there is one
    pub next: Option<(String, u32)>,
}

pub fn position_of(folders: &[FolderInfo], song_index: u32) -> Option<Position> {
    let idx = folders.iter().rposition(|f| f.first_index <= song_index)?;
    let f = &folders[idx];
    if song_index >= f.first_index + f.n_songs {
        return None;
    }
    Some(Position {
        folder: idx + 1,
        n_folders: folders.len(),
        song_in_folder: song_index - f.first_index + 1,
        folder_size: f.n_songs,
        next: folders.get(idx + 1).map(|n| (n.name.clone(), n.n_songs)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    const ROOT: &str = "/mnt/shared/Music";

    #[test]
    fn nested_folder_belongs_to_the_album_folder() {
        let root = p(ROOT);
        let a = p("/mnt/shared/Music/Akhanda (2021)/Akhanda/01.mp3");
        let b = p("/mnt/shared/Music/Akhanda (2021)/02.mp3");
        assert_eq!(folder_key(Some(&root), &a), Some(p("/mnt/shared/Music/Akhanda (2021)")));
        assert_eq!(folder_key(Some(&root), &b), Some(p("/mnt/shared/Music/Akhanda (2021)")));
    }

    #[test]
    fn loose_file_belongs_to_the_root() {
        let root = p(ROOT);
        assert_eq!(folder_key(Some(&root), &p("/mnt/shared/Music/single.mp3")), Some(root.clone()));
    }

    #[test]
    fn song_outside_the_library_uses_its_parent() {
        let root = p(ROOT);
        assert_eq!(folder_key(Some(&root), &p("/home/h/Downloads/x.mp3")), Some(p("/home/h/Downloads")));
        assert_eq!(folder_key(None, &p("/a/b/c.mp3")), Some(p("/a/b")));
    }

    #[test]
    fn a_sibling_with_the_same_prefix_is_not_inside_the_library() {
        let root = p(ROOT);
        assert_eq!(folder_key(Some(&root), &p("/mnt/shared/Music2/A/1.mp3")), Some(p("/mnt/shared/Music2/A")));
    }

    fn sample() -> Vec<PathBuf> {
        [
            "Aarya 2/1.mp3", "Aarya 2/2.mp3", "Aarya 2/3.mp3",
            "Akhanda (2021)/Akhanda/1.mp3", "Akhanda (2021)/Akhanda/2.mp3",
            "Annaatthe/Annaatthe/1.mp3",
        ]
        .iter()
        .map(|s| p(&format!("{ROOT}/{s}")))
        .collect()
    }

    #[test]
    fn groups_keep_play_order_and_counts() {
        let root = p(ROOT);
        let f = group_folders(Some(&root), &sample());
        let got: Vec<_> = f.iter().map(|x| (x.name.as_str(), x.n_songs, x.first_index)).collect();
        assert_eq!(got, vec![("Aarya 2", 3, 0), ("Akhanda (2021)", 2, 3), ("Annaatthe", 1, 5)]);
    }

    #[test]
    fn position_reports_folder_song_and_next() {
        let root = p(ROOT);
        let f = group_folders(Some(&root), &sample());
        let pos = position_of(&f, 4).unwrap();
        assert_eq!((pos.folder, pos.n_folders, pos.song_in_folder, pos.folder_size), (2, 3, 2, 2));
        assert_eq!(pos.next, Some(("Annaatthe".to_string(), 1)));
        let last = position_of(&f, 5).unwrap();
        assert_eq!((last.folder, last.next), (3, None));
    }

    #[test]
    fn position_outside_the_queue_is_none() {
        let root = p(ROOT);
        let f = group_folders(Some(&root), &sample());
        assert_eq!(position_of(&f, 6), None);
        assert_eq!(position_of(&[], 0), None);
    }
}
