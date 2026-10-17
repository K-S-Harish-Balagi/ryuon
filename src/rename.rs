// SPDX-FileCopyrightText: 2026  Harish
// SPDX-License-Identifier: GPL-3.0-or-later

//! Renaming the music library on disk to its tidy names, with a write-ahead journal so every
//! rename can be undone. Nothing is ever deleted or overwritten: a name that would clash
//! gets a number added, and a rename stops if its target appeared in the meantime.
//!
//! Order matters: renames run deepest first (a file before its folder, a folder before its
//! parent), so each source path is still valid when its turn comes. Undo runs in reverse.

use std::{
    collections::HashSet,
    fs,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{json, Value};

use crate::tidy;

const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "ogg", "oga", "opus", "m4a", "aac", "wav", "wma"];
// Names Windows refuses (the library may live on NTFS)
const RESERVED: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

#[derive(Debug, Clone, PartialEq)]
pub struct Rename {
    pub from: PathBuf,
    pub to: PathBuf,
    pub is_dir: bool,
}

#[derive(Debug, Default)]
pub struct Plan {
    pub ops: Vec<Rename>,
    /// Renames that got a number added because the tidy name was taken
    pub clashes: Vec<PathBuf>,
}

impl Plan {
    pub fn n_folders(&self) -> usize {
        self.ops.iter().filter(|r| r.is_dir).count()
    }

    pub fn n_files(&self) -> usize {
        self.ops.iter().filter(|r| !r.is_dir).count()
    }
}

fn is_audio(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// A name every file system accepts: no `<>:"/\|?*` or control characters, no trailing dot or
/// space, not a reserved Windows device name. None if nothing usable is left.
fn safe_name(name: &str) -> Option<String> {
    let cleaned: String = name
        .chars()
        .filter(|c| !c.is_control() && !"<>:\"/\\|?*".contains(*c))
        .collect();
    let cleaned = cleaned.trim().trim_end_matches(['.', ' ']).to_string();
    let stem = cleaned.split('.').next().unwrap_or("").to_ascii_lowercase();
    if cleaned.is_empty() || RESERVED.contains(&stem.as_str()) {
        None
    } else {
        Some(cleaned)
    }
}

/// The tidy name for one directory entry, or None to leave it alone. Track numbers stay.
fn tidy_entry(name: &str, is_dir: bool) -> Option<String> {
    let new = if is_dir {
        safe_name(&tidy::name(name))?
    } else if is_audio(name) {
        let path = Path::new(name);
        let stem = path.file_stem()?.to_str()?;
        let ext = path.extension()?.to_str()?;
        format!("{}.{}", safe_name(&tidy::name(stem))?, ext)
    } else {
        return None;
    };
    (new != name).then_some(new)
}

fn numbered(name: &str, is_dir: bool, n: usize) -> String {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !is_dir => format!("{stem} ({n}).{ext}"),
        _ => format!("{name} ({n})"),
    }
}

/// Plans tidy names for every folder and audio file under `root` (not `root` itself).
pub fn plan(root: &Path) -> io::Result<Plan> {
    let mut plan = Plan::default();
    plan_dir(root, &mut plan)?;
    Ok(plan)
}

fn plan_dir(dir: &Path, plan: &mut Plan) -> io::Result<()> {
    let mut entries: Vec<(String, bool)> = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else {
            continue; // not valid UTF-8: leave it alone
        };
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        entries.push((name, file_type.is_dir()));
    }
    entries.sort();

    // Deeper renames first, while this folder's paths are still the original ones
    for (name, is_dir) in &entries {
        if *is_dir {
            plan_dir(&dir.join(name), plan)?;
        }
    }

    // Clash checks ignore case (NTFS, and to avoid names differing only in case). Every
    // original name counts as taken, so a rename never depends on another one going first.
    let mut taken: HashSet<String> = entries.iter().map(|(n, _)| n.to_lowercase()).collect();
    for (name, is_dir) in &entries {
        let Some(wanted) = tidy_entry(name, *is_dir) else {
            continue;
        };
        let mut new = wanted.clone();
        let mut n = 2;
        // A change of case only is not a clash with itself
        while taken.contains(&new.to_lowercase()) && new.to_lowercase() != name.to_lowercase() {
            new = numbered(&wanted, *is_dir, n);
            n += 1;
        }
        taken.insert(new.to_lowercase());
        if new != wanted {
            plan.clashes.push(dir.join(&new));
        }
        plan.ops.push(Rename { from: dir.join(name), to: dir.join(&new), is_dir: *is_dir });
    }
    Ok(())
}

