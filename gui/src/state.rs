//! Shared application state, the main window shell and the sidebar.

use crate::detail;
use crate::widgets;
use gtk4 as gtk;
use libadwaita as adw;
use adw::prelude::*;
use nspawn_studio_core::command::{self, MachineState};
use nspawn_studio_core::model::ContainerConfig;
use nspawn_studio_core::store::Store;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// A sidebar entry we keep around so the status can be refreshed in place.
pub struct SidebarEntry {
    pub name: String,
    pub dot: gtk::Label,
    pub subtitle: gtk::Label,
}

/// Everything the UI callbacks need.
pub struct App {
    pub store: Store,
    pub window: adw::ApplicationWindow,
    pub toast: adw::ToastOverlay,
    pub list: gtk::ListBox,
    pub stack: gtk::Stack,
    pub detail_slot: gtk::Box,
    pub configs: RefCell<Vec<ContainerConfig>>,
    pub selected: RefCell<Option<String>>,
    pub entries: RefCell<Vec<SidebarEntry>>,
    pub suppress: Cell<bool>,
    detail_refresher: RefCell<Option<Box<dyn Fn()>>>,
}

impl App {
    /// Build the whole window and return the shared state.
    pub fn build(application: &adw::Application) -> Rc<Self> {
        let store = Store::new();
        if let Err(e) = store.ensure() {
            eprintln!("nspawn-studio: could not create data directories: {}", e);
        }

        let window = adw::ApplicationWindow::builder()
            .application(application)
            .title("Nspawn Studio")
            .default_width(1150)
            .default_height(780)
            .build();

        let header = adw::HeaderBar::new();
        let add_button = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text("Create a new container")
            .build();
        header.pack_start(&add_button);
        let refresh_button = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Reload containers")
            .build();
        header.pack_end(&refresh_button);

        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .build();
        list.add_css_class("navigation-sidebar");
        list.set_vexpand(true);

        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&list)
            .build();

        let sidebar_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        sidebar_box.append(&scrolled);

        let empty = adw::StatusPage::builder()
            .icon_name("utilities-terminal-symbolic")
            .title("No container selected")
            .description("Pick a container on the left, or create a new systemd-nspawn container.")
            .build();
        let empty_button = gtk::Button::builder()
            .label("Create a container")
            .halign(gtk::Align::Center)
            .css_classes(["pill", "suggested-action"])
            .build();
        empty.set_child(Some(&empty_button));

        let detail_slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
        detail_slot.set_vexpand(true);

        let stack = gtk::Stack::builder().vexpand(true).build();
        stack.add_named(&empty, Some("empty"));
        stack.add_named(&detail_slot, Some("detail"));
        stack.set_visible_child_name("empty");

