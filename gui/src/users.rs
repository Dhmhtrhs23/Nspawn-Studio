//! Account provisioning: the root password and extra users.
//!
//! Passwords are kept in the (root-only, mode 0600) container configuration
//! so the container can be re-provisioned later. They are written into the
//! container's root filesystem with chpasswd by running systemd-nspawn.

use crate::state::App;
use crate::task;
use crate::widgets;
use gtk4 as gtk;
use libadwaita as adw;
use adw::prelude::*;
use nspawn_studio_core::command;
use nspawn_studio_core::generator;
use nspawn_studio_core::model::{ContainerConfig, ContainerUser};
use nspawn_studio_core::validate;
use std::cell::RefCell;
use std::rc::Rc;

/// Ask for a user name, a password and whether the account gets sudo.
pub fn add_user_dialog(app: &Rc<App>, on_add: Rc<dyn Fn(ContainerUser)>) {
    let window = adw::Window::builder()
        .transient_for(&app.window)
        .modal(true)
        .title("Add a user")
        .default_width(480)
        .default_height(430)
        .build();

    let header = adw::HeaderBar::new();
    let add = gtk::Button::with_label("Add");
    add.add_css_class("suggested-action");
    header.pack_end(&add);

    let banner = adw::Banner::new("");
    banner.set_revealed(false);

    let group = widgets::group("Account", None);
    let name_row = adw::EntryRow::builder().title("User name").build();
    let password_row = adw::PasswordEntryRow::builder().title("Password").build();
    let confirm_row = adw::PasswordEntryRow::builder().title("Confirm password").build();
    let sudo_row = adw::SwitchRow::builder()
        .title("Grant sudo")
        .subtitle("Add the user to the sudo group")
        .build();
    group.add(&name_row);
    group.add(&password_row);
    group.add(&confirm_row);
    group.add(&sudo_row);

    let page = adw::PreferencesPage::new();
    page.add(&group);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&banner);
    content.append(&page);
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&content));
    window.set_content(Some(&toolbar));

    {
        let window = window.clone();
        let banner = banner.clone();
        add.connect_clicked(move |_| {
            let name = name_row.text().trim().to_string();
            let password = password_row.text().to_string();
            let confirm = confirm_row.text().to_string();
            if !validate::valid_username(&name) {
                banner.set_title(
                    "Use lowercase letters, digits, '_' and '-', starting with a letter or '_' (max 32)",
                );
                banner.set_revealed(true);
                return;
            }
            if password.is_empty() {
                banner.set_title("The password must not be empty");
                banner.set_revealed(true);
                return;
            }
            if password.contains('\n') {
                banner.set_title("The password must not contain a newline");
                banner.set_revealed(true);
                return;
            }
            if password != confirm {
                banner.set_title("The two passwords do not match");
                banner.set_revealed(true);
                return;
            }
            on_add(ContainerUser {
                name,
                password,
                sudo: sudo_row.is_active(),
            });
            window.close();
        });
    }

    window.present();
}

/// A simple editable list of users (no root password, no apply button). Used
/// by the "new container" dialog.
pub fn build_list(app: &Rc<App>, users: &Rc<RefCell<Vec<ContainerUser>>>) -> adw::PreferencesGroup {
    let app: Rc<App> = Rc::clone(app);
    let group = widgets::group(
        "Users",
        Some("Accounts created inside the container once the root filesystem exists"),
    );

    let rows: Rc<RefCell<Vec<gtk::Widget>>> = Rc::new(RefCell::new(Vec::new()));
    let holder: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

    let add_row = adw::ActionRow::builder().title("Add a user").build();
    let add_button = gtk::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text("Add")
        .build();
    add_button.add_css_class("flat");
    add_row.add_suffix(&add_button);

    let refresh: Rc<dyn Fn()> = {
        let group = group.clone();
        let rows = rows.clone();
        let users = users.clone();
        let holder = holder.clone();
        let add_row = add_row.clone();
        Rc::new(move || {
            for row in rows.borrow().iter() {
                group.remove(row);
            }
            rows.borrow_mut().clear();
            let snapshot = users.borrow().clone();
            for (index, user) in snapshot.iter().enumerate() {
                let subtitle = if user.sudo { "sudo" } else { "no sudo" };
                let row = adw::ActionRow::builder()
                    .title(user.name.as_str())
                    .subtitle(subtitle)
                    .build();
                let remove = gtk::Button::builder()
                    .icon_name("list-remove-symbolic")
                    .tooltip_text("Remove")
                    .build();
                remove.add_css_class("flat");
                row.add_suffix(&remove);
                {
                    let users = users.clone();
                    let holder = holder.clone();
                    remove.connect_clicked(move |_| {
                        if index < users.borrow().len() {
                            users.borrow_mut().remove(index);
                        }
                        if let Some(refresh) = holder.borrow().as_ref() {
                            refresh();
                        }
                    });
                }
                group.add(&row);
                rows.borrow_mut().push(row.upcast());
            }
            group.add(&add_row);
            rows.borrow_mut().push(add_row.clone().upcast());
        })
    };
    *holder.borrow_mut() = Some(refresh.clone());
    refresh();

    {
        let app = app.clone();
        let users = users.clone();
        let holder = holder.clone();
        add_button.connect_clicked(move |_| {
            let users = users.clone();
            let holder = holder.clone();
            add_user_dialog(
                &app,
                Rc::new(move |user| {
                    users.borrow_mut().push(user);
                    if let Some(refresh) = holder.borrow().as_ref() {
                        refresh();
                    }
                }),
            );
        });
    }

    group
}