/// Where `path` ends up after the renames (for the queue, the saved order and the resume point)
pub fn map_path(ops: &[Rename], path: &Path) -> PathBuf {
    let mut path = path.to_path_buf();
    for op in ops {
        if path == op.from {
            path = op.to.clone();
        } else if let Ok(rest) = path.strip_prefix(&op.from) {
            path = op.to.join(rest);
        }
    }
    path
}

/// The same renames under another path to the same folder: the document portal's path to the
/// library is writable when the library's own path is only readable to the sandbox
pub fn rebase(ops: &[Rename], from: &Path, to: &Path) -> Vec<Rename> {
    let moved = |p: &Path| match p.strip_prefix(from) {
        Ok(rest) if rest.as_os_str().is_empty() => to.to_path_buf(),
        Ok(rest) => to.join(rest),
        Err(_) => p.to_path_buf(),
    };
    ops.iter()
        .map(|op| Rename { from: moved(&op.from), to: moved(&op.to), is_dir: op.is_dir })
        .collect()
}

fn same_name_ignoring_case(a: &Path, b: &Path) -> bool {
    a.parent() == b.parent()
        && a.file_name().map(|n| n.to_string_lossy().to_lowercase())
            == b.file_name().map(|n| n.to_string_lossy().to_lowercase())
}

/// A new journal path in `dir`, named by the current time
pub fn new_journal_path(dir: &Path) -> PathBuf {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    dir.join(format!("rename-{secs}.jsonl"))
}

/// The most recent journal that has not been undone
pub fn latest_journal(dir: &Path) -> Option<PathBuf> {
    let mut journals: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let name = p.file_name().unwrap().to_string_lossy();
            name.starts_with("rename-") && name.ends_with(".jsonl") && !name.ends_with(".undone.jsonl")
        })
        .collect();
    journals.sort();
    journals.pop()
}

/// Renames everything in the plan. The journal is written and synced before the first
/// rename, and each finished rename is recorded, so `undo` works even after a crash.
/// Returns how many renames were done; on error, how many were done before it.
/// Renames as planned under `root`. `library` is the path the rest of the app knows the
/// library by, when `root` is another way in (the document portal); it is kept in the journal
/// so `journal_ops` can give the renames back in the app's terms.
pub fn apply(plan: &Plan, root: &Path, library: &Path, journal: &Path) -> Result<usize, (usize, io::Error)> {
    let fail = |done, e| Err((done, e));
    for op in &plan.ops {
        if !op.from.exists() {
            return fail(0, io::Error::new(io::ErrorKind::NotFound, format!("{} is gone", op.from.display())));
        }
        if op.to.exists() && !same_name_ignoring_case(&op.from, &op.to) {
            return fail(0, io::Error::new(io::ErrorKind::AlreadyExists, format!("{} exists", op.to.display())));
        }
    }

    if let Some(dir) = journal.parent() {
        fs::create_dir_all(dir).map_err(|e| (0, e))?;
    }
    let header = json!({
        "ryuon-rename": 1,
        "root": root,
        "library": library,
        "ops": plan.ops.iter().map(|r| json!([r.from, r.to])).collect::<Vec<_>>(),
    });
    let mut file = fs::File::create(journal).map_err(|e| (0, e))?;
    writeln!(file, "{header}").and_then(|_| file.sync_all()).map_err(|e| (0, e))?;

    for (i, op) in plan.ops.iter().enumerate() {
        // Never overwrite: something may have appeared since the plan was made
        if op.to.exists() && !same_name_ignoring_case(&op.from, &op.to) {
            return fail(i, io::Error::new(io::ErrorKind::AlreadyExists, format!("{} exists", op.to.display())));
        }
        fs::rename(&op.from, &op.to).map_err(|e| (i, e))?;
        writeln!(file, "{}", json!({ "done": i })).and_then(|_| file.sync_data()).map_err(|e| (i + 1, e))?;
    }
    Ok(plan.ops.len())
}