        let sidebar_page = adw::NavigationPage::new(&sidebar_box, "Containers");
        let content_page = adw::NavigationPage::new(&stack, "Container");
        let split = adw::NavigationSplitView::new();
        split.set_sidebar(Some(&sidebar_page));
        split.set_content(Some(&content_page));

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&split));

        let toast = adw::ToastOverlay::new();
        toast.set_child(Some(&toolbar));
        window.set_content(Some(&toast));

        let app = Rc::new(App {
            store,
            window,
            toast,
            list,
            stack,
            detail_slot,
            configs: RefCell::new(Vec::new()),
            selected: RefCell::new(None),
            entries: RefCell::new(Vec::new()),
            suppress: Cell::new(false),
            detail_refresher: RefCell::new(None),
        });

        {
            let app = app.clone();
            add_button.connect_clicked(move |_| crate::create::present(&app));
        }
        {
            let app = app.clone();
            empty_button.connect_clicked(move |_| crate::create::present(&app));
        }
        {
            let app = app.clone();
            refresh_button.connect_clicked(move |_| app.reload());
        }
        {
            let list = app.list.clone();
            let app = app.clone();
            list.connect_row_selected(move |_, row| {
                if app.suppress.get() {
                    return;
                }
                if let Some(row) = row {
                    let name = row.widget_name().to_string();
                    if !name.is_empty() {
                        app.select(&name);
                    }
                }
            });
        }
        {
            let app = app.clone();
            adw::glib::timeout_add_seconds_local(3, move || {
                app.refresh_states();
                adw::glib::ControlFlow::Continue
            });
        }

        app.reload();
        app.window.present();

        // Make sure the status column reflects reality right away.
        app.refresh_states();
        app
    }

    /// Show a transient message at the bottom of the window.
    pub fn notify(&self, message: &str) {
        let toast = adw::Toast::new(message);
        toast.set_timeout(3);
        self.toast.add_toast(toast);
    }

    /// Reload all configurations from disk.
    pub fn reload(self: &Rc<Self>) {
        match self.store.list() {
            Ok(configs) => *self.configs.borrow_mut() = configs,
            Err(e) => self.notify(&format!("Could not read containers: {}", e)),
        }
        self.refresh_sidebar();
        self.refresh_states();
        let selected = self.selected.borrow().clone();
        match selected {
            Some(name) if self.configs.borrow().iter().any(|c| c.name == name) => {
                self.select(&name);
            }
            _ => {
                *self.selected.borrow_mut() = None;
                self.stack.set_visible_child_name("empty");
            }
        }
    }

    /// Rebuild the sidebar from self.configs.
    pub fn refresh_sidebar(&self) {
        self.suppress.set(true);
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        let configs = self.configs.borrow();
        let mut entries = Vec::new();
        for cfg in configs.iter() {
            let (row, dot, subtitle) = widgets::sidebar_row(&cfg.name);
            row.set_widget_name(&cfg.name);
            self.list.append(&row);
            entries.push(SidebarEntry {
                name: cfg.name.clone(),
                dot,
                subtitle,
            });
        }
        *self.entries.borrow_mut() = entries;
        self.suppress.set(false);

        if let Some(name) = self.selected.borrow().clone() {
            if let Some(row) = self.find_row(&name) {
                self.suppress.set(true);
                self.list.select_row(Some(&row));
                self.suppress.set(false);
            }
        }
    }

    fn find_row(&self, name: &str) -> Option<gtk::ListBoxRow> {
        let mut index = 0;
        while let Some(row) = self.list.row_at_index(index) {
            if row.widget_name() == name {
                return Some(row);
            }
            index += 1;
        }
        None
    }

    /// Select a container and build its detail page.
    pub fn select(self: &Rc<Self>, name: &str) {
        *self.selected.borrow_mut() = Some(name.to_string());
        let cfg = match self.store.load(name) {
            Ok(cfg) => cfg,
            Err(e) => {
                self.notify(&format!("{}", e));
                return;
            }
        };
        let widget = detail::build(self, cfg);
        while let Some(child) = self.detail_slot.first_child() {
            self.detail_slot.remove(&child);
        }
        self.detail_slot.append(&widget);
        self.stack.set_visible_child_name("detail");
    }

    /// Persist a configuration (used by every editor in the detail page).
    pub fn save_config(&self, cfg: &ContainerConfig, announce: bool) {
        match self.store.save(cfg) {
            Ok(()) => {
                if let Some(slot) = self
                    .configs
                    .borrow_mut()
                    .iter_mut()
                    .find(|c| c.name == cfg.name)
                {
                    *slot = cfg.clone();
                }
                if announce {
                    self.notify("Saved");
                }
            }
            Err(e) => self.notify(&format!("Could not save: {}", e)),
        }
    }

    /// Register the detail page's own status refresher.
    pub fn set_detail_refresher(&self, refresher: Box<dyn Fn()>) {
        *self.detail_refresher.borrow_mut() = Some(refresher);
    }

    /// Refresh the running/stopped indicators.
    pub fn refresh_states(&self) {
        // One machinectl call covers containers started interactively as well.
        let registered = command::running_machines();
        let configs = self.configs.borrow();
        for entry in self.entries.borrow().iter() {
            let cfg = configs.iter().find(|c| c.name == entry.name);
            let machine = cfg
                .map(|c| c.machine_name().to_string())
                .unwrap_or_else(|| entry.name.clone());
            let unit = cfg
                .map(|c| c.transient_unit())
                .unwrap_or_else(|| nspawn_studio_core::model::transient_unit_name(&entry.name));
            let state = if registered.iter().any(|m| m == &machine) {
                MachineState::Running
            } else {
                command::machine_state(&unit)
            };
            widgets::set_dot(&entry.dot, state);
            entry.subtitle.set_text(state.label());
        }
        drop(configs);
        if let Some(refresher) = self.detail_refresher.borrow().as_ref() {
            refresher();
        }
    }
}

