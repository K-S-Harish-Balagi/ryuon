// SPDX-FileCopyrightText: 2022  Emmanuele Bassi
// SPDX-License-Identifier: GPL-3.0-or-later

// A release build on Windows opens as a window only, without a console behind it
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod application;
mod audio;
mod config;
mod cover_picture;
mod drag_overlay;
mod i18n;
mod library;
mod marquee;
mod playback_control;
mod playlist_view;
mod queue_row;
mod rename;
mod search;
mod song_cover;
mod song_details;
mod sort;
mod tidy;
mod utils;
mod volume_control;
mod waveform_view;
mod window;

use std::env;

use config::{APPLICATION_ID, GETTEXT_PACKAGE, LOCALEDIR, PKGDATADIR, PROFILE};
use gettextrs::{bind_textdomain_codeset, bindtextdomain, setlocale, textdomain, LocaleCategory};
use gtk::{gio, glib, prelude::*};
use log::{debug, error, LevelFilter};

use self::application::Application;

// Where the installed data lives: the configured path on Linux; on Windows the share folder
// next to the program (bin\ryuon.exe, share\locale, share\ryuon), wherever it was installed
fn installed_dir(configured: &str, name: &str) -> std::path::PathBuf {
    #[cfg(windows)]
    if let Some(share) = env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.parent()?.join("share")))
    {
        return share.join(name);
    }
    let _ = name;
    std::path::PathBuf::from(configured)
}

fn main() -> glib::ExitCode {
    let mut builder = pretty_env_logger::formatted_builder();
    if APPLICATION_ID.ends_with("Devel") {
        builder.filter(Some("ryuon"), LevelFilter::Debug);
    } else {
        builder.filter(Some("ryuon"), LevelFilter::Info);
    }
    builder.init();

    // Set up gettext translations
    debug!("Setting up locale data");
    setlocale(LocaleCategory::LcAll, "");

    bindtextdomain(GETTEXT_PACKAGE, installed_dir(LOCALEDIR, "locale")).expect("Unable to bind the text domain");
    bind_textdomain_codeset(GETTEXT_PACKAGE, "UTF-8")
        .expect("Unable to set the text domain encoding");
    textdomain(GETTEXT_PACKAGE).expect("Unable to switch to the text domain");

    debug!("Setting up pulseaudio environment");
    let app_id = APPLICATION_ID.trim_end_matches(".Devel");
    env::set_var("PULSE_PROP_application.icon_name", app_id);
    env::set_var("PULSE_PROP_application.metadata().name", "Ryuon");
    env::set_var("PULSE_PROP_media.role", "music");

    debug!("Loading resources");
    let resources = match env::var("MESON_DEVENV") {
        Err(_) => gio::Resource::load(installed_dir(PKGDATADIR, "ryuon").join("ryuon.gresource"))
            .expect("Unable to find ryuon.gresource"),
        Ok(_) => match env::current_exe() {
            Ok(path) => {
                let mut resource_path = path;
                resource_path.pop();
                resource_path.push("ryuon.gresource");
                gio::Resource::load(&resource_path)
                    .expect("Unable to find ryuon.gresource in devenv")
            }
            Err(err) => {
                error!("Unable to find the current path: {}", err);
                return glib::ExitCode::FAILURE;
            }
        },
    };
    gio::resources_register(&resources);

    debug!("Setting up application (profile: {})", &PROFILE);
    glib::set_application_name("Ryuon");
    glib::set_program_name(Some("ryuon"));

    gst::init().expect("Failed to initialize gstreamer");

    let ctx = glib::MainContext::default();
    let _guard = ctx.acquire().unwrap();

    Application::new().run()
}
