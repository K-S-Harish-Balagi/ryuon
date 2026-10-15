// SPDX-FileCopyrightText: 2022  Emmanuele Bassi
// SPDX-License-Identifier: GPL-3.0-or-later

use adw::subclass::prelude::*;
use gtk::{gio, glib, prelude::*, CompositeTemplate};

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/k_s_harish_balagi/Ryuon/playlist-view.ui")]
    pub struct PlaylistView {
        #[template_child]
        pub back_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub queue_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub queue_length_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub queue_actionbar: TemplateChild<gtk::ActionBar>,
        #[template_child]
        pub queue_select_all_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub queue_remove_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub queue_selected_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub playlist_progress: TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub playlist_searchbar: TemplateChild<gtk::SearchBar>,
        #[template_child]
        pub playlist_searchentry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub songs_tab: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub folders_tab: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub playlist_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub folder_list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub folder_scroller: TemplateChild<gtk::ScrolledWindow>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlaylistView {
        const NAME: &'static str = "RyuonPlaylistView";
        type Type = super::PlaylistView;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);

            klass.set_layout_manager_type::<gtk::BinLayout>();
            klass.set_css_name("playlistview");
            klass.set_accessible_role(gtk::AccessibleRole::Group);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PlaylistView {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }

        fn constructed(&self) {
            self.parent_constructed();

            self.obj().setup_searchbar();

            // Songs | Folders tabs drive the window's show-folders toggle
            let view = self.obj().clone();
            self.folders_tab.connect_toggled(move |tab| {
                if tab.is_active() != view.folders_visible() {
                    let _ = WidgetExt::activate_action(&view, "win.show-folders", None);
                }
            });
        }
    }

    impl WidgetImpl for PlaylistView {}
}

glib::wrapper! {
    pub struct PlaylistView(ObjectSubclass<imp::PlaylistView>)
        @extends gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible,
                    gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for PlaylistView {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl PlaylistView {
    pub fn new() -> Self {
        Self::default()
    }

    fn setup_searchbar(&self) {
        let entry = self.imp().playlist_searchentry.get();
        self.imp().playlist_searchbar.connect_entry(&entry);
    }

    pub fn back_button(&self) -> gtk::Button {
        self.imp().back_button.get()
    }

    pub fn queue_actionbar(&self) -> gtk::ActionBar {
        self.imp().queue_actionbar.get()
    }

    pub fn queue_remove_button(&self) -> gtk::Button {
        self.imp().queue_remove_button.get()
    }

    pub fn queue_select_all_button(&self) -> gtk::Button {
        self.imp().queue_select_all_button.get()
    }

    pub fn queue_selected_label(&self) -> gtk::Label {
        self.imp().queue_selected_label.get()
    }

    pub fn queue_view(&self) -> gtk::ListView {
        self.imp().queue_view.get()
    }

    pub fn queue_length_label(&self) -> gtk::Label {
        self.imp().queue_length_label.get()
    }

    pub fn playlist_searchbar(&self) -> gtk::SearchBar {
        self.imp().playlist_searchbar.get()
    }

    pub fn playlist_searchentry(&self) -> gtk::SearchEntry {
        self.imp().playlist_searchentry.get()
    }

    /// Keep the Songs | Folders tabs in step when the view changes from elsewhere
    pub fn sync_tabs(&self, folders: bool) {
        let imp = self.imp();
        if folders {
            imp.folders_tab.set_active(true);
        } else {
            imp.songs_tab.set_active(true);
        }
    }

    pub fn folder_list(&self) -> gtk::ListBox {
        self.imp().folder_list.get()
    }

    pub fn folder_scroller(&self) -> gtk::ScrolledWindow {
        self.imp().folder_scroller.get()
    }

    /// Songs or Folders in the sidebar
    pub fn set_folders_visible(&self, folders: bool) {
        self.imp()
            .playlist_stack
            .set_visible_child_name(if folders { "folders" } else { "songs" });
    }

    pub fn folders_visible(&self) -> bool {
        self.imp().playlist_stack.visible_child_name().as_deref() == Some("folders")
    }

    pub fn begin_loading(&self) {
        self.imp().playlist_progress.set_fraction(0.0);
        self.imp().playlist_progress.set_visible(true);
    }

    pub fn end_loading(&self) {
        self.imp().playlist_progress.set_visible(false);
    }

    pub fn update_loading(&self, cur: u32, max: u32) {
        let step = cur as f64 / max as f64;
        self.imp().playlist_progress.set_fraction(step);
    }

    pub fn set_search(&self, search: bool) {
        self.imp().playlist_searchbar.set_search_mode(search);
    }
}
