// SPDX-FileCopyrightText: 2022  Emmanuele Bassi
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    str::FromStr,
    time::{Duration, Instant},
};

use adw::prelude::{AdwDialogExt, AlertDialogExt, AlertDialogExtManual};
use adw::subclass::prelude::*;
use glib::{clone, closure_local};
use gtk::{gdk, gio, glib, prelude::*, CompositeTemplate};
use log::debug;

use crate::{
    audio::{AudioPlayer, RepeatMode, ReplayGainMode, Song},
    config::APPLICATION_ID,
    cover_picture::CoverSize,
    drag_overlay::DragOverlay,
    i18n::{i18n, i18n_f, i18n_k, ni18n_f, ni18n_k},
    library::{self, FolderSort, FolderStats},
    playback_control::PlaybackControl,
    playlist_view::PlaylistView,
    queue_row::QueueRow,
    rename,
    search::FuzzyFilter,
    song_cover::SongCover,
    song_details::SongDetails,
    sort::FuzzySorter,
    tidy,
    utils,
    volume_control::VolumeControl,
    waveform_view::WaveformView,
};

pub enum WindowMode {
    InitialView,
    MainView,
}

const ATTRIBUTE_HOST_PATH: &str = "xattr::document-portal.host-path";

#[derive(Clone, Copy, Debug)]
enum ReorderOp {
    PlayNext,
    Up,
    Down,
    FolderNext,
    FolderUp,
    FolderDown,
    Remove,
}

fn set_suggested(button: &gtk::Button, suggested: bool) {
    if suggested {
        button.add_css_class("suggested-action");
    } else {
        button.remove_css_class("suggested-action");
    }
}

mod imp {
    use glib::{ParamSpec, ParamSpecBoolean, ParamSpecEnum, Value};
    use once_cell::sync::Lazy;

    use super::*;

    #[derive(CompositeTemplate)]
    #[template(resource = "/io/github/k_s_harish_balagi/Ryuon/window.ui")]
    pub struct Window {
        // Template widgets
        #[template_child]
        pub drag_overlay: TemplateChild<DragOverlay>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub main_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub status_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub song_cover: TemplateChild<SongCover>,
        #[template_child]
        pub song_details: TemplateChild<SongDetails>,
        #[template_child]
        pub waveform_view: TemplateChild<WaveformView>,
        #[template_child]
        pub elapsed_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub remaining_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub playback_control: TemplateChild<PlaybackControl>,
        #[template_child]
        pub split_view: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub playlist_view: TemplateChild<PlaylistView>,
        #[template_child]
        pub add_folder_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub restore_playlist_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub open_library_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub folder_label: TemplateChild<gtk::Label>,

        pub provider: gtk::CssProvider,
        pub settings: gio::Settings,

        pub playlist_shuffled: Cell<bool>,
        pub playlist_visible: Cell<bool>,
        pub playlist_selection: Cell<bool>,
        pub playlist_search: Cell<bool>,
        pub replaygain_mode: Cell<ReplayGainMode>,

        pub playlist_filtermodel: RefCell<Option<gio::ListModel>>,

        // Song and position (in seconds) to go back to once the library has loaded
        pub pending_resume: RefCell<Option<(String, u64)>>,
        // Last folder announced to screen readers, so they hear folder changes, not every song
        pub announced_folder: Cell<usize>,
        // The move that undoes the last reorder: (start, len, to)
        pub undo_move: Cell<Option<(u32, u32, u32)>>,
        #[template_child]
        pub short_breakpoint: TemplateChild<adw::Breakpoint>,
        #[template_child]
        pub short_narrow_breakpoint: TemplateChild<adw::Breakpoint>,
        pub last_played_folder: RefCell<Option<PathBuf>>,
        // Songs are being added to the queue
        pub loading: Cell<bool>,

        pub notify_playing_id: RefCell<Option<glib::SignalHandlerId>>,
        pub notify_position_id: RefCell<Option<glib::SignalHandlerId>>,
        pub notify_song_id: RefCell<Option<glib::SignalHandlerId>>,
        pub notify_cover_id: RefCell<Option<glib::SignalHandlerId>>,
        pub notify_nsongs_id: RefCell<Option<glib::SignalHandlerId>>,
        pub notify_current_id: RefCell<Option<glib::SignalHandlerId>>,
        pub notify_peaks_id: RefCell<Option<glib::SignalHandlerId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "RyuonWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);

            klass.install_action("win.play", None, move |win, _, _| {
                debug!("Window::win.play()");
                if let Some(p) = win.player() {
                    p.toggle_play();
                }
            });
            klass.install_action("win.seek-backwards", None, move |win, _, _| {
                debug!("Window::win.seek-backwards");
                if let Some(p) = win.player() {
                    p.seek_backwards();
                }
            });
            klass.install_action("win.seek-forward", None, move |win, _, _| {
                debug!("Window::win.seek-forward");
                if let Some(p) = win.player() {
                    p.seek_forward();
                }
            });
            klass.install_action("win.previous", None, move |win, _, _| {
                debug!("Window::win.previous()");
                if let Some(p) = win.player() {
                    p.skip_previous();
                }
            });
            klass.install_action("win.next", None, move |win, _, _| {
                debug!("Window::win.next()");
                if let Some(p) = win.player() {
                    p.skip_next();
                }
            });
            klass.install_action("queue.repeat-mode", None, move |win, _, _| {
                debug!("Window::queue.repeat()");
                if let Some(p) = win.player() {
                    p.toggle_repeat_mode();
                }
            });
            klass.install_action_async("queue.add-song", None, |win, _, _| async move {
                debug!("Window::win.add-song()");
                let filters = gio::ListStore::new::<gtk::FileFilter>();
                let filter = gtk::FileFilter::new();
                gtk::FileFilter::set_name(&filter, Some(&i18n("Audio files")));
                filter.add_mime_type("audio/*");
                filters.append(&filter);

                let xdg_music = match glib::user_special_dir(glib::UserDirectory::Music) {
                    Some(p) => p,
                    None => glib::current_dir(),
                };

                let dialog = gtk::FileDialog::builder()
                    .accept_label(&i18n("_Add Song"))
                    .filters(&filters)
                    .initial_folder(&gio::File::for_path(&xdg_music))
                    .modal(true)
                    .title(&i18n("Open File"))
                    .build();

                if let Ok(files) = dialog.open_multiple_future(Some(&win)).await {
                    if files.n_items() == 0 {
                        win.add_toast(i18n("Unable to access files"));
                    } else {
                        win.add_files_to_queue(&files);
                    }
                }
            });
            klass.install_action_async("queue.add-folder", None, |win, _, _| async move {
                debug!("Window::win.add-folder()");
                let xdg_music = match glib::user_special_dir(glib::UserDirectory::Music) {
                    Some(p) => p,
                    None => glib::current_dir(),
                };

                let dialog = gtk::FileDialog::builder()
                    .accept_label(&i18n("_Add Folder"))
                    .initial_folder(&gio::File::for_path(&xdg_music))
                    .modal(true)
                    .title(&i18n("Open Folder"))
                    .build();

                if let Ok(files) = dialog.select_multiple_folders_future(Some(&win)).await {
                    if files.n_items() == 0 {
                        win.add_toast(i18n("Unable to access files"));
                    } else {
                        win.add_files_to_queue(&files);
                    }
                }
            });
            klass.install_action("queue.restore-playlist", None, move |win, _, _| {
                debug!("Window::queue.restore-playlist()");
                win.restore_playlist();
            });
            klass.install_action("win.preferences", None, move |win, _, _| {
                win.show_preferences();
            });
            klass.install_action("queue.rename-files", None, move |win, _, _| {
                win.rename_files();
            });
            klass.install_action("queue.undo-rename", None, move |win, _, _| {
                win.undo_rename();
            });
            klass.install_action("queue.open-library", None, move |win, _, _| {
                debug!("Window::queue.open-library()");
                win.open_library();
            });
            klass.install_action_async("queue.choose-library", None, |win, _, _| async move {
                debug!("Window::queue.choose-library()");
                let start = win.library_root().unwrap_or_else(|| {
                    glib::user_special_dir(glib::UserDirectory::Music).unwrap_or_else(glib::current_dir)
                });

                let dialog = gtk::FileDialog::builder()
                    .accept_label(&i18n("_Use as Library"))
                    .initial_folder(&gio::File::for_path(&start))
                    .modal(true)
                    .title(&i18n("Choose Music Library"))
                    .build();

                if let Ok(folder) = dialog.select_folder_future(Some(&win)).await {
                    match win.library_folder_path(&folder) {
                        Some(path) => {
                            win.imp()
                                .settings
                                .set_string("library-path", &path.to_string_lossy())
                                .expect("Unable to store library-path");
                            win.open_library();
                        }
                        None => win.add_toast(i18n("Unable to access files")),
                    }
                }
            });
            klass.install_action("win.copy", None, move |win, _, _| {
                debug!("Window::win.copy()");
                win.copy_song();
            });
            klass.install_action_async("win.open-directory", None, async move |win, _, _| {
                debug!("Window::win.open-directory()");
                win.open_song_directory().await
            });
            klass.install_action("queue.clear", None, move |win, _, _| {
                debug!("Window::queue.clear()");
                win.clear_queue();
            });
            klass.install_property_action("queue.toggle", "playlist-visible");
            klass.install_property_action("queue.shuffle", "playlist-shuffled");
            klass.install_property_action("queue.select", "playlist-selection");
            klass.install_property_action("queue.search", "playlist-search");
            klass.install_property_action("win.replaygain", "replaygain-mode");

            klass.install_action(
                "win.skip-to",
                Some(glib::VariantTy::UINT32),
                move |win, _, param| {
                    if let Some(pos) = param.and_then(u32::from_variant) {
                        if let Some(player) = win.player() {
                            player.skip_to(pos);
                            player.play();
                        }
                    }
                },
            );
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }

