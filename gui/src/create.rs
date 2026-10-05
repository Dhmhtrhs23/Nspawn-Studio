//! The "new container" dialog.

use crate::state::App;
use crate::task;
use crate::widgets;
use gtk4 as gtk;
use libadwaita as adw;
use adw::prelude::*;
use nspawn_studio_core::generator;
use nspawn_studio_core::model::{
    BootMode, ContainerConfig, ContainerUser, DebootstrapSpec, SourceKind, X11Access,
};
use nspawn_studio_core::validate;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

pub fn present(app: &Rc<App>) {
    let window = adw::Window::builder()
        .transient_for(&app.window)
        .modal(true)
        .title("New container")
        .default_width(640)
        .default_height(760)
        .build();

    let header = adw::HeaderBar::new();
    let cancel = gtk::Button::with_label("Cancel");
    let create = gtk::Button::with_label("Create");
    create.add_css_class("suggested-action");
    header.pack_start(&cancel);
    header.pack_end(&create);

    let banner = adw::Banner::new("");
    banner.set_revealed(false);

    let page = adw::PreferencesPage::new();

    let general = widgets::group("General", None);
    let name_row = adw::EntryRow::builder().title("Name").build();
    general.add(&name_row);

    let source_row = {
        let model = gtk::StringList::new(&[
            "Debootstrap a new Debian system",
            "Use an existing chroot directory",
        ]);
        let row = adw::ComboRow::builder().title("Source").build();
        row.set_model(Some(&model));
        row.set_selected(0);
        row
    };
    general.add(&source_row);

    let rootfs_row = adw::EntryRow::builder().title("Root filesystem").build();
    let browse = gtk::Button::builder()
        .icon_name("folder-open-symbolic")
        .tooltip_text("Choose an existing chroot directory")
        .build();
    rootfs_row.add_suffix(&browse);
    general.add(&rootfs_row);

    let boot_row = {
        let model = gtk::StringList::new(&["Boot the container init (systemd)", "Run a command"]);
        let row = adw::ComboRow::builder().title("Start mode").build();
        row.set_model(Some(&model));
        row.set_selected(0);
        row
    };
    general.add(&boot_row);

    let command_row = adw::EntryRow::builder().title("Command").build();
    command_row.set_text("/bin/bash -l");
    command_row.set_visible(false);
    general.add(&command_row);
    {
        let command_row = command_row.clone();
        boot_row.connect_selected_notify(move |row| {
            command_row.set_visible(row.selected() == 1);
        });
    }

    let hostname_row = adw::EntryRow::builder().title("Hostname (optional)").build();
    general.add(&hostname_row);

    let x11_row = {
        let model = gtk::StringList::new(&[
            "None",
            "Sockets only (needs xhost)",
            "Sockets and xauth cookie (recommended)",
            "Xephyr (isolated nested X server)",
        ]);
        let row = adw::ComboRow::builder().title("X11 access").build();
        row.set_model(Some(&model));
        row.set_selected(2);
        row
    };
    general.add(&x11_row);

    let root_password_row = adw::PasswordEntryRow::builder()
        .title("Root password")
        .build();
    general.add(&root_password_row);
    page.add(&general);

    let users_rc: Rc<RefCell<Vec<ContainerUser>>> = Rc::new(RefCell::new(Vec::new()));
    let users_group = crate::users::build_list(&app, &users_rc);
    page.add(&users_group);

    let deb_group = widgets::group("Debootstrap", Some("Packages and suite to download"));
    let spec = DebootstrapSpec::default();
    let suite_row = adw::EntryRow::builder().title("Suite").build();
    suite_row.set_text(&spec.suite);
    let mirror_row = adw::EntryRow::builder().title("Mirror").build();
    mirror_row.set_text(&spec.mirror);
    let arch_row = adw::EntryRow::builder().title("Architecture").build();
    arch_row.set_text(&spec.arch);
    let variant_row = adw::EntryRow::builder().title("Variant").build();
    variant_row.set_text(&spec.variant);
    let include_row = adw::EntryRow::builder().title("Packages (comma separated)").build();
    include_row.set_text(&spec.include.join(", "));
    deb_group.add(&suite_row);
    deb_group.add(&mirror_row);
    deb_group.add(&arch_row);
    deb_group.add(&variant_row);
    deb_group.add(&include_row);

    let build_now_row = adw::SwitchRow::builder()
        .title("Build the root filesystem now")
        .subtitle("Runs debootstrap; this can take several minutes")
        .build();
    build_now_row.set_active(true);
    deb_group.add(&build_now_row);
    page.add(&deb_group);

    let user_edited = Rc::new(Cell::new(false));

    {
        let rootfs_row = rootfs_row.clone();
        let user_edited = user_edited.clone();
        let store = app.store.clone();
        name_row.connect_changed(move |row| {
            if user_edited.get() {
                return;
            }
            let name = row.text().to_string();
            if name.trim().is_empty() {
                rootfs_row.set_text("");
            } else {
                rootfs_row.set_text(&store.default_rootfs(name.trim()).to_string_lossy());
            }
        });
    }
    {
        let user_edited = user_edited.clone();
        let deb_group = deb_group.clone();
        let build_now_row = build_now_row.clone();
        source_row.connect_selected_notify(move |row| {
            let custom = row.selected() == 1;
            deb_group.set_visible(!custom);
            build_now_row.set_visible(!custom);
            if custom {
                user_edited.set(true);
            }
        });
    }
    {
        let rootfs_row = rootfs_row.clone();
        let user_edited = user_edited.clone();
        let window = window.clone();
        browse.connect_clicked(move |_| {
            let dialog = gtk::FileDialog::builder()
                .title("Select an existing chroot directory")
                .build();
            let rootfs_row = rootfs_row.clone();
            let user_edited = user_edited.clone();
            let parent = window.clone();
            dialog.select_folder(Some(&parent), None::<&gtk::gio::Cancellable>, move |res| {
                if let Ok(file) = res {
                    if let Some(path) = file.path() {
                        rootfs_row.set_text(&path.to_string_lossy());
                        user_edited.set(true);
                    }
                }
            });
        });
    }
    {
        let window = window.clone();
        cancel.connect_clicked(move |_| window.close());
    }

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&banner);
    content.append(&page);
    toolbar.set_content(Some(&content));
    window.set_content(Some(&toolbar));

    {
        let window = window.clone();
        let banner = banner.clone();
        let name_row = name_row.clone();
        let rootfs_row = rootfs_row.clone();
        let source_row = source_row.clone();
        let boot_row = boot_row.clone();
        let command_row = command_row.clone();
        let hostname_row = hostname_row.clone();
        let x11_row = x11_row.clone();
        let suite_row = suite_row.clone();
        let mirror_row = mirror_row.clone();
        let arch_row = arch_row.clone();
        let variant_row = variant_row.clone();
        let include_row = include_row.clone();
        let build_now_row = build_now_row.clone();
        let root_password_row = root_password_row.clone();
        let users_rc = users_rc.clone();
        let app = app.clone();
        create.connect_clicked(move |_| {
            let name = name_row.text().trim().to_string();
            let rootfs_text = rootfs_row.text().trim().to_string();
            if name.is_empty() {
                banner.set_title("Please enter a name for the container");
                banner.set_revealed(true);
                return;
            }
            if rootfs_text.is_empty() {
                banner.set_title("Please choose a root filesystem directory");
                banner.set_revealed(true);
                return;
            }
            let name_errors = validate::validate_container_name(&name);
            if !name_errors.is_empty() {
                banner.set_title(&name_errors.join("; "));
                banner.set_revealed(true);
                return;
            }
            if app.store.exists(&name) {
                banner.set_title("A container with this name already exists");
                banner.set_revealed(true);
                return;
            }
            let rootfs = PathBuf::from(&rootfs_text);
            let custom = source_row.selected() == 1;
            if custom && !rootfs.is_dir() {
                banner.set_title("The selected chroot directory does not exist");
                banner.set_revealed(true);
                return;
            }

            let mut cfg = ContainerConfig::new(name.clone(), rootfs.clone());
            cfg.source = if custom {
                SourceKind::Custom
            } else {
                SourceKind::Debootstrap
            };
            cfg.boot = if boot_row.selected() == 1 {
                BootMode::Command
            } else {
                BootMode::Boot
            };
            cfg.command = command_row.text().to_string();
            cfg.x11 = match x11_row.selected() {
                1 => X11Access::Socket,
                2 => X11Access::Authority,
                3 => X11Access::Xephyr,
                _ => X11Access::None,
            };
            let hostname = hostname_row.text().trim().to_string();
            cfg.hostname = if hostname.is_empty() {
                None
            } else {
                Some(hostname)
            };
            cfg.debootstrap = DebootstrapSpec {
                suite: suite_row.text().trim().to_string(),
                mirror: mirror_row.text().trim().to_string(),
                arch: arch_row.text().trim().to_string(),
                variant: variant_row.text().trim().to_string(),
                include: widgets::parse_list(&include_row.text()),
                ..DebootstrapSpec::default()
            };
            cfg.root_password = root_password_row.text().to_string();
            cfg.users = users_rc.borrow().clone();

            let mut errors = validate::validate_config(&cfg);
            if !custom {
                errors.extend(validate::validate_debootstrap(&cfg));
            }
            if !errors.is_empty() {
                banner.set_title(&errors.join("; "));
                banner.set_revealed(true);
                return;
            }

            if let Err(e) = std::fs::create_dir_all(rootfs.parent().unwrap_or(&rootfs)) {
                banner.set_title(&format!("Could not create parent directory: {}", e));
                banner.set_revealed(true);
                return;
            }
            if let Err(e) = app.store.save(&cfg) {
                banner.set_title(&format!("Could not save: {}", e));
                banner.set_revealed(true);
                return;
            }

            let build = !custom && build_now_row.is_active();
            let needs_provision = !cfg.root_password.is_empty() || !cfg.users.is_empty();
            let provision_cfg = cfg.clone();
            let app = app.clone();
            window.close();
            app.reload();
            app.select(&name);
            if build {
                let argv = generator::debootstrap_argv(&cfg);
                let program = argv[0].clone();
                let args = argv[1..].to_vec();
                let created = name.clone();
                task::run_command_dialog(
                    &app,
                    "Creating the root filesystem",
                    &program,
                    args,
                    move |app| {
                        app.notify("Root filesystem created");
                        app.select(&created);
                        if needs_provision {
                            crate::users::apply_to_container(app, &provision_cfg);
                        }
                    },
                );
            } else {
                app.notify("Container created");
                if needs_provision {
                    crate::users::apply_to_container(&app, &provision_cfg);
                }
            }
        });
    }

    window.present();
}