/// The journal's renames reversed and newest first: applying them in order, or passing them
/// to `map_path`, goes back to the original names. Renames that may not have happened are
/// included: their tidy path does not exist, so they match nothing.
pub fn journal_ops(journal: &Path) -> io::Result<Vec<Rename>> {
    let ops: Vec<Rename> = read_journal(journal)?.into_iter().map(|(op, _)| op).collect();
    let header: Value = io::BufReader::new(fs::File::open(journal)?)
        .lines()
        .next()
        .and_then(|l| l.ok())
        .and_then(|l| serde_json::from_str(&l).ok())
        .unwrap_or_default();
    match (header["root"].as_str(), header["library"].as_str()) {
        (Some(root), Some(library)) if root != library => Ok(rebase(&ops, Path::new(root), Path::new(library))),
        _ => Ok(ops),
    }
}

// Reversed renames, each with whether it was recorded as done
fn read_journal(journal: &Path) -> io::Result<Vec<(Rename, bool)>> {
    let bad = |what: &str| io::Error::new(io::ErrorKind::InvalidData, format!("journal: {what}"));
    let reader = io::BufReader::new(fs::File::open(journal)?);
    let mut lines = reader.lines();
    let header: Value = serde_json::from_str(&lines.next().ok_or_else(|| bad("empty"))??)
        .map_err(|_| bad("unreadable header"))?;
    let ops: Vec<(PathBuf, PathBuf)> = header["ops"]
        .as_array()
        .ok_or_else(|| bad("no ops"))?
        .iter()
        .filter_map(|op| Some((PathBuf::from(op[0].as_str()?), PathBuf::from(op[1].as_str()?))))
        .collect();
    let mut done: HashSet<usize> = HashSet::new();
    for line in lines {
        if let Ok(v) = serde_json::from_str::<Value>(&line?) {
            if let Some(i) = v["done"].as_u64() {
                done.insert(i as usize);
            }
        }
    }
    Ok(ops
        .into_iter()
        .enumerate()
        .rev()
        .map(|(i, (from, to))| (Rename { is_dir: false, from: to, to: from }, done.contains(&i)))
        .collect())
}