        fn new() -> Self {
            Self {
                song_details: TemplateChild::default(),
                song_cover: TemplateChild::default(),
                split_view: TemplateChild::default(),
                toast_overlay: TemplateChild::default(),
                drag_overlay: TemplateChild::default(),
                playback_control: TemplateChild::default(),
                waveform_view: TemplateChild::default(),
                elapsed_label: TemplateChild::default(),
                remaining_label: TemplateChild::default(),
                main_stack: TemplateChild::default(),
                status_page: TemplateChild::default(),
                add_folder_button: TemplateChild::default(),
                restore_playlist_button: TemplateChild::default(),
                open_library_button: TemplateChild::default(),
                folder_label: TemplateChild::default(),
                playlist_view: TemplateChild::default(),
                playlist_shuffled: Cell::new(false),
                playlist_visible: Cell::new(true),
                playlist_selection: Cell::new(false),
                playlist_search: Cell::new(false),
                playlist_filtermodel: RefCell::default(),
                pending_resume: RefCell::default(),
                announced_folder: Cell::new(0),
                undo_move: Cell::new(None),
                short_breakpoint: TemplateChild::default(),
                short_narrow_breakpoint: TemplateChild::default(),
                last_played_folder: RefCell::new(None),
                loading: Cell::new(false),
                replaygain_mode: Cell::new(ReplayGainMode::default()),
                provider: gtk::CssProvider::new(),
                settings: utils::settings_manager(),
                notify_playing_id: RefCell::new(None),
                notify_position_id: RefCell::new(None),
                notify_song_id: RefCell::new(None),
                notify_cover_id: RefCell::new(None),
                notify_nsongs_id: RefCell::new(None),
                notify_current_id: RefCell::new(None),
                notify_peaks_id: RefCell::new(None),
            }
        }
    }

    impl ObjectImpl for Window {
        fn constructed(&self) {
            self.parent_constructed();

            if APPLICATION_ID.ends_with("Devel") {
                self.obj().add_css_class("devel");
            }
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecBoolean::builder("playlist-shuffled").build(),
                    ParamSpecBoolean::builder("playlist-visible").build(),
                    ParamSpecBoolean::builder("playlist-selection").build(),
                    ParamSpecBoolean::builder("playlist-search").build(),
                    ParamSpecEnum::builder::<ReplayGainMode>("replaygain-mode").build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            let obj = self.obj();
            match pspec.name() {
                "playlist-shuffled" => obj.set_playlist_shuffled(value.get::<bool>().unwrap()),
                "playlist-visible" => obj.set_playlist_visible(value.get::<bool>().unwrap()),
                "playlist-selection" => obj.set_playlist_selection(value.get::<bool>().unwrap()),
                "playlist-search" => obj.set_playlist_search(value.get::<bool>().unwrap()),
                "replaygain-mode" => obj.set_replaygain(value.get::<ReplayGainMode>().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            let obj = self.obj();
            match pspec.name() {
                "playlist-shuffled" => obj.playlist_shuffled().to_value(),
                "playlist-visible" => obj.playlist_visible().to_value(),
                "playlist-selection" => obj.playlist_selection().to_value(),
                "playlist-search" => obj.playlist_search().to_value(),
                "replaygain-mode" => obj.replaygain().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl Window {
    pub fn new<P: IsA<gtk::Application>>(application: &P) -> Self {
        let win = glib::Object::builder::<Window>()
            .property("application", application)
            .build();

        win.setup_waveform();
        win.setup_actions();
        win.setup_short_layout();
        win.update_undo_rename();
        win.setup_playlist();
        win.setup_drop_target();
        win.setup_provider();
        win.bind_state();
        win.bind_queue();
        win.connect_signals();
        win.restore_window_state();
        win.set_initial_state();

        win
    }

    fn player(&self) -> Option<Rc<AudioPlayer>> {
        if let Some(app) = self.application() {
            let player = app
                .downcast::<crate::application::Application>()
                .unwrap()
                .player();
            return Some(player);
        }

        None
    }

    fn setup_actions(&self) {
        // Bound to the setting, so the menu and the Preferences switch agree
        self.add_action(&self.imp().settings.create_action("enable-recoloring"));

        // Reordering, from the row menu, drag and drop and keyboard shortcuts. These live in
        // the window's action map (not the class) so they are also reachable over D-Bus, which
        // tools/test_reorder.py uses to test them without input.
        let mut entries = Vec::new();
        for (name, op) in [
            ("play-next", ReorderOp::PlayNext),
            ("move-up", ReorderOp::Up),
            ("move-down", ReorderOp::Down),
            ("folder-next", ReorderOp::FolderNext),
            ("folder-up", ReorderOp::FolderUp),
            ("folder-down", ReorderOp::FolderDown),
            ("remove-at", ReorderOp::Remove),
        ] {
            entries.push(
                gio::ActionEntry::builder(name)
                    .parameter_type(Some(glib::VariantTy::UINT32))
                    .activate(move |this: &Window, _, param| {
                        if let Some(pos) = param.and_then(u32::from_variant) {
                            this.reorder(op, pos);
                        }
                    })
                    .build(),
            );
        }
        // Song Info: from the row menu (a queue position) or for the playing song
        entries.push(
            gio::ActionEntry::builder("song-info")
                .parameter_type(Some(glib::VariantTy::UINT32))
                .activate(|this: &Window, _, param| {
                    if let Some(pos) = param.and_then(u32::from_variant) {
                        this.show_song_info(pos);
                    }
                })
                .build(),
        );
        entries.push(
            gio::ActionEntry::builder("current-song-info")
                .activate(|this: &Window, _, _| {
                    if let Some(pos) = this.player().and_then(|p| p.queue().current_song_index()) {
                        this.show_song_info(pos);
                    }
                })
                .build(),
        );
        entries.push(
            gio::ActionEntry::builder("undo-move")
                .activate(|this: &Window, _, _| this.undo_move())
                .build(),
        );
        // Songs or Folders in the sidebar (the toggle in the playlist header)
        entries.push(
            gio::ActionEntry::builder("show-folders")
                .state(false.to_variant())
                .activate(|this: &Window, action, _| {
                    let show = !action.state().and_then(|s| s.get::<bool>()).unwrap_or(false);
                    action.set_state(&show.to_variant());
                    this.imp().playlist_view.set_folders_visible(show);
                    this.imp().playlist_view.sync_tabs(show);
                    this.refresh_folder_list();
                })
                .build(),
        );
        entries.push(
            gio::ActionEntry::builder("reset-order")
                .activate(|this: &Window, _, _| this.reset_order())
                .build(),
        );
        self.add_action_entries(entries);

        // Sort Folders menu: a radio group bound to the library-sort setting
        let settings = &self.imp().settings;
        self.add_action(&settings.create_action("library-sort"));
        settings.connect_changed(
            Some("library-sort"),
            clone!(
                #[weak(rename_to = win)]
                self,
                move |_, _| win.apply_sort()
            ),
        );

        // For tools/test_rename.py only: these exist when started with RYUON_TEST_HOOKS=1,
        // so a normal session exposes no way to rename files over D-Bus
        if std::env::var_os("RYUON_TEST_HOOKS").is_some() {
            self.add_action_entries([
                gio::ActionEntry::builder("test-rename-preview")
                    .activate(|this: &Window, _, _| this.rename_files())
                    .build(),
                // Renames only if the number of changes matches, like the typed confirmation
                gio::ActionEntry::builder("test-rename-confirmed")
                    .parameter_type(Some(glib::VariantTy::UINT32))
                    .activate(|this: &Window, _, param| {
                        let Some(root) = this.library_root() else { return };
                        let Ok(plan) = rename::plan(&root) else { return };
                        if param.and_then(u32::from_variant) == Some(plan.ops.len() as u32) {
                            this.apply_rename(&plan, &root);
                        }
                    })
                    .build(),
                gio::ActionEntry::builder("test-undo-rename")
                    .activate(|this: &Window, _, _| this.undo_rename())
                    .build(),
                gio::ActionEntry::builder("test-preferences")
                    .activate(|this: &Window, _, _| this.show_preferences())
                    .build(),
            ]);
        }
    }

    fn setup_short_layout(&self) {
        let imp = self.imp();
        for breakpoint in [imp.short_breakpoint.get(), imp.short_narrow_breakpoint.get()] {
            breakpoint.connect_apply(clone!(
                #[weak(rename_to = win)]
                self,
                move |_| win.imp().song_cover.album_image().set_cover_size(CoverSize::Medium)
            ));
            breakpoint.connect_unapply(clone!(
                #[weak(rename_to = win)]
                self,
                move |_| win.imp().song_cover.album_image().set_cover_size(CoverSize::Large)
            ));
        }
    }

    fn setup_waveform(&self) {
        if let Some(player) = self.player() {
            let peaks = player.waveform_generator().peaks();
            self.imp().waveform_view.set_peaks(peaks);

            let notify_peaks_id = player.waveform_generator().connect_notify_local(
                Some("has-peaks"),
                clone!(
                    #[weak(rename_to = win)]
                    self,
                    move |gen, _| {
                        let peaks = gen.peaks();
                        win.imp().waveform_view.set_peaks(peaks);
                    }
                ),
            );
            self.imp().notify_peaks_id.replace(Some(notify_peaks_id));
        }
    }

    fn unbind_waveform(&self) {
        if let Some(player) = self.player() {
            if let Some(id) = self.imp().notify_peaks_id.take() {
                player.waveform_generator().disconnect(id);
            }
        }
    }

    fn restore_window_state(&self) {
        let settings = utils::settings_manager();
        let width = settings.int("window-width");
        let height = settings.int("window-height");
        self.set_default_size(width, height);
    }

    fn reset_queue(&self) {
        self.set_playlist_visible(false);
        self.set_playlist_shuffled(false);
        self.set_playlist_selection(false);
        self.update_style(None);
    }

    fn clear_queue(&self) {
        if let Some(p) = self.player() {
            p.clear_queue();
        }
    }

    fn playlist_visible(&self) -> bool {
        self.imp().playlist_visible.get()
    }

    fn set_playlist_visible(&self, visible: bool) {
        if visible != self.imp().playlist_visible.replace(visible) {
            self.imp().split_view.set_show_sidebar(visible);
            self.notify("playlist-visible");
        }
    }

    fn playlist_shuffled(&self) -> bool {
        self.imp().playlist_shuffled.get()
    }

    fn set_playlist_shuffled(&self, shuffled: bool) {
        let imp = self.imp();

        if shuffled != imp.playlist_shuffled.replace(shuffled) {
            if let Some(player) = self.player() {
                let queue = player.queue();
                let state = player.state();
                let reset_song = queue.is_first_song() && !state.playing();

                queue.set_shuffled(shuffled);

                if reset_song {
                    player.skip_to(0);
                }
            }

            self.notify("playlist-shuffled");
            self.update_folder_label();
        }
    }

    fn playlist_selection(&self) -> bool {
        self.imp().playlist_selection.get()
    }

    fn set_playlist_selection(&self, selection: bool) {
        let imp = self.imp();

        if selection != imp.playlist_selection.replace(selection) {
            if !selection {
                if let Some(player) = self.player() {
                    let queue = player.queue();
                    queue.unselect_all_songs();
                }
            }

            self.imp()
                .playlist_view
                .queue_actionbar()
                .set_revealed(selection);

            self.notify("playlist-selection");
        }
    }

    fn playlist_search(&self) -> bool {
        self.imp().playlist_search.get()
    }

    fn set_playlist_search(&self, search: bool) {
        let imp = self.imp();

        if search != imp.playlist_search.replace(search) {
            if search {
                self.set_playlist_visible(true);
            }
            imp.playlist_view.set_search(search);
            self.notify("playlist-search");
        }
    }

    fn restore_playlist(&self) {
        if let Some(songs) = utils::load_cached_songs() {
            self.queue_songs(songs);
        }
    }

    // The music library: a folder of albums, queued folder by folder
    fn library_root(&self) -> Option<PathBuf> {
        let path = self.imp().settings.string("library-path");
        if path.is_empty() {
            None
        } else {
            Some(PathBuf::from(path.as_str()))
        }
    }

    pub fn open_library(&self) {
        // A second load would clear the queue under the first one and queue songs twice
        if self.imp().loading.get() {
            return;
        }

        let Some(root) = self.library_root() else {
            self.add_toast(i18n("Choose a music library first"));
            return;
        };

        let folder = gio::File::for_path(&root);
        if !folder.query_exists(gio::Cancellable::NONE) {
            self.add_toast(i18n_f("Music library not found: {}", &[&root.to_string_lossy()]));
            return;
        }

        // Look for songs before touching the queue, so an empty library keeps what is queued
        let mut files = utils::load_files_from_folder(&folder, true);

        match self.folder_sort() {
            // Songs or folders were moved: keep that order, new songs go at the end
            FolderSort::Manual => {
                if let Some(saved) = utils::load_cached_songs() {
                    let disk: Vec<String> = files.iter().map(|f| f.uri().to_string()).collect();
                    let saved: Vec<String> = saved.iter().map(|f| f.uri().to_string()).collect();
                    files = library::apply_saved_order(&disk, &saved)
                        .into_iter()
                        .map(|i| files[i].clone())
                        .collect();
                }
            }
            FolderSort::Name => {}
            sort => {
                let paths: Vec<PathBuf> = files.iter().filter_map(|f| f.path()).collect();
                if paths.len() == files.len() {
                    files = self
                        .sorted_order(&paths, sort)
                        .into_iter()
                        .map(|i| files[i as usize].clone())
                        .collect();
                }
            }
        }
        if files.is_empty() {
            self.add_toast(i18n("No songs found in the music library"));
            return;
        }

        // Opening the library replaces whatever is queued
        if let Some(player) = self.player() {
            if !player.queue().is_empty() {
                self.clear_queue();
            }
        }

        self.imp().pending_resume.replace(self.load_resume());
        self.queue_songs(files);
    }

    fn load_resume(&self) -> Option<(String, u64)> {
        let settings = &self.imp().settings;
        let uri = settings.string("resume-uri");
        if uri.is_empty() {
            None
        } else {
            Some((uri.to_string(), settings.uint64("resume-position")))
        }
    }

    // The resume point is kept by the application, so it also works without a window
    pub fn save_resume(&self) {
        if let Some(app) = self.application() {
            app.downcast::<crate::application::Application>()
                .unwrap()
                .save_resume();
        }
    }

    // All library and playback options in one place; switches are bound to the settings
    // Settings, laid out like the "Music library" board on the design canvas:
    // Location, Playback order, Names, Playback
    fn show_preferences(&self) {
        use adw::prelude::{ActionRowExt, PreferencesDialogExt, PreferencesGroupExt, PreferencesPageExt, PreferencesRowExt};
        let settings = &self.imp().settings;
        let dialog = adw::PreferencesDialog::new();
        dialog.set_title(&i18n("Settings"));
        let page = adw::PreferencesPage::new();

        // Folder names in the library: counted and used for the tidy-names example.
        // One directory listing, only when Settings opens.
        let describe = |path: &str| -> (String, Option<(String, String)>) {
            if path.is_empty() {
                return (i18n("No music library chosen yet"), None);
            }
            let mut n = 0u32;
            let mut example = None;
            if let Ok(entries) = std::fs::read_dir(path) {
                let mut names: Vec<String> = entries
                    .flatten()
                    .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .filter(|n| !n.starts_with('.'))
                    .collect();
                names.sort();
                n = names.len() as u32;
                example = names
                    .iter()
                    .find(|f| tidy::name(f) != **f)
                    .map(|f| (f.clone(), tidy::name(f)));
            }
            (ni18n_f("One folder", "{} folders", n, &[&n.to_string()]), example)
        };

        // Location
        let library = adw::PreferencesGroup::builder().title(i18n("Location")).build();
        let path = settings.string("library-path");
        let (count, example) = describe(&path);
        let location = adw::ActionRow::builder()
            .title(if path.is_empty() { i18n("Not chosen") } else { path.to_string() })
            .subtitle(&count)
            .use_markup(false)
            .title_selectable(true)
            .build();
        location.add_prefix(&gtk::Image::from_icon_name("folder-music-symbolic"));
        let choose = gtk::Button::builder()
            .label(i18n("_Change…"))
            .use_underline(true)
            .valign(gtk::Align::Center)
            .action_name("queue.choose-library")
            .build();
        location.add_suffix(&choose);
        library.add(&location);
        let play = adw::ButtonRow::builder()
            .title(i18n("Play Library"))
            .start_icon_name("media-playback-start-symbolic")
            .action_name("queue.open-library")
            .build();
        play.add_css_class("suggested-action");
        library.add(&play);
        let autoload = adw::SwitchRow::builder()
            .title(i18n("Open Library at Start"))
            .subtitle(i18n("When Ryuon starts with an empty playlist"))
            .build();
        settings.bind("library-autoload", &autoload, "active").build();
        library.add(&autoload);
        page.add(&library);

        // Playback order
        let order_group = adw::PreferencesGroup::builder().title(i18n("Playback Order")).build();
        let by_folder = adw::SwitchRow::builder()
            .title(i18n("Play Folder by Folder"))
            .subtitle(i18n("When one folder ends, the next one starts; songs in a folder play in file order"))
            .build();
        // The same thing as shuffle being off, so the shuffle button and this switch agree
        self.bind_property("playlist-shuffled", &by_folder, "active")
            .invert_boolean()
            .bidirectional()
            .sync_create()
            .build();
        order_group.add(&by_folder);
        let sorts = ["name", "newest", "most-songs", "recent", "manual"];
        let labels = [
            i18n("A–Z"),
            i18n("Newest First"),
            i18n("Most Songs"),
            i18n("Recently Played"),
            i18n("Manual Order"),
        ];
        let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let order = adw::ComboRow::builder()
            .title(i18n("Folder Order"))
            .model(&gtk::StringList::new(&label_refs))
            .build();
        settings
            .bind("library-sort", &order, "selected")
            .mapping(move |value, _| {
                let s: String = value.get()?;
                Some((sorts.iter().position(|x| *x == s).unwrap_or(0) as u32).to_value())
            })
            .set_mapping(move |value, _| {
                let i: u32 = value.get().ok()?;
                Some(sorts.get(i as usize)?.to_variant())
            })
            .build();
        order_group.add(&order);
        page.add(&order_group);

        // Names
        let names = adw::PreferencesGroup::builder().title(i18n("Names")).build();
        let tidy_row = adw::SwitchRow::builder()
            .title(i18n("Clean Up Folder and Song Names"))
            .subtitle(match &example {
                Some((from, to)) => format!("{from}  →  {to}"),
                None => i18n("Hide download-site tags, bitrates and underscores on screen"),
            })
            .subtitle_lines(2)
            .build();
        settings.bind("tidy-names", &tidy_row, "active").build();
        names.add(&tidy_row);
        let rename_row = adw::ButtonRow::builder()
            .title(i18n("Tidy Names…"))
            .action_name("queue.rename-files")
            .build();
        names.add(&rename_row);
        page.add(&names);

        // Choosing a library updates the rows while the dialog is open
        let handler = settings.connect_changed(
            Some("library-path"),
            clone!(
                #[weak]
                location,
                #[weak]
                tidy_row,
                move |settings, _| {
                    let path = settings.string("library-path");
                    let (count, example) = describe(&path);
                    location.set_title(&path);
                    location.set_subtitle(&count);
                    if let Some((from, to)) = example {
                        tidy_row.set_subtitle(&format!("{from}  →  {to}"));
                    }
                }
            ),
        );
        let handler = RefCell::new(Some(handler));
        dialog.connect_closed(clone!(
            #[weak]
            settings,
            move |_| {
                if let Some(handler) = handler.take() {
                    settings.disconnect(handler);
                }
            }
        ));

        // Playback
        let playback = adw::PreferencesGroup::builder().title(i18n("Playback")).build();
        let background = adw::SwitchRow::builder()
            .title(i18n("Background Playback"))
            .subtitle(i18n("Keep playing after the window is closed"))
            .build();
        settings.bind("background-play", &background, "active").build();
        playback.add(&background);
        let recolor = adw::SwitchRow::builder()
            .title(i18n("Match Cover Art"))
            .subtitle(i18n("Color the window from the cover of the playing song"))
            .build();
        settings.bind("enable-recoloring", &recolor, "active").build();
        playback.add(&recolor);
        page.add(&playback);

        dialog.add(&page);
        dialog.present(Some(self));
    }

    fn rename_journal_dir() -> PathBuf {
        glib::user_data_dir().join("ryuon").join("rename-journal")
    }

    pub fn update_undo_rename(&self) {
        let has_journal = rename::latest_journal(&Self::rename_journal_dir()).is_some();
        self.action_set_enabled("queue.undo-rename", has_journal);
    }

    // Renaming on disk needs write access, which the Flatpak only has when granted
    fn can_write(root: &std::path::Path) -> bool {
        let probe = root.join(".ryuon-write-test");
        let ok = std::fs::write(&probe, b"").is_ok();
        let _ = std::fs::remove_file(&probe);
        ok
    }

    fn show_write_permission_dialog(&self, root: &std::path::Path) {
        let command = format!(
            "flatpak override --user --filesystem={} {}",
            glib::shell_quote(root).to_string_lossy(),
            APPLICATION_ID
        );
        let dialog = adw::AlertDialog::new(
            Some(&i18n("Ryuon Cannot Rename Files Here")),
            Some(&i18n(
                "The music library is read-only for Ryuon. To allow renaming, run this command in a terminal, then restart Ryuon:",
            )),
        );
        let label = gtk::Label::builder()
            .label(&command)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .css_classes(["monospace"])
            .build();
        dialog.set_extra_child(Some(&label));
        dialog.add_responses(&[("copy", &i18n("_Copy Command")), ("close", &i18n("_Close"))]);
        dialog.set_default_response(Some("close"));
        dialog.set_close_response("close");
        dialog.connect_response(Some("copy"), clone!(
            #[weak(rename_to = win)]
            self,
            move |_, _| {
                win.clipboard().set_text(&command);
                win.add_toast(i18n("Command copied"));
            }
        ));
        dialog.present(Some(self));
    }

    // Preview of the tidy names on disk; renaming needs the number of changes typed in
    // The "Tidy names" dialog from the design canvas: a before → after preview of every
    // name that changes, clashes marked, and a choice between only changing how names look
    // (safe, nothing on disk changes) and renaming the files (typed confirmation, undo journal).
    fn rename_files(&self) {
        if self.imp().loading.get() {
            return;
        }
        let Some(root) = self.library_root() else {
            self.add_toast(i18n("Choose a music library first"));
            return;
        };
        let plan = match rename::plan(&root) {
            Ok(plan) => plan,
            Err(e) => {
                self.add_toast(i18n_f("Unable to read the music library: {}", &[&e.to_string()]));
                return;
            }
        };

        let settings = self.imp().settings.clone();
        let total = plan.ops.len().to_string();
        let page = gtk::Box::new(gtk::Orientation::Vertical, 10);
        page.set_margin_start(18);
        page.set_margin_end(18);
        page.set_margin_bottom(18);

        // Summary
        let summary = gtk::Label::builder().xalign(0.0).wrap(true).build();
        if plan.ops.is_empty() {
            summary.set_label(&i18n("Every folder and song name in the library is already tidy."));
        } else {
            let mut text = i18n_f(
                "{} folders and {} songs change",
                &[&plan.n_folders().to_string(), &plan.n_files().to_string()],
            );
            if !plan.clashes.is_empty() {
                text.push_str(" · ");
                text.push_str(&ni18n_f(
                    "one name clashes",
                    "{} names clash",
                    plan.clashes.len() as u32,
                    &[&plan.clashes.len().to_string()],
                ));
            }
            summary.set_label(&text);
        }
        summary.add_css_class("heading");
        page.append(&summary);

        // Before → after, one row per change (names only; the folder is in the tooltip)
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        list.update_property(&[gtk::accessible::Property::Label(&i18n("Changes"))]);
        for op in plan.ops.iter().take(400) {
            let name = |p: &std::path::Path| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.set_margin_top(6);
            row.set_margin_bottom(6);
            row.set_margin_start(10);
            row.set_margin_end(10);
            let from = gtk::Label::builder()
                .use_markup(true)
                .label(format!("<s>{}</s>", glib::markup_escape_text(&name(&op.from))))
                .xalign(0.0)
                .width_chars(26)
                .max_width_chars(26)
                .ellipsize(gtk::pango::EllipsizeMode::Middle)
                .css_classes(["dim-label"])
                .build();
            let arrow = gtk::Image::from_icon_name("go-next-symbolic");
            arrow.add_css_class("dim-label");
            let to = gtk::Label::builder()
                .label(name(&op.to))
                .xalign(0.0)
                .hexpand(true)
                .width_chars(18)
                .max_width_chars(22)
                .ellipsize(gtk::pango::EllipsizeMode::Middle)
                .css_classes(["heading"])
                .build();
            row.append(&gtk::Image::from_icon_name(if op.is_dir { "folder-symbolic" } else { "audio-x-generic-symbolic" }));
            row.append(&from);
            row.append(&arrow);
            row.append(&to);
            if plan.clashes.contains(&op.to) {
                let badge = gtk::Label::builder().label(i18n("same as another")).css_classes(["clash-badge", "caption"]).build();
                row.append(&badge);
            }
            row.set_tooltip_text(Some(&op.from.parent().unwrap_or(&root).strip_prefix(&root).unwrap_or(std::path::Path::new("")).to_string_lossy()));
            list.append(&row);
        }
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .min_content_height(240)
            .vexpand(true)
            .child(&list)
            .build();
        if !plan.ops.is_empty() {
            page.append(&scroller);
        }
        let lock = gtk::Label::builder()
            .label(i18n("Track numbers like “1-” always stay in the file: songs play in file name order. They are only hidden on screen."))
            .wrap(true)
            .xalign(0.0)
            .css_classes(["dim-label", "caption"])
            .build();
        page.append(&lock);

        // Mode: only how names look, or rename on disk
        let look = gtk::ToggleButton::with_label(&i18n("Only Change How Names Look"));
        let disk = gtk::ToggleButton::with_label(&i18n("Rename Files on Disk"));
        disk.set_group(Some(&look));
        look.set_active(true);
        disk.set_sensitive(!plan.ops.is_empty());
        let modes = gtk::Box::builder().homogeneous(true).css_classes(["linked"]).build();
        modes.append(&look);
        modes.append(&disk);
        page.append(&modes);
        let note = gtk::Label::builder().wrap(true).xalign(0.0).css_classes(["caption"]).build();
        page.append(&note);
        let entry = gtk::Entry::builder()
            .placeholder_text(i18n_f("Type {} to confirm", &[&total]))
            .input_purpose(gtk::InputPurpose::Digits)
            .visible(false)
            .build();
        entry.update_property(&[gtk::accessible::Property::Label(&i18n_f(
            "Type the number of changes, {}, to confirm",
            &[&total],
        ))]);
        page.append(&entry);

        // Footer
        let cancel = gtk::Button::with_mnemonic(&i18n("_Cancel"));
        let apply = gtk::Button::with_mnemonic(&i18n("_Apply"));
        apply.add_css_class("suggested-action");
        let footer = gtk::Box::builder().spacing(10).halign(gtk::Align::End).build();
        footer.append(&cancel);
        footer.append(&apply);
        page.append(&footer);

        let update = clone!(
            #[weak]
            disk,
            #[weak]
            note,
            #[weak]
            entry,
            #[weak]
            apply,
            #[strong]
            total,
            move || {
                if disk.is_active() {
                    note.set_label(&i18n(
                        "Changes the real file and folder names. Saves an undo list first, never deletes anything, and adds a number when a name is taken.",
                    ));
                    entry.set_visible(true);
                    apply.set_label(&i18n("_Rename"));
                    apply.remove_css_class("suggested-action");
                    apply.add_css_class("destructive-action");
                    apply.set_sensitive(entry.text().trim() == total);
                } else {
                    note.set_label(&i18n("Safe: your files are not touched, so clashes never matter."));
                    entry.set_visible(false);
                    apply.set_label(&i18n("_Apply"));
                    apply.remove_css_class("destructive-action");
                    apply.add_css_class("suggested-action");
                    apply.set_sensitive(true);
                }
            }
        );
        update();
        let u = update.clone();
        disk.connect_toggled(move |_| u());
        let u = update.clone();
        entry.connect_changed(move |_| u());

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&adw::HeaderBar::new());
        toolbar.set_content(Some(&page));
        let dialog = adw::Dialog::builder()
            .title(i18n("Tidy Names"))
            .content_width(620)
            .content_height(if plan.ops.is_empty() { 300 } else { 600 })
            .child(&toolbar)
            .build();

        cancel.connect_clicked(clone!(
            #[weak]
            dialog,
            move |_| {
                dialog.close();
            }
        ));
        let plan = Rc::new(plan);
        apply.connect_clicked(clone!(
            #[weak(rename_to = win)]
            self,
            #[weak]
            dialog,
            #[weak]
            disk,
            move |_| {
                if disk.is_active() {
                    if !Self::can_write(&root) {
                        dialog.close();
                        win.show_write_permission_dialog(&root);
                        return;
                    }
                    dialog.close();
                    win.apply_rename(&plan, &root);
                } else {
                    let _ = settings.set_boolean("tidy-names", true);
                    dialog.close();
                    win.add_toast(i18n("Names are tidied on screen; your files are unchanged"));
                }
            }
        ));
        dialog.present(Some(self));
    }

    // The queue, the saved order and the resume point follow renamed files
    fn follow_renames(&self, ops: &[rename::Rename]) {
        self.save_resume();
        let settings = &self.imp().settings;
        if let Some(player) = self.player() {
            let queue = player.queue();
            let paths: Vec<PathBuf> = (0..queue.n_songs())
                .filter_map(|i| queue.song_at(i))
                .filter_map(|s| s.file().path())
                .map(|p| rename::map_path(ops, &p))
                .collect();
            if !paths.is_empty() {
                utils::store_playlist_paths(&paths);
            }
        }
        let resume = settings.string("resume-uri");
        if let Some(path) = gio::File::for_uri(&resume).path().filter(|_| !resume.is_empty()) {
            let new = gio::File::for_path(rename::map_path(ops, &path));
            let _ = settings.set_string("resume-uri", &new.uri());
        }
    }

    fn apply_rename(&self, plan: &rename::Plan, root: &std::path::Path) {
        let journal = rename::new_journal_path(&Self::rename_journal_dir());
        match rename::apply(plan, root, &journal) {
            Ok(n) => {
                self.follow_renames(&plan.ops);
                self.update_undo_rename();
                self.open_library();
                let toast = adw::Toast::new(&ni18n_f(
                    "Renamed one folder or song",
                    "Renamed {} folders and songs",
                    n as u32,
                    &[&n.to_string()],
                ));
                toast.set_button_label(Some(&i18n("Undo")));
                toast.set_action_name(Some("queue.undo-rename"));
                self.imp().toast_overlay.add_toast(toast);
            }
            Err((0, e)) => {
                let _ = std::fs::remove_file(&journal);
                self.add_toast(i18n_f("Nothing was renamed: {}", &[&e.to_string()]));
            }
            Err((done, e)) => {
                self.follow_renames(&plan.ops[..done]);
                self.update_undo_rename();
                self.open_library();
                self.add_toast(i18n_f(
                    "Renamed {} of {}, then stopped: {}. Undo Rename puts them back.",
                    &[&done.to_string(), &plan.ops.len().to_string(), &e.to_string()],
                ));
            }
        }
    }

    fn undo_rename(&self) {
        if self.imp().loading.get() {
            return;
        }
        let Some(journal) = rename::latest_journal(&Self::rename_journal_dir()) else {
            self.update_undo_rename();
            return;
        };
        let ops = match rename::journal_ops(&journal) {
            Ok(ops) => ops,
            Err(e) => {
                self.add_toast(i18n_f("Unable to read the rename record: {}", &[&e.to_string()]));
                return;
            }
        };
        self.follow_renames(&ops);
        let result = rename::undo(&journal);
        self.update_undo_rename();
        self.open_library();
        match result {
            Ok(n) => self.add_toast(ni18n_f(
                "Put back one name",
                "Put back {} names",
                n as u32,
                &[&n.to_string()],
            )),
            Err(e) => self.add_toast(i18n_f("Undo stopped: {}", &[&e.to_string()])),
        }
    }

    // A folder picked through the file portal comes back as a /run/user/…/doc/… path. Prefer
    // the real host path when this sandbox can read it, so the setting stays valid across
    // restarts and readable in messages; otherwise keep the portal path.
    fn library_folder_path(&self, folder: &gio::File) -> Option<PathBuf> {
        let portal_path = folder.path()?;
        let host_path = folder
            .query_info(
                ATTRIBUTE_HOST_PATH,
                gio::FileQueryInfoFlags::NONE,
                gio::Cancellable::NONE,
            )
            .ok()
            .and_then(|info| info.attribute_as_string(ATTRIBUTE_HOST_PATH))
            .map(|p| PathBuf::from(p.as_str()));
        match host_path {
            Some(host) if gio::File::for_path(&host).query_exists(gio::Cancellable::NONE) => Some(host),
            _ => Some(portal_path),
        }
    }

    // The length of a song is only known once the stream has loaded, so wait for it
    fn seek_when_ready(&self, position: u64) {
        let mut tries = 0;
        glib::timeout_add_local(
            Duration::from_millis(200),
            clone!(
                #[weak(rename_to = win)]
                self,
                #[upgrade_or]
                glib::ControlFlow::Break,
                move || {
                    tries += 1;
                    if let Some(player) = win.player() {
                        let duration = player.state().duration();
                        if duration > 0 {
                            if position > 0 && position < duration {
                                player.seek_position_rel(position as f64 / duration as f64);
                                player.update_position(position, true);
                            }
                            return glib::ControlFlow::Break;
                        }
                    }
                    if tries > 25 {
                        glib::ControlFlow::Break
                    } else {
                        glib::ControlFlow::Continue
                    }
                }
            ),
        );
    }

    // Used at startup: only when nothing is queued or playing
    pub fn open_library_if_autoload(&self) {
        if !self.imp().settings.boolean("library-autoload") || self.library_root().is_none() {
            return;
        }
        if let Some(player) = self.player() {
            if player.queue().is_empty() && !player.state().playing() {
                self.open_library();
            }
        }
    }

    // ---- Reordering the queue -------------------------------------------------------------

    fn queue_index_of(&self, uri: &str) -> Option<u32> {
        let player = self.player()?;
        let queue = player.queue();
        (0..queue.n_songs()).find(|&i| queue.song_at(i).map_or(false, |s| s.uri() == uri))
    }

    fn queue_folders(&self) -> Vec<library::FolderInfo> {
        let Some(player) = self.player() else {
            return Vec::new();
        };
        let queue = player.queue();
        let paths: Vec<PathBuf> = (0..queue.n_songs())
            .filter_map(|i| queue.song_at(i))
            .filter_map(|s| gio::File::for_uri(&s.uri()).path())
            .collect();
        library::group_folders(self.library_root().as_deref(), &paths)
    }

    fn folder_of(folders: &[library::FolderInfo], pos: u32) -> Option<usize> {
        folders
            .iter()
            .position(|f| pos >= f.first_index && pos < f.first_index + f.n_songs)
    }

    fn song_title_at(&self, pos: u32) -> String {
        self.player()
            .and_then(|p| p.queue().song_at(pos))
            .map(|s| s.title())
            .unwrap_or_default()
    }

    // Move a block of songs, remember how to undo it, and keep the order across restarts
    fn move_songs(&self, start: u32, len: u32, to: u32, msg: String) {
        let Some(player) = self.player() else {
            return;
        };
        let queue = player.queue();
        if queue.is_shuffled() {
            self.add_toast(i18n("Turn off shuffle to change the order"));
            return;
        }
        if !queue.move_block(start, len, to) {
            return;
        }

        self.imp().undo_move.set(Some((to, len, start)));
        utils::store_playlist(queue);
        if self.library_root().is_some() && self.folder_sort() != FolderSort::Manual {
            let _ = self.imp().settings.set_string("library-sort", "manual");
        }
        self.update_folder_label();

        let toast = adw::Toast::new(&msg);
        toast.set_button_label(Some(&i18n("Undo")));
        toast.set_action_name(Some("win.undo-move"));
        self.imp().toast_overlay.add_toast(toast);
    }

    fn undo_move(&self) {
        let Some((start, len, to)) = self.imp().undo_move.take() else {
            return;
        };
        if let Some(player) = self.player() {
            let queue = player.queue();
            if queue.move_block(start, len, to) {
                utils::store_playlist(queue);
                self.update_folder_label();
            }
        }
    }

    // Back to folder order by name, with any removed songs back in place
    fn reset_order(&self) {
        let settings = &self.imp().settings;
        if settings.string("library-sort") != "name" {
            // the change signal reorders the queue in place
            let _ = settings.set_string("library-sort", "name");
        }
        if self.library_root().is_some() {
            self.save_resume();
            self.open_library();
        }
    }

    fn folder_sort(&self) -> FolderSort {
        FolderSort::from_setting(&self.imp().settings.string("library-sort"))
    }

    fn played_folders_path() -> PathBuf {
        glib::user_data_dir().join("ryuon").join("recent-folders.tsv")
    }

    fn played_folders() -> std::collections::HashMap<PathBuf, u64> {
        std::fs::read_to_string(Self::played_folders_path())
            .map(|text| library::parse_played(&text))
            .unwrap_or_default()
    }

    // Remember when each folder last played, for the Recently Played sort
    fn record_played(&self) {
        let Some(player) = self.player() else { return };
        let Some(path) = player.state().current_song().and_then(|s| s.file().path()) else {
            return;
        };
        let Some(key) = library::folder_key(self.library_root().as_deref(), &path) else {
            return;
        };
        if self.imp().last_played_folder.borrow().as_ref() == Some(&key) {
            return;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut played = Self::played_folders();
        played.insert(key.clone(), now);
        let file = Self::played_folders_path();
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&file, library::format_played(&played)) {
            debug!("Unable to save recently played folders: {e}");
        }
        self.imp().last_played_folder.replace(Some(key));
    }

    /// Play order for songs given in name order (indices into `paths`); songs outside the
    /// library keep their place at the end
    fn sorted_order(&self, paths: &[PathBuf], sort: FolderSort) -> Vec<u32> {
        let root = self.library_root();
        let folders = library::group_folders(root.as_deref(), paths);
        let played = if sort == FolderSort::Recent { Self::played_folders() } else { Default::default() };
        let stats: Vec<FolderStats> = folders
            .iter()
            .map(|f| FolderStats {
                newest: if sort == FolderSort::Newest {
                    paths[f.first_index as usize..(f.first_index + f.n_songs) as usize]
                        .iter()
                        .filter_map(|p| std::fs::metadata(p).ok()?.modified().ok())
                        .filter_map(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .max()
                        .unwrap_or(0)
                } else {
                    0
                },
                last_played: played.get(&f.key).copied().unwrap_or(0),
            })
            .collect();
        let mut order =
            library::folder_order_to_songs(&folders, &library::sort_folders(&folders, sort, &stats));
        let mut placed = vec![false; paths.len()];
        for &i in &order {
            placed[i as usize] = true;
        }
        order.extend((0..paths.len() as u32).filter(|&i| !placed[i as usize]));
        order
    }

    // The library-sort setting changed: reorder the queue in place, so playback goes on
    fn apply_sort(&self) {
        let sort = self.folder_sort();
        if sort == FolderSort::Manual || self.library_root().is_none() {
            return;
        }
        let Some(player) = self.player() else { return };
        let queue = player.queue();
        if queue.n_songs() < 2 {
            return;
        }
        if queue.is_shuffled() {
            self.add_toast(i18n("The new folder order applies when shuffle is off and the library opens again"));
            return;
        }
        let base = self.library_root().map(gio::File::for_path);
        let mut by_name: Vec<(u32, gio::File)> =
            (0..queue.n_songs()).filter_map(|i| Some((i, queue.song_at(i)?.file()))).collect();
        by_name.sort_by(|a, b| utils::cmp_two_files(base.as_ref(), &a.1, &b.1));
        let paths: Vec<PathBuf> = by_name.iter().filter_map(|(_, f)| f.path()).collect();
        if paths.len() != by_name.len() {
            return;
        }
        let order: Vec<u32> =
            self.sorted_order(&paths, sort).into_iter().map(|i| by_name[i as usize].0).collect();
        if queue.set_order(&order) {
            self.imp().undo_move.set(None);
            utils::store_playlist(queue);
            self.update_folder_label();
        }
    }

    // Folders view: one row per folder in play order, the playing one marked
    fn refresh_folder_list(&self) {
        use adw::prelude::ActionRowExt;
        let view = &self.imp().playlist_view;
        if !view.folders_visible() {
            return;
        }
        let list = view.folder_list();
        list.remove_all();
        let Some(player) = self.player() else { return };
        let queue = player.queue();
        let paths: Vec<PathBuf> = (0..queue.n_songs())
            .filter_map(|i| queue.song_at(i))
            .filter_map(|s| s.file().path())
            .collect();
        let folders = library::group_folders(self.library_root().as_deref(), &paths);
        let current = if queue.is_shuffled() {
            None
        } else {
            queue.current_song_index().and_then(|i| library::position_of(&folders, i)).map(|p| p.folder as usize - 1)
        };

        let mut current_row = None;
        let playing_index = queue.current_song_index();
        for (i, folder) in folders.iter().enumerate() {
            let name = if tidy::enabled() { tidy::name(&folder.name) } else { folder.name.clone() };
            let songs = ni18n_f("One song", "{} songs", folder.n_songs, &[&folder.n_songs.to_string()]);
            let played = current.map_or(false, |c| i < c);
            let is_current = current == Some(i);
            let subtitle = if is_current {
                i18n_f("{} · playing", &[&songs])
            } else if played {
                i18n_f("{} · played", &[&songs])
            } else {
                songs
            };
            let row = adw::ActionRow::builder()
                .title(&name)
                .use_markup(false)
                .subtitle(subtitle)
                .activatable(true)
                .build();
            row.add_css_class("folder-row");

            // Number, then the cover of the folder's first song
            let number = gtk::Label::builder()
                .label((i + 1).to_string())
                .width_chars(3)
                .xalign(1.0)
                .css_classes(["dim-label", "numeric", "caption"])
                .build();
            // A fixed 40 px square: an Image scales the cover down, a Picture would grow to it
            let cover = gtk::Image::builder()
                .pixel_size(40)
                .icon_name("folder-music-symbolic")
                .css_classes(["folder-cover"])
                .build();
            cover.set_overflow(gtk::Overflow::Hidden);
            if let Some(texture) = queue.song_at(folder.first_index).and_then(|s| s.cover_texture()) {
                cover.set_paintable(Some(&texture));
            }
            row.add_prefix(&cover);
            row.add_prefix(&number);
            row.set_title_lines(1);

            if played {
                row.add_css_class("folder-played");
                let tick = gtk::Image::from_icon_name("object-select-symbolic");
                tick.update_property(&[gtk::accessible::Property::Label(&i18n("Played"))]);
                row.add_suffix(&tick);
            }
            if is_current {
                row.add_css_class("folder-current");
                current_row = Some(row.clone());
            }

            let menu = gio::Menu::new();
            let item = |label: String, action: &str| {
                let item = gio::MenuItem::new(Some(&label), None);
                item.set_action_and_target_value(Some(action), Some(&folder.first_index.to_variant()));
                item
            };
            menu.append_item(&item(i18n("Play This Folder Next"), "win.folder-next"));
            menu.append_item(&item(i18n("Move Folder Up"), "win.folder-up"));
            menu.append_item(&item(i18n("Move Folder Down"), "win.folder-down"));
            let button = gtk::MenuButton::builder()
                .icon_name("view-more-symbolic")
                .menu_model(&menu)
                .valign(gtk::Align::Center)
                .tooltip_text(i18n("Folder Options"))
                .css_classes(["flat"])
                .build();
            row.add_suffix(&button);

            let first = folder.first_index;
            row.connect_activated(clone!(
                #[weak(rename_to = win)]
                self,
                move |_| {
                    if let Some(player) = win.player() {
                        player.skip_to(first);
                        player.play();
                    }
                }
            ));
            list.append(&row);

            // The playing folder opens up to show its songs
            if is_current {
                for k in 0..folder.n_songs {
                    let index = folder.first_index + k;
                    let Some(song) = queue.song_at(index) else { continue };
                    let track = adw::ActionRow::builder()
                        .title(song.title())
                        .use_markup(false)
                        .activatable(true)
                        .build();
                    track.add_css_class("folder-track");
                    track.add_prefix(
                        &gtk::Label::builder()
                            .label((k + 1).to_string())
                            .width_chars(2)
                            .xalign(1.0)
                            .css_classes(["dim-label", "numeric", "caption"])
                            .build(),
                    );
                    if playing_index == Some(index) {
                        track.add_css_class("folder-track-now");
                        let spk = gtk::Image::from_icon_name("audio-volume-high-symbolic");
                        spk.update_property(&[gtk::accessible::Property::Label(&i18n("Now playing"))]);
                        track.add_suffix(&spk);
                    }
                    track.connect_activated(clone!(
                        #[weak(rename_to = win)]
                        self,
                        move |_| {
                            if let Some(player) = win.player() {
                                player.skip_to(index);
                                player.play();
                            }
                        }
                    ));
                    list.append(&track);
                }
            }
        }

        // Keep the playing folder in view
        if let Some(row) = current_row {
            glib::idle_add_local_once(move || {
                if let Some(bounds) = row.compute_bounds(&row.parent().unwrap()) {
                    if let Some(scroller) = row.ancestor(gtk::ScrolledWindow::static_type()) {
                        let adj = scroller.downcast::<gtk::ScrolledWindow>().unwrap().vadjustment();
                        let y = bounds.y() as f64;
                        if y < adj.value() || y + bounds.height() as f64 > adj.value() + adj.page_size() {
                            adj.set_value(y - adj.page_size() / 3.0);
                        }
                    }
                }
            });
        }
    }

    fn reorder(&self, op: ReorderOp, pos: u32) {
        let Some(player) = self.player() else {
            return;
        };
        let queue = player.queue();
        let n = queue.n_songs();
        if pos >= n {
            return;
        }
        let title = self.song_title_at(pos);

        match op {
            ReorderOp::PlayNext => match queue.current_song_index() {
                Some(current) if current == pos => {}
                Some(current) => self.move_songs(
                    pos,
                    1,
                    library::play_next_target(pos, current),
                    i18n_f("“{}” plays next", &[&title]),
                ),
                None => self.move_songs(pos, 1, 0, i18n_f("“{}” plays next", &[&title])),
            },
            ReorderOp::Up if pos > 0 => {
                self.move_songs(pos, 1, pos - 1, i18n_f("Moved “{}” up", &[&title]))
            }
            ReorderOp::Down if pos + 1 < n => {
                self.move_songs(pos, 1, pos + 1, i18n_f("Moved “{}” down", &[&title]))
            }
            ReorderOp::FolderNext | ReorderOp::FolderUp | ReorderOp::FolderDown => {
                let folders = self.queue_folders();
                let Some(i) = Self::folder_of(&folders, pos) else {
                    return;
                };
                let f = folders[i].clone();
                match op {
                    ReorderOp::FolderNext => {
                        let Some(c) = queue
                            .current_song_index()
                            .and_then(|cur| Self::folder_of(&folders, cur))
                        else {
                            return;
                        };
                        if c == i {
                            self.add_toast(i18n("This folder is already playing"));
                            return;
                        }
                        let end = folders[c].first_index + folders[c].n_songs;
                        let to = if f.first_index < end { end - f.n_songs } else { end };
                        self.move_songs(f.first_index, f.n_songs, to, i18n_f("“{}” plays next", &[&f.name]));
                    }
                    ReorderOp::FolderUp if i > 0 => {
                        let prev = &folders[i - 1];
                        self.move_songs(f.first_index, f.n_songs, prev.first_index, i18n_f("Moved “{}” up", &[&f.name]));
                    }
                    ReorderOp::FolderDown if i + 1 < folders.len() => {
                        let next = &folders[i + 1];
                        let to = next.first_index - f.n_songs + next.n_songs;
                        self.move_songs(f.first_index, f.n_songs, to, i18n_f("Moved “{}” down", &[&f.name]));
                    }
                    _ => {}
                }
            }
            ReorderOp::Remove => {
                // Lasts until the library is opened again: the file is still in the library,
                // so it comes back after the saved order, like any new song
                if let Some(song) = queue.song_at(pos) {
                    self.remove_song(&song);
                    utils::store_playlist(queue);
                }
            }
            _ => {}
        }
    }

    // Drop the dragged song before or after the song of the row it was dropped on
    fn drop_song(&self, uri: &str, target_uri: &str, before: bool) -> bool {
        if uri == target_uri {
            return false;
        }
        let (Some(from), Some(target)) = (self.queue_index_of(uri), self.queue_index_of(target_uri)) else {
            return false;
        };
        let to = library::drop_target(from, target, before);
        if to == from {
            return false;
        }
        let title = self.song_title_at(from);
        self.move_songs(from, 1, to, i18n_f("Moved “{}”", &[&title]));
        true
    }

    fn show_row_menu(&self, row: &QueueRow, x: f64, y: f64) {
        let Some(song) = row.song() else {
            return;
        };
        let Some(pos) = self.queue_index_of(&song.uri()) else {
            return;
        };

        let item = |label: &str, action: &str| {
            let item = gio::MenuItem::new(Some(label), None);
            item.set_action_and_target_value(Some(action), Some(&pos.to_variant()));
            item
        };
        let info = gio::Menu::new();
        info.append_item(&item(&i18n("Song Info"), "win.song-info"));
        let songs = gio::Menu::new();
        songs.append_item(&item(&i18n("Play Next"), "win.play-next"));
        songs.append_item(&item(&i18n("Move Up"), "win.move-up"));
        songs.append_item(&item(&i18n("Move Down"), "win.move-down"));
        let menu = gio::Menu::new();
        menu.append_section(None, &info);
        menu.append_section(None, &songs);
        if self.queue_folders().len() > 1 {
            let folders = gio::Menu::new();
            folders.append_item(&item(&i18n("Play This Folder Next"), "win.folder-next"));
            folders.append_item(&item(&i18n("Move Folder Up"), "win.folder-up"));
            folders.append_item(&item(&i18n("Move Folder Down"), "win.folder-down"));
            menu.append_section(None, &folders);
        }
        let remove = gio::Menu::new();
        remove.append_item(&item(&i18n("Remove from Playlist"), "win.remove-at"));
        menu.append_section(None, &remove);

        let popover = gtk::PopoverMenu::from_model(Some(&menu));
        popover.set_parent(row);
        popover.set_has_arrow(false);
        popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        popover.connect_closed(|p| {
            let p = p.clone();
            glib::idle_add_local_once(move || p.unparent());
        });
        popover.popup();
    }

    // Everything about one song: what it is, which folder it came from, and the file itself.
    // Reads the audio properties only when the dialog opens.
    fn show_song_info(&self, pos: u32) {
        use adw::prelude::{ActionRowExt, PreferencesGroupExt, PreferencesPageExt};
        let Some(player) = self.player() else { return };
        let queue = player.queue();
        let Some(song) = queue.song_at(pos) else { return };
        let file = song.file();
        let path = file.path();

        let row = |title: String, value: &str| {
            let r = adw::ActionRow::builder()
                .title(title)
                .subtitle(value)
                .use_markup(false)
                .subtitle_selectable(true)
                .build();
            r.add_css_class("property");
            r
        };
        let mmss = |secs: u64| format!("{}:{:02}", secs / 60, secs % 60);

        // What it is
        let what = adw::PreferencesGroup::builder().title(i18n("Song")).build();
        what.add(&row(i18n("Title"), &song.title()));
        what.add(&row(i18n("Artist"), &song.artist()));
        what.add(&row(i18n("Album"), &song.album()));
        what.add(&row(i18n("Length"), &mmss(song.duration())));
        let (raw_title, raw_artist, raw_album) = song.raw_tags();
        let raw = [raw_title, raw_artist, raw_album].iter().flatten().cloned().collect::<Vec<String>>().join(" · ");
        let shown = format!("{} · {} · {}", song.title(), song.artist(), song.album());
        if tidy::enabled() && !raw.is_empty() && raw != shown {
            what.add(&row(i18n("Tags in the File"), &raw));
        }

        // Where it is
        let wher = adw::PreferencesGroup::builder().title(i18n("Where It Is")).build();
        let folders = self.queue_folders();
        if let Some(fi) = Self::folder_of(&folders, pos) {
            let f = &folders[fi];
            let name = if tidy::enabled() { tidy::name(&f.name) } else { f.name.clone() };
            wher.add(&row(i18n("Folder"), &name));
            let place = i18n_f(
                "Folder {} of {} · song {} of {}",
                &[
                    &(fi + 1).to_string(),
                    &folders.len().to_string(),
                    &(pos - f.first_index + 1).to_string(),
                    &f.n_songs.to_string(),
                ],
            );
            wher.add(&row(i18n("Position"), &place));
        }
        if let Some(ref p) = path {
            let fname = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            wher.add(&row(i18n("File Name"), &fname));
            let dir = p.parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
            let location = row(i18n("Location"), &dir);
            let open = gtk::Button::builder()
                .icon_name("folder-open-symbolic")
                .tooltip_text(i18n("Open the Folder"))
                .valign(gtk::Align::Center)
                .css_classes(["flat"])
                .build();
            open.update_property(&[gtk::accessible::Property::Label(&i18n("Open the Folder"))]);
            let target = file.clone();
            open.connect_clicked(clone!(
                #[weak(rename_to = win)]
                self,
                move |_| {
                    let launcher = gtk::FileLauncher::new(Some(&target));
                    launcher.open_containing_folder(Some(win.upcast_ref::<gtk::Window>()), gio::Cancellable::NONE, |_| {});
                }
            ));
            location.add_suffix(&open);
            wher.add(&location);
        } else {
            wher.add(&row(i18n("Location"), &song.uri()));
        }

        // The file
        let filegrp = adw::PreferencesGroup::builder().title(i18n("File")).build();
        if let Some(ref p) = path {
            let ext = p.extension().map(|e| e.to_string_lossy().to_uppercase()).unwrap_or_default();
            let mut fmt = vec![ext];
            if let Ok(tagged) = lofty::read_from_path(p) {
                let props = lofty::prelude::AudioFile::properties(&tagged);
                if let Some(kbps) = props.audio_bitrate() {
                    fmt.push(format!("{kbps} kbps"));
                }
                if let Some(hz) = props.sample_rate() {
                    fmt.push(format!("{:.1} kHz", hz as f64 / 1000.0));
                }
                match props.channels() {
                    Some(1) => fmt.push(i18n("mono")),
                    Some(2) => fmt.push(i18n("stereo")),
                    Some(n) => fmt.push(format!("{n} ch")),
                    None => {}
                }
            }
            filegrp.add(&row(i18n("Format"), &fmt.into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ")));
            if let Ok(meta) = std::fs::metadata(p) {
                filegrp.add(&row(i18n("Size"), &format!("{:.1} MB", meta.len() as f64 / 1_048_576.0)));
            }
        }

        let page = adw::PreferencesPage::new();
        page.add(&what);
        page.add(&wher);
        page.add(&filegrp);
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&adw::HeaderBar::new());
        toolbar.set_content(Some(&page));
        let dialog = adw::Dialog::builder()
            .title(i18n("Song Info"))
            .content_width(440)
            .content_height(600)
            .child(&toolbar)
            .build();
        dialog.present(Some(self));
    }

    // The row that has keyboard focus in the playlist, if any
    fn focused_row(&self) -> Option<QueueRow> {
        let view = self.imp().playlist_view.queue_view();
        let child = view.focus_child()?;
        child
            .downcast_ref::<QueueRow>()
            .cloned()
            .or_else(|| child.first_child().and_then(|c| c.downcast::<QueueRow>().ok()))
    }

    // "Folder 7 of 139 · Song 4 of 10" and "Up next", shown while playing in order
    fn update_folder_label(&self) {
        self.refresh_folder_list();
        let label = &self.imp().folder_label;
        let Some(player) = self.player() else {
            label.set_visible(false);
            return;
        };

        let queue = player.queue();
        let n = queue.n_songs();
        if n < 2 || queue.is_shuffled() {
            label.set_visible(false);
            return;
        }

        let root = self.library_root();
        let paths: Vec<PathBuf> = (0..n)
            .filter_map(|i| queue.song_at(i))
            .filter_map(|s| gio::File::for_uri(&s.uri()).path())
            .collect();
        let folders = library::group_folders(root.as_deref(), &paths);
        let position = queue
            .current_song_index()
            .and_then(|i| library::position_of(&folders, i));

        match position {
            Some(pos) if pos.n_folders >= 2 => {
                let mut text = i18n_f(
                    // Translators: the `{}` placeholders are, in order: folder number, number
                    // of folders, song number in the folder, number of songs in the folder
                    "Folder {} of {} · Song {} of {}",
                    &[
                        &pos.folder.to_string(),
                        &pos.n_folders.to_string(),
                        &pos.song_in_folder.to_string(),
                        &pos.folder_size.to_string(),
                    ],
                );
                if let Some((name, count)) = pos.next {
                    let name = if tidy::enabled() { tidy::name(&name) } else { name };
                    text.push('\n');
                    text.push_str(&ni18n_f(
                        "Up next: {} · one song",
                        "Up next: {} · {} songs",
                        count,
                        &[&name, &count.to_string()],
                    ));
                }
                label.set_label(&text);
                label.set_visible(true);
                if self.imp().announced_folder.replace(pos.folder) != pos.folder {
                    label.announce(&text, gtk::AccessibleAnnouncementPriority::Medium);
                }
            }
            _ => label.set_visible(false),
        }
    }

    fn queue_songs(&self, queue: Vec<gio::File>) {
        if queue.is_empty() {
            self.imp().pending_resume.take();
            self.add_toast(i18n("No available song found"));
            return;
        }

        self.switch_mode(WindowMode::MainView);

        // Disable actions on the queue; loading is "atomic"
        self.action_set_enabled("queue.add-song", false);
        self.action_set_enabled("queue.add-folder", false);
        self.action_set_enabled("queue.clear", false);
        self.action_set_enabled("queue.open-library", false);
        self.action_set_enabled("queue.choose-library", false);
        self.action_set_enabled("queue.rename-files", false);
        self.imp().loading.set(true);

        self.set_playlist_visible(true);
        self.imp().playlist_view.begin_loading();

        // Begin the trace
        let now = Instant::now();

        // Turn the list of files into songs one at a time into the main loop
        let n_files = queue.len() as u32;

        let mut files = queue.into_iter();
        let mut songs = Vec::new();
        let mut cur_file: u32 = 0;
        let mut duplicates: u32 = 0;

        glib::idle_add_local(clone!(
            #[weak(rename_to = win)]
            self,
            #[upgrade_or]
            glib::ControlFlow::Break,
            move || {
                files
                    .next()
                    .map(|f| {
                        win.imp().playlist_view.update_loading(cur_file, n_files);
                        if let Ok(s) = Song::from_uri(f.uri().as_str()) {
                            if let Some(player) = win.player() {
                                let queue = player.queue();
                                // A duplicate of a queued song, or of one earlier in this load
                                if queue.contains(&s) || songs.iter().any(|q: &Song| q.equals(&s)) {
                                    duplicates += 1;
                                } else {
                                    songs.push(s);
                                    cur_file += 1;
                                }
                            }
                        }
                    })
                    .map(|_| glib::ControlFlow::Continue)
                    .unwrap_or_else(|| {
                        debug!(
                            "Total loading time for {} files: {} ms",
                            n_files,
                            now.elapsed().as_millis()
                        );

                        // Re-enable the actions
                        win.action_set_enabled("queue.add-song", true);
                        win.action_set_enabled("queue.add-folder", true);
                        win.action_set_enabled("queue.clear", true);
                        win.action_set_enabled("queue.open-library", true);
                        win.action_set_enabled("queue.choose-library", true);
                        win.action_set_enabled("queue.rename-files", true);
                        win.imp().loading.set(false);

                        // Only this load may resume; never a later one
                        let resume = win.imp().pending_resume.take();

                        if songs.is_empty() {
                            if duplicates == 0 {
                                win.add_toast(i18n("No songs found"));
                            }
                        } else if let Some(player) = win.player() {
                            let queue = player.queue();
                            let was_empty = queue.is_empty();

                            win.imp().playlist_view.end_loading();

                            // Bulk add to avoid hammering the UI with list model updates
                            queue.add_songs(&songs);

                            // Store the current state of the playlist
                            utils::store_playlist(queue);

                            debug!(
                                "Queue was empty: {}, new size: {}",
                                was_empty,
                                queue.n_songs()
                            );
                            if was_empty {
                                player.skip_to(0);
                            }

                            // Opening the library: go back to where we stopped, paused
                            if let Some((uri, position)) = resume {
                                let found = (0..queue.n_songs())
                                    .find(|&i| queue.song_at(i).map_or(false, |s| s.uri() == uri));
                                if let Some(index) = found {
                                    player.skip_to(index);
                                    player.cue();
                                    // Changing song saved it at 0 s; keep the old position
                                    // until the seek has happened
                                    let _ = win
                                        .imp()
                                        .settings
                                        .set_uint64("resume-position", position);
                                    win.seek_when_ready(position);
                                }
                            }

                            // Allow jumping to the song we just added
                            if songs.len() == 1 {
                                // If we added a single song, and the queue was empty, we
                                // dispense with the pleasantries and we start playing
                                // immediately; otherwise, we let the user choose whether
                                // to jump to the newly added song
                                if was_empty {
                                    player.play();
                                } else {
                                    win.add_skip_to_toast(
                                        i18n("Added a new song"),
                                        i18n("Play"),
                                        queue.n_songs() - 1,
                                    );
                                }
                            } else {
                                let msg = ni18n_f(
                                    // Translators: the `{}` must be left unmodified;
                                    // it will be expanded to the number of songs added
                                    // to the playlist
                                    "Added one song",
                                    "Added {} songs",
                                    songs.len() as u32,
                                    &[&songs.len().to_string()],
                                );

                                let msg = if duplicates > 0 {
                                    let skipped = ni18n_f(
                                        "Skipped one duplicate",
                                        "Skipped {} duplicates",
                                        duplicates,
                                        &[&duplicates.to_string()],
                                    );
                                    format!("{} · {}", msg, skipped)
                                } else {
                                    msg
                                };

                                win.add_toast(msg);
                            }
                        }

                        glib::ControlFlow::Break
                    })
            }
        ));
    }

    fn add_files_to_queue(&self, model: &gio::ListModel) {
        let mut queue: Vec<gio::File> = vec![];

        for pos in 0..model.n_items() {
            let file = model.item(pos).unwrap().downcast::<gio::File>().unwrap();

            if let Ok(info) = file.query_info(
                "standard::name,standard::display-name,standard::type,standard::content-type",
                gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                gio::Cancellable::NONE,
            ) {
                match info.file_type() {
                    gio::FileType::Regular => {
                        if let Some(content_type) = info.content_type() {
                            if gio::content_type_is_mime_type(&content_type, "audio/*") {
                                debug!("Adding file '{}' to the queue", file.uri());
                                queue.push(file);
                            }
                        }
                    }
                    gio::FileType::Directory => {
                        debug!("Adding folder '{}' to the queue", file.uri());
                        let files = utils::load_files_from_folder(&file, true);
                        queue.extend(files);
                    }
                    _ => (),
                }
            }
        }

        self.queue_songs(queue);
    }

    // Bind the PlayerState to the UI
    fn bind_state(&self) {
        let imp = self.imp();
        if let Some(player) = self.player() {
            let state = player.state();

            // Use the PlayerState:playing property to control the play/pause button
            self.update_play_button();
            let notify_playing_id = state.connect_notify_local(
                Some("playing"),
                clone!(
                    #[weak(rename_to = win)]
                    self,
                    move |_, _| {
                        win.set_playlist_selection(false);
                        win.update_play_button();
                    }
                ),
            );
            imp.notify_playing_id.replace(Some(notify_playing_id));

            // Update the position labels
            self.update_position_labels();
            let notify_position_id = state.connect_notify_local(
                Some("position"),
                clone!(
                    #[weak(rename_to = win)]
                    self,
                    move |_, _| {
                        win.update_position_labels();
                    }
                ),
            );
            imp.notify_position_id.replace(Some(notify_position_id));

            // Update the UI
            self.update_song();
            let notify_song_id = state.connect_notify_local(
                Some("song"),
                clone!(
                    #[weak(rename_to = win)]
                    self,
                    move |_, _| {
                        win.update_song();
                    }
                ),
            );
            imp.notify_song_id.replace(Some(notify_song_id));

            // Update the cover, if any is available
            self.update_cover();
            let notify_cover_id = state.connect_notify_local(
                Some("cover"),
                clone!(
                    #[weak(rename_to = win)]
                    self,
                    move |_, _| {
                        win.update_cover();
                    }
                ),
            );
            imp.notify_cover_id.replace(Some(notify_cover_id));

            // Bind the song properties to the UI
            state
                .bind_property("title", &imp.song_details.get().title_label(), "label")
                .sync_create()
                .build();
            state
                .bind_property("artist", &imp.song_details.get().artist_label(), "label")
                .sync_create()
                .build();
            state
                .bind_property("album", &imp.song_details.get().album_label(), "label")
                .sync_create()
                .build();
            state
                .bind_property(
                    "volume",
                    &imp.playback_control.get().volume_control(),
                    "volume",
                )
                .sync_create()
                .build();
        }
    }

    fn unbind_state(&self) {
        if let Some(player) = self.player() {
            let state = player.state();

            if let Some(id) = self.imp().notify_playing_id.take() {
                state.disconnect(id);
            }
            if let Some(id) = self.imp().notify_position_id.take() {
                state.disconnect(id);
            }
            if let Some(id) = self.imp().notify_song_id.take() {
                state.disconnect(id);
            }
            if let Some(id) = self.imp().notify_cover_id.take() {
                state.disconnect(id);
            }
        }
    }

    // Bind the Queue to the UI
    fn bind_queue(&self) {
        if let Some(player) = self.player() {
            let queue = player.queue();

            let notify_nsongs_id = queue.connect_notify_local(
                Some("n-songs"),
                clone!(
                    #[weak(rename_to = win)]
                    self,
                    move |queue, _| {
                        debug!("queue.n_songs() = {}", queue.n_songs());
                        if queue.is_empty() {
                            win.set_initial_state();
                            win.reset_queue();
                        } else {
                            win.action_set_enabled("queue.toggle", true);
                            win.action_set_enabled("queue.shuffle", queue.n_songs() > 1);

                            win.action_set_enabled("win.play", true);
                            win.action_set_enabled("win.previous", true);
                            win.action_set_enabled("win.next", queue.n_songs() > 1);
                        }

                        if queue.n_songs() == 1 {
                            if let Some(p) = win.player() {
                                p.skip_next();
                            }
                        }

                        win.update_playlist_time();
                        win.update_folder_label();
                        win.imp().undo_move.set(None);
                    }
                ),
            );
            self.imp().notify_nsongs_id.replace(Some(notify_nsongs_id));

            queue.connect_notify_local(
                Some("repeat-mode"),
                clone!(
                    #[weak(rename_to = win)]
                    self,
                    move |queue, _| {
                        win.imp()
                            .playback_control
                            .set_repeat_mode(queue.repeat_mode());
                    }
                ),
            );

            let notify_current_id = queue.connect_notify_local(
                Some("current"),
                clone!(
                    #[weak(rename_to = win)]
                    self,
                    move |queue, _| {
                        if queue.is_last_song() {
                            match queue.repeat_mode() {
                                RepeatMode::Consecutive => {
                                    win.action_set_enabled("win.next", false);
                                }
                                _ => {
                                    win.action_set_enabled("win.next", true);
                                }
                            }
                        } else {
                            win.action_set_enabled("win.next", true);
                        }
                    }
                ),
            );
            self.imp()
                .notify_current_id
                .replace(Some(notify_current_id));
        }
    }

    fn unbind_queue(&self) {
        if let Some(player) = self.player() {
            let queue = player.queue();
            if let Some(id) = self.imp().notify_nsongs_id.take() {
                queue.disconnect(id);
            }
            if let Some(id) = self.imp().notify_current_id.take() {
                queue.disconnect(id);
            }
        }
    }

    fn connect_signals(&self) {
        self.imp().split_view.connect_notify_local(
            Some("collapsed"),
            clone!(
                #[weak(rename_to = win)]
                self,
                move |split_view, _| {
                    win.set_playlist_visible(split_view.shows_sidebar());
                    win.imp()
                        .playlist_view
                        .back_button()
                        .set_visible(split_view.is_collapsed());
                }
            ),
        );

        self.imp().split_view.connect_notify_local(
            Some("show-sidebar"),
            clone!(
                #[weak(rename_to = win)]
                self,
                move |split_view, _| {
                    win.set_playlist_visible(split_view.shows_sidebar());
                }
            ),
        );

        self.imp().waveform_view.connect_closure(
            "position-changed",
            false,
            closure_local!(
                #[watch(rename_to = win)]
                self,
                move |_wv: WaveformView, position: f64| {
                    debug!("New position: {}", position);
                    if let Some(player) = win.player() {
                        let state = player.state();
                        if state.current_song().is_some() {
                            player.seek_position_rel(position);
                            player.play();
                        }
                    }
                }
            ),
        );

        self.imp()
            .playback_control
            .volume_control()
            .connect_closure(
                "volume-changed",
                false,
                closure_local!(
                    #[watch(rename_to = win)]
                    self,
                    move |_vc: VolumeControl, volume: f64| {
                        debug!("Volume changed: {}", volume);
                        if let Some(p) = win.player() {
                            p.set_volume(volume);
                        }
                    }
                ),
            );

        self.imp()
            .playlist_view
            .queue_select_all_button()
            .connect_clicked(clone!(
                #[weak(rename_to = win)]
                self,
                move |_| {
                    if win.playlist_search() {
                        if let Some(ref model) = *win.imp().playlist_filtermodel.borrow() {
                            for idx in 0..model.n_items() {
                                let item = model.item(idx).unwrap();
                                let song = item.downcast_ref::<Song>().unwrap();
                                song.set_selected(true);
                            }
                        }
                    } else if let Some(player) = win.player() {
                        let queue = player.queue();
                        for idx in 0..queue.n_songs() {
                            let song = queue.song_at(idx).unwrap();
                            song.set_selected(true);
                        }
                    }
                }
            ));

        self.imp()
            .playlist_view
            .queue_remove_button()
            .connect_clicked(clone!(
                #[weak(rename_to = win)]
                self,
                move |_| {
                    if let Some(player) = win.player() {
                        let queue = player.queue();
                        let mut remove_songs: Vec<Song> = Vec::new();
                        // Collect all songs to be removed first, since we can't
                        // remove objects from the model while we're iterating it
                        for idx in 0..queue.n_songs() {
                            let song = queue.song_at(idx).unwrap();
                            if song.selected() {
                                remove_songs.push(song);
                            }
                        }

                        for song in remove_songs {
                            win.remove_song(&song);
                        }

                        // Store the current state of the playlist
                        utils::store_playlist(queue);
                    }
                }
            ));

        self.imp()
            .playlist_view
            .playlist_searchbar()
            .connect_notify_local(
                Some("search-mode-enabled"),
                clone!(
                    #[weak(rename_to = win)]
                    self,
                    move |searchbar, _| {
                        win.set_playlist_search(searchbar.is_search_mode());
                    }
                ),
            );

        self.imp().settings.connect_changed(
            Some("enable-recoloring"),
            clone!(
                #[weak(rename_to = this)]
                self,
                move |settings, _| {
                    debug!(
                        "GSettings:enable-recoloring: {}",
                        settings.boolean("enable-recoloring")
                    );
                    if let Some(player) = this.player() {
                        let state = player.state();
                        this.update_style(state.current_song().as_ref());
                    }
                }
            ),
        );
        let _dummy = self.imp().settings.boolean("enable-recoloring");

        self.connect_close_request(move |window| {
            debug!("Saving window state");
            let width = window.default_size().0;
            let height = window.default_size().1;

            let settings = utils::settings_manager();
            settings
                .set_int("window-width", width)
                .expect("Unable to store window-width");
            settings
                .set_int("window-height", height)
                .expect("Unable to stop window-height");

            window.save_resume();

            window.unbind_queue();
            window.unbind_state();
            window.unbind_waveform();

            glib::Propagation::Proceed
        });

        self.imp()
            .playlist_view
            .playlist_searchbar()
            .set_key_capture_widget(Some(self.upcast_ref::<gtk::Widget>()));
    }

    // The initial state of the playback actions
    fn set_initial_state(&self) {
        if let Some(player) = self.player() {
            let queue = player.queue();
            self.action_set_enabled("win.play", !queue.is_empty());
            self.action_set_enabled("win.previous", !queue.is_empty());
            self.action_set_enabled("win.next", !queue.is_last_song());

            self.action_set_enabled("queue.toggle", !queue.is_empty());
            self.action_set_enabled("queue.shuffle", queue.n_songs() > 1);
            self.action_set_enabled("win.replaygain", player.replaygain_available());

            let replaygain = self.imp().settings.enum_("replay-gain").into();
            self.set_replaygain(replaygain);

            // Manually set player state, because set_replaygain
            // only updates player state when the value changes.
            player.set_replaygain(replaygain);

            self.imp()
                .playback_control
                .set_repeat_mode(queue.repeat_mode());
            self.set_playlist_shuffled(queue.is_shuffled());

            // Manually update the icon on the initial empty state
            // to avoid generating the UI definition file at build
            // time
            self.imp().status_page.set_icon_name(Some(APPLICATION_ID));

            // The first choice that is available is the suggested one
            let has_library = self.library_root().is_some();
            let has_cache = utils::has_cached_playlist();
            let imp = self.imp();
            imp.open_library_button.set_visible(has_library);
            imp.restore_playlist_button.set_visible(has_cache);
            set_suggested(&imp.open_library_button, has_library);
            set_suggested(&imp.restore_playlist_button, !has_library && has_cache);
            set_suggested(&imp.add_folder_button, !has_library && !has_cache);

            let state = player.state();
            if state.playing() || !queue.is_empty() {
                self.switch_mode(WindowMode::MainView);
            } else {
                self.switch_mode(WindowMode::InitialView);
            }
        }
    }

    fn setup_playlist(&self) {
        let imp = self.imp();


        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(clone!(
            #[weak(rename_to = win)]
            self,
            move |_, item| {
                let row = QueueRow::default();
                let list_item = item.downcast_ref::<gtk::ListItem>().unwrap();
                list_item.set_child(Some(&row));

                row.connect_notify_local(
                    Some("selected"),
                    clone!(
                        #[weak]
                        win,
                        move |_, _| {
                            win.update_selected_count();
                        }
                    ),
                );

                win.bind_property("playlist-selection", &row, "selection-mode")
                    .sync_create()
                    .build();

                // Row menu: right click and long press
                let click = gtk::GestureClick::new();
                click.set_button(gdk::BUTTON_SECONDARY);
                click.connect_pressed(clone!(
                    #[weak]
                    win,
                    #[weak]
                    row,
                    move |gesture, _, x, y| {
                        gesture.set_state(gtk::EventSequenceState::Claimed);
                        win.show_row_menu(&row, x, y);
                    }
                ));
                row.add_controller(click);
                let long_press = gtk::GestureLongPress::new();
                long_press.connect_pressed(clone!(
                    #[weak]
                    win,
                    #[weak]
                    row,
                    move |_, x, y| {
                        win.show_row_menu(&row, x, y);
                    }
                ));
                row.add_controller(long_press);

                // Drag a song onto another row to move it there
                let drag = gtk::DragSource::new();
                drag.set_actions(gdk::DragAction::MOVE);
                drag.connect_prepare(clone!(
                    #[weak]
                    win,
                    #[weak]
                    row,
                    #[upgrade_or]
                    None,
                    move |_, _, _| {
                        if win.playlist_selection() || win.playlist_shuffled() {
                            return None;
                        }
                        row.song()
                            .map(|s| gdk::ContentProvider::for_value(&s.uri().to_value()))
                    }
                ));
                drag.connect_drag_begin(clone!(
                    #[weak]
                    row,
                    move |source, _| {
                        let icon = gtk::WidgetPaintable::new(Some(&row));
                        source.set_icon(Some(&icon), 0, 0);
                    }
                ));
                row.add_controller(drag);
                let drop = gtk::DropTarget::new(String::static_type(), gdk::DragAction::MOVE);
                drop.connect_drop(clone!(
                    #[weak]
                    win,
                    #[weak]
                    row,
                    #[upgrade_or]
                    false,
                    move |_, value, _, y| {
                        let (Ok(uri), Some(target)) = (value.get::<String>(), row.song()) else {
                            return false;
                        };
                        let before = y < row.height() as f64 / 2.0;
                        win.drop_song(&uri, &target.uri(), before)
                    }
                ));
                row.add_controller(drop);

                list_item
                    .bind_property("item", &row, "song")
                    .sync_create()
                    .build();

                list_item
                    .property_expression("item")
                    .chain_property::<Song>("artist")
                    .bind(&row, "song-artist", gtk::Widget::NONE);
                list_item
                    .property_expression("item")
                    .chain_property::<Song>("title")
                    .bind(&row, "song-title", gtk::Widget::NONE);
                list_item
                    .property_expression("item")
                    .chain_property::<Song>("cover")
                    .bind(&row, "song-cover", gtk::Widget::NONE);
                list_item
                    .property_expression("item")
                    .chain_property::<Song>("playing")
                    .bind(&row, "playing", gtk::Widget::NONE);
                list_item
                    .property_expression("item")
                    .chain_property::<Song>("selected")
                    .bind(&row, "selected", gtk::Widget::NONE);
            }
        ));
        imp.playlist_view
            .queue_view()
            .set_factory(Some(&factory.upcast::<gtk::ListItemFactory>()));

        let keys = gtk::ShortcutController::new();
        for (trigger, op) in [
            ("Menu|<Shift>F10", None),
            ("<Alt>Up", Some(ReorderOp::Up)),
            ("<Alt>Down", Some(ReorderOp::Down)),
            ("<Alt><Shift>Up", Some(ReorderOp::FolderUp)),
            ("<Alt><Shift>Down", Some(ReorderOp::FolderDown)),
        ] {
            let action = gtk::CallbackAction::new(clone!(
                #[weak(rename_to = win)]
                self,
                #[upgrade_or]
                glib::Propagation::Proceed,
                move |_, _| {
                    let Some(row) = win.focused_row() else {
                        return glib::Propagation::Proceed;
                    };
                    match op {
                        None => win.show_row_menu(&row, row.width() as f64 / 2.0, row.height() as f64 / 2.0),
                        Some(op) => {
                            if let Some(pos) = row.song().and_then(|s| win.queue_index_of(&s.uri())) {
                                win.reorder(op, pos);
                            }
                        }
                    }
                    glib::Propagation::Stop
                }
            ));
            keys.add_shortcut(gtk::Shortcut::new(gtk::ShortcutTrigger::parse_string(trigger), Some(action)));
        }
        imp.playlist_view.queue_view().add_controller(keys);

        if let Some(player) = self.player() {
            let queue = player.queue();

            let filter = FuzzyFilter::new();
            let filter_model =
                gtk::FilterListModel::new(Some(queue.model().clone()), Some(filter.clone()));
            let sorter = FuzzySorter::new();
            let sorter_model = gtk::SortListModel::new(Some(filter_model), Some(sorter.clone()));
            let selection = gtk::NoSelection::new(Some(sorter_model.clone()));
            imp.playlist_view
                .queue_view()
                .set_model(Some(selection.upcast_ref::<gtk::SelectionModel>()));
            imp.playlist_view.queue_view().connect_activate(clone!(
                #[weak(rename_to = win)]
                self,
                #[weak]
                selection,
                move |_, pos| {
                    let song = selection
                        .upcast::<gio::ListModel>()
                        .item(pos)
                        .unwrap()
                        .downcast::<Song>()
                        .unwrap();

                    if let Some(player) = win.player() {
                        let queue = player.queue();

                        let mut real_pos = None;
                        for i in 0..queue.model().n_items() {
                            if let Some(item) = queue.model().item(i) {
                                let s = item.downcast::<Song>().unwrap();
                                if s.equals(&song) {
                                    real_pos = Some(i);
                                    break;
                                }
                            }
                        }

                        if let Some(real_pos) = real_pos {
                            if win.playlist_selection() {
                                queue.select_song_at(real_pos);
                            } else if queue.current_song_index() != Some(real_pos) {
                                player.skip_to(real_pos);
                                player.play();
                            } else if !player.state().playing() {
                                player.play();
                            }
                        }
                    }
                }
            ));

            imp.playlist_filtermodel
                .replace(Some(sorter_model.upcast::<gio::ListModel>()));

            imp.playlist_view
                .playlist_searchentry()
                .bind_property("text", &filter, "search")
                .sync_create()
                .build();
            imp.playlist_view
                .playlist_searchentry()
                .bind_property("text", &sorter, "search")
                .sync_create()
                .build();
            imp.playlist_view
                .playlist_searchentry()
                .connect_search_changed(clone!(
                    #[weak(rename_to = win)]
                    self,
                    move |_| {
                        if let Some(adjustment) = win.imp().playlist_view.queue_view().vadjustment()
                        {
                            adjustment.set_value(0.0);
                        }
                    }
                ));
        }
    }

    fn setup_drop_target(&self) {
        let drop_target = gtk::DropTarget::builder()
            .name("file-drop-target")
            .actions(gdk::DragAction::COPY)
            .formats(&gdk::ContentFormats::for_type(gdk::FileList::static_type()))
            .build();

        drop_target.connect_drop(clone!(
            #[weak(rename_to = win)]
            self,
            #[upgrade_or]
            false,
            move |_, value, _, _| {
                if let Ok(file_list) = value.get::<gdk::FileList>() {
                    if file_list.files().is_empty() {
                        win.add_toast(i18n("Unable to access dropped files"));
                        return false;
                    }

                    let model = gio::ListStore::new::<gio::File>();
                    for f in file_list.files() {
                        model.append(&f);
                    }
                    win.add_files_to_queue(model.upcast_ref::<gio::ListModel>());
                    return true;
                }

                false
            }
        ));

        self.imp().drag_overlay.set_drop_target(&drop_target);
    }

    fn update_play_button(&self) {
        if let Some(player) = self.player() {
            let state = player.state();
            let play_button = self.imp().playback_control.play_button();
            if state.playing() {
                play_button.set_icon_name("media-playback-pause-symbolic");
            } else {
                play_button.set_icon_name("media-playback-start-symbolic");
            }
        }
    }

    fn update_position_labels(&self) {
        if let Some(player) = self.player() {
            let state = player.state();
            if state.current_song().is_some() {
                let elapsed = state.position();
                let duration = state.duration();
                let remaining = duration.checked_sub(elapsed).unwrap_or_default();
                self.set_song_time(Some(elapsed), Some(remaining));

                let position = state.position() as f64 / state.duration() as f64;
                self.set_song_position(position);
            } else {
                self.set_song_time(None, None);
                self.set_song_position(0.0);
            }
        }
    }

    fn update_song(&self) {
        if let Some(player) = self.player() {
            let state = player.state();
            self.scroll_playlist_to_song();
            self.update_playlist_time();
            self.update_title(state.current_song().as_ref());
            self.update_style(state.current_song().as_ref());
            self.update_folder_label();
            self.record_played();
            self.save_resume();
        }
    }

    fn update_cover(&self) {
        if let Some(player) = self.player() {
            let state = player.state();
            let song_cover = self.imp().song_cover.get();
            if let Some(cover) = state.cover() {
                song_cover.album_image().set_cover(Some(&cover));
                song_cover.show_cover_image(true);
            } else {
                song_cover.album_image().set_cover(None);
                song_cover.show_cover_image(false);
            }
        }
    }

    fn update_playlist_time(&self) {
        if let Some(player) = self.player() {
            let queue = player.queue();
            let n_songs = queue.n_songs();
            if let Some(current) = queue.current_song_index() {
                let mut remaining_time = 0;
                for pos in 0..n_songs {
                    let song = queue.song_at(pos).unwrap();
                    if pos >= current {
                        remaining_time += song.duration();
                    }
                }

                let remaining_min = ((remaining_time - (remaining_time % 60)) / 60) as u32;
                let remaining_hrs = (remaining_min - (remaining_min % 60)) / 60;

                let remaining_str = if remaining_hrs > 0 {
                    // Translators: the `{}` must be left unmodified, and
                    // it will be replaced by the number of minutes remaining
                    // in the string "N hours M minutes remaining"
                    let minutes = ni18n_f(
                        "{} minute",
                        "{} minutes",
                        remaining_min % 60,
                        &[&(remaining_min % 60).to_string()],
                    );

                    // Translators: `{hours}` and `{minutes}` must be left
                    // unmodified, and they will be replaced by number of
                    // hours and by the translated number of minutes
                    // remaining, respectively
                    ni18n_k(
                        "{hours} hour {minutes} remaining",
                        "{hours} hours {minutes} remaining",
                        remaining_hrs,
                        &[("hours", &remaining_hrs.to_string()), ("minutes", &minutes)],
                    )
                } else {
                    ni18n_f(
                        // Translators: the '{}' must be left unmodified, and
                        // it will be replaced by the number of minutes remaining
                        // in the playlist
                        "{} minute remaining",
                        "{} minutes remaining",
                        remaining_min,
                        &[&remaining_min.to_string()],
                    )
                };

                self.imp()
                    .playlist_view
                    .queue_length_label()
                    .set_label(&remaining_str);
                self.imp()
                    .playlist_view
                    .queue_length_label()
                    .set_visible(true);
            } else {
                self.imp()
                    .playlist_view
                    .queue_length_label()
                    .set_visible(false);
            }
        }
    }

    fn scroll_playlist_to_song(&self) {
        let queue_view = self.imp().playlist_view.queue_view();
        if let Some(player) = self.player() {
            if let Some(current_idx) = player.queue().current_song_index() {
                debug!("Scrolling playlist to {}", current_idx);
                queue_view
                    .upcast_ref::<gtk::Widget>()
                    .activate_action("list.scroll-to-item", Some(&current_idx.to_variant()))
                    .expect("Failed to activate action");
            }
        }
    }

    fn setup_provider(&self) {
        let imp = self.imp();
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(&display, &imp.provider, 400);
        }
    }

    fn update_style(&self, song: Option<&Song>) {
        let imp = self.imp();

        if !imp.settings.boolean("enable-recoloring") {
            imp.provider.load_from_string("");
            imp.main_stack.remove_css_class("main-window");
            return;
        }

        if let Some(song) = song {
            if let Some(bg_colors) = song.cover_palette() {
                let mut css = String::new();

                css.push_str(":root {");

                let n_colors = bg_colors.len();
                for (i, color) in bg_colors.iter().enumerate().take(n_colors) {
                    let s = format!("--background-color-{}: {};", i, color);
                    css.push_str(&s);
                }

                for i in n_colors + 1 - 1..5 {
                    css.push_str(&format!(
                        "--background-color-{}: var(--window-bg-color);",
                        i
                    ));
                }

                css.push_str("}");

                imp.provider.load_from_string(&css);
                if !imp.main_stack.has_css_class("main-window") {
                    imp.main_stack.add_css_class("main-window");
                }

                self.action_set_enabled("win.enable-recoloring", true);

                return;
            }
        }

        imp.provider.load_from_string("");
        imp.main_stack.remove_css_class("main-window");
        self.action_set_enabled("win.enable-recoloring", false);
    }

    /// Show names again after the tidy-names setting changed
    pub fn refresh_names(&self) {
        if let Some(player) = self.player() {
            let queue = player.queue();
            for song in (0..queue.n_songs()).filter_map(|i| queue.song_at(i)) {
                song.notify_names();
            }
            let state = player.state();
            state.notify_names();
            self.update_title(state.current_song().as_ref());
        }
        self.update_folder_label();
    }

    fn update_title(&self, song: Option<&Song>) {
        if let Some(song) = song {
            self.set_title(Some(&format!("{} - {}", song.artist(), song.title())));
        } else {
            self.set_title(Some("Ryuon"));
        }
    }

    fn update_selected_count(&self) {
        if let Some(player) = self.player() {
            let queue = player.queue();
            let n_selected = queue.n_selected_songs();

            let selected_str = if n_selected == 0 {
                i18n("No song selected")
            } else {
                ni18n_f(
                    // Translators: The '{}' must be left unmodified, and
                    // it is expanded to the number of songs selected
                    "{} song selected",
                    "{} songs selected",
                    n_selected,
                    &[&n_selected.to_string()],
                )
            };

            self.imp()
                .playlist_view
                .queue_selected_label()
                .set_label(&selected_str);
        }
    }

    pub fn open_files(&self, files: &[gio::File]) {
        if files.is_empty() {
            self.add_toast(i18n("Unable to access files"));
            return;
        }

        let model = gio::ListStore::new::<gio::File>();
        for f in files {
            model.append(f);
        }
        self.add_files_to_queue(model.upcast_ref::<gio::ListModel>());
    }

    pub fn remove_song(&self, song: &Song) {
        if let Some(p) = self.player() {
            p.remove_song(song);
        }
        self.update_selected_count();
        self.update_playlist_time();
    }

    pub fn add_toast(&self, msg: String) {
        let toast = adw::Toast::new(&msg);
        self.imp().toast_overlay.add_toast(toast);
    }

    pub fn add_skip_to_toast(&self, msg: String, button: String, pos: u32) {
        let toast = adw::Toast::new(&msg);
        toast.set_button_label(Some(&button));
        toast.set_action_name(Some("win.skip-to"));
        toast.set_action_target_value(Some(&pos.to_variant()));
        self.imp().toast_overlay.add_toast(toast);
    }

    fn copy_song(&self) {
        if let Some(player) = self.player() {
            let state = player.state();
            if let Some(song) = state.current_song() {
                let s = i18n_k(
                    // Translators: `{title}` and `{artist}` must be left
                    // untranslated; they will expand to the title and
                    // artist of the currently playing song, respectively
                    "Currently playing “{title}” by “{artist}”",
                    &[("title", &song.title()), ("artist", &song.artist())],
                );
                self.clipboard().set_text(&s);
            }
        }
    }

    async fn open_song_directory(&self) {
        if let Some(player) = self.player() {
            let state = player.state();

            if let Some(song) = state.current_song() {
                let file = song.file();

                match file
                    .query_info_future(
                        &ATTRIBUTE_HOST_PATH,
                        gio::FileQueryInfoFlags::NONE,
                        glib::Priority::DEFAULT,
                    )
                    .await
                {
                    Err(err) => debug!("Unable to get host-path attribute: {err}"),
                    Ok(info) => {
                        let host_path = info
                            .attribute_as_string(&ATTRIBUTE_HOST_PATH)
                            .and_then(|x| PathBuf::from_str(x.as_str()).ok());

                        if let Some(path) = host_path.or_else(|| file.path()) {
                            let launcher = gtk::FileLauncher::new(Some(&gio::File::for_path(path)));
                            match launcher
                                .open_containing_folder_future(Some(
                                    self.upcast_ref::<gtk::Window>(),
                                ))
                                .await
                            {
                                Err(_e) => self.add_toast(i18n("Unable to open folder")),
                                Ok(()) => {}
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn switch_mode(&self, mode: WindowMode) {
        let stack = self.imp().main_stack.get();
        match mode {
            WindowMode::InitialView => {
                stack.set_visible_child_name("initial-view");
                self.set_default_widget(Some(&self.imp().add_folder_button.get()));
            }
            WindowMode::MainView => {
                stack.set_visible_child_name("main-view");
                self.set_default_widget(Some(&self.imp().playback_control.play_button()));
            }
        };
    }

    pub fn set_replaygain(&self, replaygain: ReplayGainMode) {
        let imp = self.imp();

        if replaygain != imp.replaygain_mode.replace(replaygain) {
            if let Some(p) = self.player() {
                p.set_replaygain(replaygain);
            }
            self.imp()
                .settings
                .set_enum("replay-gain", replaygain.into())
                .expect("Unable to store setting");

            self.notify("replaygain-mode");
        }
    }

    pub fn replaygain(&self) -> ReplayGainMode {
        self.imp().replaygain_mode.get()
    }

    pub fn set_song_time(&self, elapsed: Option<u64>, remaining: Option<u64>) {
        if let Some(elapsed) = elapsed {
            self.imp()
                .elapsed_label
                .set_text(&utils::format_time(elapsed as i64));
        } else {
            self.imp().elapsed_label.set_text("0:00");
        }

        if let Some(remaining) = remaining {
            self.imp()
                .remaining_label
                .set_text(&utils::format_remaining_time(remaining as i64));
        } else {
            self.imp().remaining_label.set_text("0:00");
        }
    }

    pub fn set_song_position(&self, position: f64) {
        self.imp().waveform_view.set_position(position);
    }
}
