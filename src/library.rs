// SPDX-FileCopyrightText: 2026  Harish
// SPDX-License-Identifier: GPL-3.0-or-later

//! Folder grouping for the music library.
//!
//! A library is a root directory whose first-level subfolders are albums. Songs are queued
//! folder by folder (see `utils::load_files_from_folder`), so consecutive songs that share a
//! folder form a group. This module answers "which folder is this song in" and "where am I in
//! the play order", without touching GTK, so it can be unit tested.

use std::{
    cmp::Ordering,
    path::{Path, PathBuf},
};

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

/// Compare two directories given as path components, one component at a time.
///
/// Comparing whole path strings sorts `Beast 2` between `Beast` and `Beast/CD2` (a space
/// sorts before `/`), which splits the Beast folder in two. Comparing per component keeps a
/// folder's own subfolders right after it. Files directly in the root have no components and
/// sort first.
pub fn cmp_components<F: Fn(&str, &str) -> Ordering>(a: &[String], b: &[String], cmp: F) -> Ordering {
    for (x, y) in a.iter().zip(b.iter()) {
        let order = cmp(x, y);
        if order != Ordering::Equal {
            return order;
        }
    }
    a.len().cmp(&b.len())
}

/// Where the song at `index` ends up after moving the block `[start, start + len)` so that
/// it begins at `to`. `to` counts positions in the list after the block was taken out.
pub fn moved_index(index: u32, start: u32, len: u32, to: u32) -> u32 {
    if index >= start && index < start + len {
        return to + (index - start);
    }
    let without_block = if index >= start + len { index - len } else { index };
    if without_block >= to {
        without_block + len
    } else {
        without_block
    }
}

/// Destination (after removal) that makes the song at `from` play right after `current`.
pub fn play_next_target(from: u32, current: u32) -> u32 {
    let current_after_removal = if from < current { current - 1 } else { current };
    current_after_removal + 1
}

/// Destination (after removal) for dropping the song at `from` before or after `target`.
pub fn drop_target(from: u32, target: u32, before: bool) -> u32 {
    let insert_at = if before { target } else { target + 1 };
    if from < insert_at {
        insert_at - 1
    } else {
        insert_at
    }
}

/// Order the songs found on disk by a saved manual order: songs from the saved order that
/// still exist come first, in that order; new songs follow in disk order. Returns indices
/// into `disk`.
pub fn apply_saved_order(disk: &[String], saved: &[String]) -> Vec<usize> {
    use std::collections::{HashMap, HashSet};
    let position: HashMap<&str, usize> = disk.iter().enumerate().map(|(i, u)| (u.as_str(), i)).collect();
    let mut seen = HashSet::new();
    let mut order: Vec<usize> = saved
        .iter()
        .filter_map(|u| position.get(u.as_str()).copied())
        .filter(|i| seen.insert(*i))
        .collect();
    order.extend((0..disk.len()).filter(|i| !seen.contains(i)));
    order
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

    fn comps(s: &str) -> Vec<String> {
        s.split('/').filter(|c| !c.is_empty()).map(str::to_string).collect()
    }

    #[test]
    fn subfolders_stay_with_their_folder() {
        let mut dirs = vec![comps("Beast 2"), comps("Beast/CD2"), comps("Beast"), comps("")];
        dirs.sort_by(|a, b| cmp_components(a, b, |x, y| x.cmp(y)));
        assert_eq!(dirs, vec![comps(""), comps("Beast"), comps("Beast/CD2"), comps("Beast 2")]);
    }

    #[test]
    fn sorted_library_groups_without_splitting() {
        let root = p(ROOT);
        let mut files = vec![
            "Beast 2/1.mp3", "Beast/CD2/1.mp3", "loose.mp3", "Beast/1.mp3", "Beast/CD2/2.mp3", "Beast 2/2.mp3",
        ];
        files.sort_by(|a, b| {
            let (da, fa) = a.rsplit_once('/').unwrap_or(("", a));
            let (db, fb) = b.rsplit_once('/').unwrap_or(("", b));
            cmp_components(&comps(da), &comps(db), |x, y| x.cmp(y)).then(fa.cmp(fb))
        });
        let paths: Vec<PathBuf> = files.iter().map(|f| p(&format!("{ROOT}/{f}"))).collect();
        let names: Vec<_> = group_folders(Some(&root), &paths).iter().map(|f| (f.name.clone(), f.n_songs)).collect();
        assert_eq!(names, vec![("Music".to_string(), 1), ("Beast".to_string(), 3), ("Beast 2".to_string(), 2)]);
    }

    // Reference model: really move the block in a Vec and see where each element lands
    fn simulate(n: u32, start: u32, len: u32, to: u32) -> Vec<u32> {
        let mut v: Vec<u32> = (0..n).collect();
        let block: Vec<u32> = v.drain(start as usize..(start + len) as usize).collect();
        for (k, x) in block.into_iter().enumerate() {
            v.insert(to as usize + k, x);
        }
        let mut where_ = vec![0; n as usize];
        for (pos, x) in v.iter().enumerate() {
            where_[*x as usize] = pos as u32;
        }
        where_
    }

    #[test]
    fn moved_index_matches_a_real_move_for_every_case() {
        let n = 7;
        for len in 1..=3 {
            for start in 0..=(n - len) {
                for to in 0..=(n - len) {
                    let expected = simulate(n, start, len, to);
                    for i in 0..n {
                        assert_eq!(moved_index(i, start, len, to), expected[i as usize], "i={i} start={start} len={len} to={to}");
                    }
                }
            }
        }
    }

    #[test]
    fn play_next_lands_right_after_the_current_song() {
        for current in 0..6u32 {
            for from in 0..6u32 {
                if from == current { continue; }
                let to = play_next_target(from, current);
                let where_ = simulate(6, from, 1, to);
                assert_eq!(where_[from as usize], where_[current as usize] + 1, "from={from} current={current}");
            }
        }
    }

    #[test]
    fn drop_before_and_after_a_row() {
        for from in 0..5u32 {
            for target in 0..5u32 {
                if from == target { continue; }
                let w = simulate(5, from, 1, drop_target(from, target, true));
                assert_eq!(w[from as usize] + 1, w[target as usize], "before: from={from} target={target}");
                let w = simulate(5, from, 1, drop_target(from, target, false));
                assert_eq!(w[from as usize], w[target as usize] + 1, "after: from={from} target={target}");
            }
        }
    }

    #[test]
    fn saved_order_first_then_new_songs_and_missing_ones_dropped() {
        let disk: Vec<String> = ["a", "b", "c", "d"].iter().map(|s| s.to_string()).collect();
        let saved: Vec<String> = ["c", "gone", "a", "c"].iter().map(|s| s.to_string()).collect();
        assert_eq!(apply_saved_order(&disk, &saved), vec![2, 0, 1, 3]);
        assert_eq!(apply_saved_order(&disk, &[]), vec![0, 1, 2, 3]);
    }

    #[test]
    fn position_outside_the_queue_is_none() {
        let root = p(ROOT);
        let f = group_folders(Some(&root), &sample());
        assert_eq!(position_of(&f, 6), None);
        assert_eq!(position_of(&[], 0), None);
    }
}