/// Puts back every rename recorded in the journal, newest first, and marks the journal as
/// undone. Renames that happened but were not recorded (a crash in between) are found by
/// looking at the disk. Returns how many names were restored.
pub fn undo(journal: &Path) -> io::Result<usize> {
    let mut restored = 0;
    // Already reversed: each op renames `from` (the tidy name) back to `to`. An unrecorded
    // rename (crash in between) is checked on disk now, after its parents were put back.
    for (op, recorded) in read_journal(journal)? {
        let (to, from) = (&op.from, &op.to);
        let happened = recorded || (to.exists() && !from.exists());
        if !happened || from == to {
            continue;
        }
        if from.exists() && !same_name_ignoring_case(from, to) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("cannot put back {}: the name is in use", from.display()),
            ));
        }
        fs::rename(to, from)?;
        restored += 1;
    }

    let undone = journal.with_extension("undone.jsonl");
    fs::rename(journal, undone)?;
    Ok(restored)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("ryuon-rename-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }

        fn touch(&self, rel: &str) {
            let p = self.0.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, rel).unwrap();
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    // Every file, relative, with its contents: the full state of a tree
    fn snapshot(root: &Path) -> Vec<(String, String)> {
        fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
            for e in fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    out.push((rel(root, &p) + "/", String::new()));
                    walk(root, &p, out);
                } else {
                    out.push((rel(root, &p), fs::read_to_string(&p).unwrap()));
                }
            }
        }
        let mut out = Vec::new();
        walk(root, root, &mut out);
        out.sort();
        out
    }

    // Relative path with "/" on every system, so expectations read the same on Windows
    fn rel(root: &Path, p: &Path) -> String {
        p.strip_prefix(root).unwrap().display().to_string().replace('\\', "/")
    }

    #[test]
    fn plans_tidy_names_and_keeps_track_numbers() {
        let t = TempDir::new("plan");
        t.touch("Coolie-320kbps-MassTamilan/Monica-MassTamilan.mp3");
        t.touch("Coolie-320kbps-MassTamilan/cover.jpg");
        t.touch("Kubera (2025)/[iSongs.info] 04 - Poyiraa Mama.mp3");
        t.touch("Annaatthe/Annaatthe/Thalaivar Rampage.mp3");
        let plan = plan(&t.0).unwrap();
        let ops: Vec<(String, String)> = plan.ops.iter().map(|r| (rel(&t.0, &r.from), rel(&t.0, &r.to))).collect();
        assert_eq!(
            ops,
            vec![
                (
                    "Coolie-320kbps-MassTamilan/Monica-MassTamilan.mp3".into(),
                    "Coolie-320kbps-MassTamilan/Monica.mp3".into()
                ),
                ("Kubera (2025)/[iSongs.info] 04 - Poyiraa Mama.mp3".into(), "Kubera (2025)/04 - Poyiraa Mama.mp3".into()),
                ("Coolie-320kbps-MassTamilan".into(), "Coolie".into()),
            ]
        );
        assert_eq!((plan.n_folders(), plan.n_files()), (1, 2));
        assert!(plan.clashes.is_empty());
    }

    #[test]
    fn clashes_get_a_number_and_ignore_case() {
        let t = TempDir::new("clash");
        t.touch("Beast/Arabic Kuthu.mp3");
        t.touch("Beast - MassTamilan.so/Jolly O Gymkhana.mp3");
        t.touch("Jailer-128kbps-MassTamilan.dev/Hukum.mp3");
        t.touch("Jailer - MassTamilan.dev/Hukum.mp3");
        t.touch("jailer/x.mp3");
        let plan = plan(&t.0).unwrap();
        let mut tos: Vec<String> = plan.ops.iter().filter(|r| r.is_dir).map(|r| rel(&t.0, &r.to)).collect();
        tos.sort();
        assert_eq!(tos, vec!["Beast (2)", "Jailer (2)", "Jailer (3)"]);
        assert_eq!(plan.clashes.len(), 3);
    }

    #[test]
    fn unsafe_names_are_left_alone_or_cleaned() {
        assert_eq!(safe_name("Dhurandhar: The Revenge").as_deref(), Some("Dhurandhar The Revenge"));
        assert_eq!(safe_name("Wait...").as_deref(), Some("Wait"));
        assert_eq!(safe_name("CON"), None);
        assert_eq!(safe_name("aux.mp3"), None);
        assert_eq!(tidy_entry("notes.txt", false), None);
        assert_eq!(tidy_entry("Leo", true), None);
    }

    #[test]
    fn apply_then_undo_restores_everything() {
        let t = TempDir::new("roundtrip");
        for f in [
            "Coolie-320kbps-MassTamilan/Monica-MassTamilan.mp3",
            "Coolie-320kbps-MassTamilan/cover.jpg",
            "Beast/Arabic Kuthu.mp3",
            "Beast - MassTamilan.so/Arabic-Kuthu-MassTamilan.so.mp3",
            // A colon is legal on Linux only; Windows cannot hold such a file at all
            if cfg!(windows) { "Annaatthe/Annaatthe-SenSongsMp3.Co/Vaa Saamy - x.mp3" } else { "Annaatthe/Annaatthe-SenSongsMp3.Co/Vaa Saamy :: x.mp3" },
        ] {
            t.touch(f);
        }
        let library = t.0.join("lib");
        fs::rename(t.0.join("Coolie-320kbps-MassTamilan"), t.0.join("tmp")).unwrap();
        fs::create_dir_all(&library).unwrap();
        for d in ["Beast", "Beast - MassTamilan.so", "Annaatthe"] {
            fs::rename(t.0.join(d), library.join(d)).unwrap();
        }
        fs::rename(t.0.join("tmp"), library.join("Coolie-320kbps-MassTamilan")).unwrap();

        let before = snapshot(&library);
        let plan = plan(&library).unwrap();
        let journal = t.0.join("journal").join("rename-1.jsonl");
        assert_eq!(apply(&plan, &library, &library, &journal).unwrap(), plan.ops.len());
        let after = snapshot(&library);
        assert!(after.iter().any(|(p, _)| p == "Coolie/Monica.mp3"));
        assert!(after.iter().any(|(p, _)| p == "Beast (2)/Arabic Kuthu.mp3"));
        assert!(after.iter().any(|(p, _)| p == "Coolie/cover.jpg"));
        // contents travel with the names
        let monica = after.iter().find(|(p, _)| p == "Coolie/Monica.mp3").unwrap();
        assert_eq!(monica.1, "Coolie-320kbps-MassTamilan/Monica-MassTamilan.mp3");
        assert_eq!(before.len(), after.len());

        assert_eq!(latest_journal(&t.0.join("journal")).as_deref(), Some(journal.as_path()));
        // the reversed journal maps the new paths back to the old ones
        let back = journal_ops(&journal).unwrap();
        for (p, _) in before.iter().filter(|(p, _)| !p.ends_with('/')) {
            let new = map_path(&plan.ops, &library.join(p));
            assert_eq!(map_path(&back, &new), library.join(p));
        }
        assert_eq!(undo(&journal).unwrap(), plan.ops.len());
        assert_eq!(snapshot(&library), before);
        assert_eq!(latest_journal(&t.0.join("journal")), None);
    }

    #[test]
    fn undo_after_a_crash_finds_unrecorded_renames() {
        let t = TempDir::new("crash");
        t.touch("lib/Leo - MassTamilan.dev/Naa-Ready-MassTamilan.dev.mp3");
        t.touch("lib/Darbar 64Kbps/Dumm Dumm.mp3");
        let library = t.0.join("lib");
        let before = snapshot(&library);
        let plan = plan(&library).unwrap();
        let journal = t.0.join("rename-1.jsonl");
        apply(&plan, &library, &library, &journal).unwrap();
        // Drop the "done" records, as if the app died right after renaming
        let header = fs::read_to_string(&journal).unwrap().lines().next().unwrap().to_string();
        fs::write(&journal, header + "\n").unwrap();
        undo(&journal).unwrap();
        assert_eq!(snapshot(&library), before);
    }

    #[test]
    #[cfg(unix)]
    fn renames_through_another_path_and_undo_reports_library_paths() {
        // A symlink stands in for the document portal's path to the same folder
        let t = TempDir::new("portal");
        t.touch("lib/Leo - MassTamilan.dev/Naa-Ready-MassTamilan.dev.mp3");
        t.touch("lib/Darbar 64Kbps/Dumm Dumm.mp3");
        let library = t.0.join("lib");
        let portal = t.0.join("doc-1234");
        std::os::unix::fs::symlink(&library, &portal).unwrap();
        let before = snapshot(&library);
        let plan = plan(&library).unwrap();
        let through = Plan { ops: rebase(&plan.ops, &library, &portal), clashes: vec![] };
        assert!(through.ops.iter().all(|op| op.from.starts_with(&portal) && op.to.starts_with(&portal)));
        let journal = t.0.join("rename-1.jsonl");
        apply(&through, &portal, &library, &journal).unwrap();
        let original = library.join("Leo - MassTamilan.dev/Naa-Ready-MassTamilan.dev.mp3");
        let tidy = map_path(&plan.ops, &original);
        assert_ne!(tidy, original);
        assert!(tidy.exists());
        // The app's queue holds library paths: the journal gives the renames back in those terms
        let back = journal_ops(&journal).unwrap();
        assert!(back.iter().all(|op| op.from.starts_with(&library) && op.to.starts_with(&library)));
        assert_eq!(map_path(&back, &tidy), original);
        undo(&journal).unwrap();
        assert_eq!(snapshot(&library), before);
    }

    #[test]
    fn rebase_leaves_other_paths_alone() {
        let ops = vec![Rename { from: "/m/a".into(), to: "/m/b".into(), is_dir: false },
                       Rename { from: "/x/a".into(), to: "/x/b".into(), is_dir: false }];
        let r = rebase(&ops, Path::new("/m"), Path::new("/run/user/1000/doc/1/m"));
        assert_eq!(r[0].from, PathBuf::from("/run/user/1000/doc/1/m/a"));
        assert_eq!(r[1].from, PathBuf::from("/x/a"));
        assert_eq!(rebase(&[Rename { from: "/m".into(), to: "/m".into(), is_dir: true }], Path::new("/m"), Path::new("/p"))[0].to, PathBuf::from("/p"));
    }

    #[test]
    fn never_overwrites_a_name_that_appeared_later() {
        let t = TempDir::new("overwrite");
        t.touch("lib/Leo - MassTamilan.dev/a.mp3");
        let library = t.0.join("lib");
        let plan = plan(&library).unwrap();
        t.touch("lib/Leo/b.mp3");
        let err = apply(&plan, &library, &library, &t.0.join("rename-1.jsonl")).unwrap_err();
        assert_eq!((err.0, err.1.kind()), (0, io::ErrorKind::AlreadyExists));
        assert!(t.0.join("lib/Leo - MassTamilan.dev/a.mp3").exists());
        assert!(t.0.join("lib/Leo/b.mp3").exists());
    }

    #[test]
    fn paths_follow_the_renames() {
        let ops = vec![
            Rename { from: "/m/Leo - X.dev/a-X.dev.mp3".into(), to: "/m/Leo - X.dev/a.mp3".into(), is_dir: false },
            Rename { from: "/m/Leo - X.dev".into(), to: "/m/Leo".into(), is_dir: true },
        ];
        assert_eq!(map_path(&ops, Path::new("/m/Leo - X.dev/a-X.dev.mp3")), PathBuf::from("/m/Leo/a.mp3"));
        assert_eq!(map_path(&ops, Path::new("/m/Leo - X.dev/b.mp3")), PathBuf::from("/m/Leo/b.mp3"));
        assert_eq!(map_path(&ops, Path::new("/m/Other/c.mp3")), PathBuf::from("/m/Other/c.mp3"));
    }

    /// Prints the number of planned changes for RYUON_RENAME_DIR (used by tools/test_rename.py)
    #[test]
    #[ignore]
    fn print_plan_count() {
        let dir = std::env::var("RYUON_RENAME_DIR").expect("RYUON_RENAME_DIR");
        println!("RYUON_PLAN_COUNT={}", plan(Path::new(&dir)).unwrap().ops.len());
    }

    /// Plans, applies and undoes on a copy of a whole library made of small dummy files.
    /// RYUON_RENAME_TSV=tags.tsv (first column: path relative to the library), --ignored --nocapture
    #[test]
    #[ignore]
    fn library_fixture_roundtrip() {
        let tsv = std::env::var("RYUON_RENAME_TSV").expect("RYUON_RENAME_TSV");
        let t = TempDir::new("fixture");
        let library = t.0.join("Music");
        for line in fs::read_to_string(tsv).unwrap().lines() {
            let rel = line.split('\t').next().unwrap();
            let p = library.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, rel).unwrap();
        }
        let before = snapshot(&library);
        let plan = plan(&library).unwrap();
        println!("folders: {}, files: {}, clashes: {}", plan.n_folders(), plan.n_files(), plan.clashes.len());
        for c in &plan.clashes {
            println!("CLASH\t{}", rel(&library, c));
        }
        for op in &plan.ops {
            println!("{}\t{}\t=>\t{}", if op.is_dir { "DIR" } else { "FILE" }, rel(&library, &op.from), rel(&library, &op.to));
        }
        let journal = t.0.join("rename-1.jsonl");
        apply(&plan, &library, &library, &journal).unwrap();
        let after = snapshot(&library);
        assert_eq!(after.iter().filter(|(p, _)| !p.ends_with('/')).count(), before.iter().filter(|(p, _)| !p.ends_with('/')).count());
        // every file still holds its own contents, at the path the plan says
        for (rel_before, contents) in before.iter().filter(|(p, _)| !p.ends_with('/')) {
            let new = map_path(&plan.ops, &library.join(rel_before));
            assert_eq!(&fs::read_to_string(&new).unwrap(), contents);
        }
        undo(&journal).unwrap();
        assert_eq!(snapshot(&library), before);
        println!("round trip OK: {} entries", before.len());
    }
}
