// SPDX-FileCopyrightText: 2026  Harish
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tidy names for display: remove download-site tags, bitrate and format labels, and
//! underscores or hyphens used instead of spaces. Display only: nothing here touches files.
//!
//! Examples from a real library:
//! - `Coolie-320kbps-MassTamilan` becomes `Coolie`
//! - `Naa Ready - MassTamilan.dev` becomes `Naa Ready`
//! - `[iSongs.info] 04 - Poyiraa Mama` becomes `Poyiraa Mama` (as a title)
//! - `Anaganaga-Oka-Raju-(2026)-DFLAC_128Kbps` becomes `Anaganaga Oka Raju (2026)`

use std::sync::atomic::{AtomicBool, Ordering};

use once_cell::sync::Lazy;
use regex::Regex;

static ENABLED: AtomicBool = AtomicBool::new(true);

/// Whether names are tidied on screen (the `tidy-names` setting)
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

// Known download sites, with or without a domain ending, plus any word that ends in a
// common domain ending ("TeluguWap.Net", "isaiminiya.net").
const SITE: &str = r"(?:mass\s?tamilan|mass\s?telugu|mp3\s?bhai|sen\s?songs\s?mp3|isongs|isaimini(?:ya)?|starmusiq|tamilwire|telugu\s?wap|naa\s?songs|pagal\s?world)(?:\.[a-z]{2,4})?|[a-z0-9]+\.(?:com|net|org|info|co|in|fm|dev|io|so|day|me|cc|biz|xyz|site|online|live|pro|vip|top|ws|to|tv|club|mobi)";
// Bitrate and lossless format labels: "64Kbps", "320 kbps", "DFLAC 128Kbps", "ALAC"
const QUALITY: &str = r"(?:(?:d?flac|alac)\s*)?\d{2,4}\s*kbps|d?flac|alac";

static BRACKETED_SITE: Lazy<Regex> =
    Lazy::new(|| Regex::new(&format!(r"(?i)[\[(][^\])]*\b(?:{SITE})\b[^\])]*[\])]")).unwrap());
static TRAILING_TAG: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(r"(?i)(?:^|[\s\-~:.]+)(?:{SITE}|{QUALITY})[\s\-~:.]*$")).unwrap()
});
static TRACK_NUMBER: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\d{1,3}\s*[-.]\s*(\p{L})").unwrap());
static TIGHT_BRACKET: Lazy<Regex> = Lazy::new(|| Regex::new(r"(\S)\(").unwrap());
static COMMA: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s*,\s*").unwrap());
static SPACES: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").unwrap());

fn trim_separators(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || matches!(c, '-' | '~' | ':'))
}

/// Repairs UTF-16 text read with the wrong byte order, which shows as CJK characters
/// ("慒祭⁡敂牨ⱡ" for "Ramya Behara"): only when every character's two bytes are printable
/// ASCII, which real CJK text of a few characters practically never is.
fn fix_swapped_utf16(s: &str) -> Option<String> {
    let s = s.trim_end_matches(['\0', ' ']);
    if s.chars().count() < 3 || s.is_ascii() {
        return None;
    }
    let printable = |b: u32| (0x20..0x7f).contains(&b);
    let mut out = String::new();
    for c in s.chars() {
        let (hi, lo) = ((c as u32) >> 8, (c as u32) & 0xff);
        if (c as u32) > 0xffff || !printable(lo) || !(printable(hi) || hi == 0) {
            return None;
        }
        out.push(char::from(lo as u8));
        if hi != 0 {
            out.push(char::from(hi as u8));
        }
    }
    Some(out)
}

fn clean(name: &str, title: bool) -> String {
    let repaired = fix_swapped_utf16(name);
    let name = repaired.as_deref().unwrap_or(name);
    // Hyphens stand for spaces only in names written without any spaces
    // ("Good-Bad-Ugly-128kbps-MassTamilan", but not "Ez-Ez - Mp3Bhai")
    let hyphens_are_spaces = !name.trim().contains(' ');
    let mut s = name.replace('_', " ");
    s = BRACKETED_SITE.replace_all(&s, " ").into_owned();
    // Tags pile up at the end ("Coolie-320kbps-MassTamilan"): peel them one at a time
    loop {
        let next = TRAILING_TAG.replace(&s, "").into_owned();
        if next == s {
            break;
        }
        s = next;
    }
    s = trim_separators(&s).to_string();
    if title {
        s = TRACK_NUMBER.replace(&s, "$1").into_owned();
    }
    if hyphens_are_spaces {
        s = s.replace('-', " ");
    }
    s = TIGHT_BRACKET.replace_all(&s, "$1 (").into_owned();
    s = SPACES.replace_all(&s, " ").into_owned();
    let s = trim_separators(&s);
    if s.is_empty() { name.trim().to_string() } else { s.to_string() }
}

