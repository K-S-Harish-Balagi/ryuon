// SPDX-FileCopyrightText: 2026  Harish
// SPDX-License-Identifier: GPL-3.0-or-later

//! On Windows, puts the app icon and version details into ryuon.exe (Explorer, the taskbar
//! and the installer show them). Uses windres from the MinGW toolchain; does nothing on Linux.

use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=windows/ryuon.ico");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let numbers: Vec<u32> = version
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|n| n.parse().ok())
        .chain(std::iter::repeat(0))
        .take(4)
        .collect();
    let comma = numbers.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    let icon = root.join("windows").join("ryuon.ico");

    let rc = format!(
        r#"1 ICON "{icon}"
1 VERSIONINFO
FILEVERSION {comma}
PRODUCTVERSION {comma}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "K S Harish Balagi"
      VALUE "FileDescription", "Ryuon music player"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "ryuon"
      VALUE "LegalCopyright", "GPL-3.0-or-later. A fork of Amberol by Emmanuele Bassi"
      VALUE "OriginalFilename", "ryuon.exe"
      VALUE "ProductName", "Ryuon"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#,
        icon = icon.display().to_string().replace('\\', "/"),
    );
    let rc_path = out.join("ryuon.rc");
    fs::write(&rc_path, rc).unwrap();

    let object = out.join("ryuon-resources.o");
    let windres = env::var("WINDRES").unwrap_or_else(|_| "windres".into());
    let status = Command::new(&windres)
        .arg(&rc_path)
        .arg("-O")
        .arg("coff")
        .arg("-o")
        .arg(&object)
        .status()
        .unwrap_or_else(|e| panic!("could not run {}: {}", windres, e));
    assert!(status.success(), "{} failed on {}", windres, rc_path.display());
    println!("cargo:rustc-link-arg-bins={}", object.display());
}