/// The full account editor for the detail page.
pub fn build_group(app: &Rc<App>, work: &Rc<RefCell<ContainerConfig>>) -> adw::PreferencesGroup {
    let app: Rc<App> = Rc::clone(app);
    let group = widgets::group(
        "Users and passwords",
        Some("Written into the container's root filesystem with systemd-nspawn"),
    );

    let root_row = adw::PasswordEntryRow::builder()
        .title("Root password")
        .build();
    root_row.set_text(&work.borrow().root_password);
    {
        let app = app.clone();
        let work = work.clone();
        root_row.connect_changed(move |row| {
            let value = row.text().to_string();
            let snapshot = {
                let mut cfg = work.borrow_mut();
                cfg.root_password = value;
                cfg.touch();
                cfg.clone()
            };
            app.save_config(&snapshot, false);
        });
    }
    group.add(&root_row);

    let rows: Rc<RefCell<Vec<gtk::Widget>>> = Rc::new(RefCell::new(Vec::new()));
    let holder: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

    let add_row = adw::ActionRow::builder().title("Add a user").build();
    let add_button = gtk::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text("Add")
        .build();
    add_button.add_css_class("flat");
    add_row.add_suffix(&add_button);

    let apply_row = adw::ActionRow::builder()
        .title("Apply users and passwords")
        .subtitle("Stop the container first; this writes accounts into its root filesystem")
        .build();
    let apply_button = gtk::Button::with_label("Apply");
    apply_button.add_css_class("flat");
    apply_row.add_suffix(&apply_button);

    let refresh_app = Rc::clone(&app);
    let refresh: Rc<dyn Fn()> = {
        let group = group.clone();
        let rows = rows.clone();
        let work = work.clone();
        let holder = holder.clone();
        let add_row = add_row.clone();
        let apply_row = apply_row.clone();
        Rc::new(move || {
            for row in rows.borrow().iter() {
                group.remove(row);
            }
            rows.borrow_mut().clear();
            let snapshot = work.borrow().clone();
            for (index, user) in snapshot.users.iter().enumerate() {
                let subtitle = if user.sudo {
                    "sudo allowed".to_string()
                } else {
                    "no sudo".to_string()
                };
                let row = adw::ActionRow::builder()
                    .title(user.name.as_str())
                    .subtitle(subtitle)
                    .build();
                let remove = gtk::Button::builder()
                    .icon_name("list-remove-symbolic")
                    .tooltip_text("Remove")
                    .build();
                remove.add_css_class("flat");
                row.add_suffix(&remove);
                {
                    let app = Rc::clone(&refresh_app);
                    let work = work.clone();
                    let holder = holder.clone();
                    remove.connect_clicked(move |_| {
                        let snapshot = {
                            let mut cfg = work.borrow_mut();
                            if index < cfg.users.len() {
                                cfg.users.remove(index);
                            }
                            cfg.touch();
                            cfg.clone()
                        };
                        app.save_config(&snapshot, false);
                        if let Some(refresh) = holder.borrow().as_ref() {
                            refresh();
                        }
                    });
                }
                group.add(&row);
                rows.borrow_mut().push(row.upcast());
            }
            group.add(&add_row);
            rows.borrow_mut().push(add_row.clone().upcast());
            group.add(&apply_row);
            rows.borrow_mut().push(apply_row.clone().upcast());
        })
    };
    *holder.borrow_mut() = Some(refresh.clone());
    refresh();

    {
        let app = app.clone();
        let work = work.clone();
        let holder = holder.clone();
        add_button.connect_clicked(move |_| {
            let app_for_dialog = Rc::clone(&app);
            let app2 = Rc::clone(&app);
            let work2 = work.clone();
            let holder2 = holder.clone();
            add_user_dialog(
                &app_for_dialog,
                Rc::new(move |user| {
                    let snapshot = {
                        let mut cfg = work2.borrow_mut();
                        cfg.users.push(user);
                        cfg.touch();
                        cfg.clone()
                    };
                    app2.save_config(&snapshot, false);
                    if let Some(refresh) = holder2.borrow().as_ref() {
                        refresh();
                    }
                }),
            );
        });
    }

    {
        let app = app.clone();
        let work = work.clone();
        apply_button.connect_clicked(move |_| {
            let cfg = work.borrow().clone();
            apply_to_container(&app, &cfg);
        });
    }

    group
}

/// Write the configured accounts into the container's root filesystem.
pub fn apply_to_container(app: &Rc<App>, cfg: &ContainerConfig) {
    if cfg.root_password.is_empty() && cfg.users.is_empty() {
        app.notify("Set a root password or add a user first");
        return;
    }
    if !cfg.rootfs.is_dir() {
        app.notify("The root filesystem does not exist yet");
        return;
    }
    if command::machine_registered(cfg.machine_name())
        || command::machine_state(&cfg.transient_unit()).is_running()
    {
        app.notify("Stop the container before applying users and passwords");
        return;
    }
    let script = generator::provision_script(cfg);
    let rootfs = cfg.rootfs.to_string_lossy().to_string();
    task::run_command_dialog(
        app,
        "Applying users and passwords",
        "systemd-nspawn",
        vec![
            "-D".to_string(),
            rootfs,
            "/bin/sh".to_string(),
            "-c".to_string(),
            script,
        ],
        |app| app.notify("Users and passwords applied"),
    );
}