/// A folder, album or general name
pub fn name(name: &str) -> String {
    clean(name, false)
}

/// A song title: as `name`, and also drops a leading track number ("04 - Poyiraa Mama")
pub fn title(title: &str) -> String {
    clean(title, true)
}

/// An artist list: as `name`, with commas written ", ". None if only a site was left.
pub fn artist(artist: &str) -> Option<String> {
    let cleaned = clean(artist, false);
    if TRAILING_TAG.is_match(&cleaned) && TRAILING_TAG.replace(&cleaned, "").trim().is_empty() {
        return None;
    }
    Some(COMMA.replace_all(&cleaned, ", ").into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_names() {
        for (raw, want) in [
            ("Coolie-320kbps-MassTamilan", "Coolie"),
            ("Aasa Kooda (Indie) - MassTamilan", "Aasa Kooda (Indie)"),
            ("Anaganaga-Oka-Raju-(2026)-DFLAC_128Kbps", "Anaganaga Oka Raju (2026)"),
            ("The-Rajasaab-(2026)-ALAC_320Kbps", "The Rajasaab (2026)"),
            ("Anbarivu-128kbps-MassTamilan.fm", "Anbarivu"),
            ("Ayalaan - MassTamilan.io", "Ayalaan"),
            ("Bhaskar_Oru_Rascal", "Bhaskar Oru Rascal"),
            ("Darbar 64Kbps", "Darbar"),
            ("Hero (Tamil) 64Kbps", "Hero (Tamil)"),
            ("Junga_64Kbps", "Junga"),
            ("Kaththi_(2014)", "Kaththi (2014)"),
            ("Hanu man(2024)", "Hanu man (2024)"),
            ("Maharshi (2019) - 128KBPS", "Maharshi (2019)"),
            ("Veeran - Masstamilan.DAY", "Veeran"),
            ("Dacoit-320kbps-MassTelugu", "Dacoit"),
            ("Dhurandhar-The-Revenge-320kbps-Mp3Bhai", "Dhurandhar The Revenge"),
            ("Silukkuvaarpatti_Singam_64Kbps", "Silukkuvaarpatti Singam"),
            ("K.G.F- Chapter 1 - MassTamilan.org", "K.G.F- Chapter 1"),
            ("Ponniyin Selvan Part-1 (PS1) - MassTamilan.dev", "Ponniyin Selvan Part-1 (PS1)"),
            ("Naam - Adi Penne Indie - MassTamilan.fm", "Naam - Adi Penne Indie"),
            ("Saguni-320kbps-MassTamilan.com", "Saguni"),
            ("Viswasam_128Kbps", "Viswasam"),
            ("Mr. Local", "Mr. Local"),
            ("Retro 2025", "Retro 2025"),
            ("Sarkaru vaari paata", "Sarkaru vaari paata"),
        ] {
            assert_eq!(name(raw), want, "{raw}");
        }
    }

    #[test]
    fn song_titles() {
        for (raw, want) in [
            ("Mr. Perfect-SenSongsMp3.Co", "Mr. Perfect"),
            ("Jai Balayya :: SenSongsMp3.Com", "Jai Balayya"),
            ("Bhimavaram Balma - TeluguWap.Net", "Bhimavaram Balma"),
            ("Lojakku Mojakku :: isaiminiya.net", "Lojakku Mojakku"),
            ("Breakup Song -- isaiminiya.net", "Breakup Song"),
            ("[iSongs.info] 04 - Poyiraa Mama", "Poyiraa Mama"),
            ("2-Uppenantha-SenSongsMp3.Co", "Uppenantha"),
            ("03_-_Andhra_To_Telangana", "Andhra To Telangana"),
            ("Mr.-Local-Theme-MassTamilan.org", "Mr. Local Theme"),
            ("Soul-Of-Varisu-MassTamilan.dev", "Soul Of Varisu"),
            ("Hukum - Thalaivar Alappara - MassTamilan.dev", "Hukum - Thalaivar Alappara"),
            ("Yennuyire(Female version) :: SenSongsMp3.Com", "Yennuyire (Female version)"),
            ("Samajavaragamana - Female", "Samajavaragamana - Female"),
            ("Not A Teaser (Theme)", "Not A Teaser (Theme)"),
            ("3 Idiots", "3 Idiots"),
            ("Sa Ri Ma Pa", "Sa Ri Ma Pa"),
            ("Ez-Ez - Mp3Bhai", "Ez-Ez"),
            ("Padara Padara [TeluguSongs.in]", "Padara Padara"),
            ("Bhaskar Ra Ra Ra Rascal Theme -5StarMusiQ.Com", "Bhaskar Ra Ra Ra Rascal Theme"),
        ] {
            assert_eq!(title(raw), want, "{raw}");
        }
    }

    #[test]
    fn artists() {
        assert_eq!(artist("Anirudh Ravichander - MassTamilan.dev").as_deref(), Some("Anirudh Ravichander"));
        assert_eq!(artist("Allu Arjun,Sruthi Hassan").as_deref(), Some("Allu Arjun, Sruthi Hassan"));
        assert_eq!(
            artist("Shashwat Sachdev, Diljit Dosanjh - Mp3Bhai").as_deref(),
            Some("Shashwat Sachdev, Diljit Dosanjh")
        );
        assert_eq!(artist("MassTamilan.dev"), None);
        assert_eq!(artist("S P Charan , M L R Karthikeyan").as_deref(), Some("S P Charan, M L R Karthikeyan"));
    }

    #[test]
    fn swapped_utf16_is_repaired() {
        // "Ramya Behara, Bhargavi Pillai" stored as UTF-16 and read with the wrong byte order
        let garbled: String = "Ramya Behara, Bhargavi Pillai "
            .as_bytes()
            .chunks(2)
            .map(|p| char::from_u32(p[0] as u32 | (p[1] as u32) << 8).unwrap())
            .collect();
        assert_eq!(artist(&garbled).as_deref(), Some("Ramya Behara, Bhargavi Pillai"));
        // Real non-Latin names are left alone
        assert_eq!(name("இளையராஜா"), "இளையராஜா");
        assert_eq!(name("坂本龍一"), "坂本龍一");
    }

    #[test]
    fn never_empty() {
        assert_eq!(name("MassTamilan"), "MassTamilan");
        assert_eq!(title("128kbps"), "128kbps");
        assert_eq!(name("  "), "");
    }

    /// Prints before and after for a whole library. Run with
    /// RYUON_TIDY_TSV=tags.tsv (columns: path, title, artist, album) and --ignored --nocapture.
    #[test]
    #[ignore]
    fn library_report() {
        let path = std::env::var("RYUON_TIDY_TSV").expect("RYUON_TIDY_TSV");
        let text = std::fs::read_to_string(path).unwrap();
        let mut folders = std::collections::BTreeSet::new();
        let (mut changed, mut total) = (0, 0);
        for line in text.lines() {
            let cols: Vec<&str> = line.split('\t').collect();
            let rel = cols[0];
            folders.insert(rel.split('/').next().unwrap().to_string());
            let stem = rel.rsplit('/').next().unwrap().trim_end_matches(".mp3");
            let raw_title = if cols[1].trim().is_empty() { stem } else { cols[1] };
            let t = title(raw_title);
            total += 1;
            if t != raw_title {
                changed += 1;
                println!("TITLE\t{raw_title}\t=>\t{t}");
            }
            if cols.len() > 2 && !cols[2].is_empty() {
                let a = artist(cols[2]);
                if a.as_deref() != Some(cols[2]) {
                    println!("ARTIST\t{}\t=>\t{:?}", cols[2], a);
                }
            }
        }
        let mut fchanged = 0;
        for f in &folders {
            let n = name(f);
            if &n != f {
                fchanged += 1;
                println!("FOLDER\t{f}\t=>\t{n}");
            }
        }
        println!("titles changed: {changed} of {total}; folders changed: {fchanged} of {}", folders.len());
    }
}
