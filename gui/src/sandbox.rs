//! Sandboxing UI: switches, checkbox menus for capabilities and system call
//! groups (with a description for each), and list editors for paths.

use crate::state::App;
use crate::widgets;
use gtk4 as gtk;
use libadwaita as adw;
use adw::prelude::*;
use nspawn_studio_core::model::{ContainerConfig, OwnershipMode, SecurityConfig};
use nspawn_studio_core::security::{DescribedOption, CAPABILITY_INFO, SYSCALL_GROUP_INFO};
use std::cell::RefCell;
use std::rc::Rc;

fn switch_row(title: &str, subtitle: &str, active: bool) -> adw::SwitchRow {
    let row = adw::SwitchRow::builder()
        .title(title)
        .subtitle(subtitle)
        .build();
    row.set_active(active);
    row
}

fn combo_row(title: &str, options: &[&str], selected: u32) -> adw::ComboRow {
    let model = gtk::StringList::new(options);
    let row = adw::ComboRow::builder().title(title).build();
    row.set_model(Some(&model));
    row.set_selected(selected);
    row
}

fn edit<F>(app: &Rc<App>, work: &Rc<RefCell<ContainerConfig>>, change: F)
where
    F: FnOnce(&mut ContainerConfig),
{
    let snapshot = {
        let mut cfg = work.borrow_mut();
        change(&mut cfg);
        cfg.touch();
        cfg.clone()
    };
    app.save_config(&snapshot, false);
}

fn summarize(list: &[String]) -> String {
    if list.is_empty() {
        "none".to_string()
    } else if list.len() <= 4 {
        list.join(", ")
    } else {
        format!("{} selected", list.len())
    }
}

/// Add every sandboxing group to a preferences page.
pub fn install(app: &Rc<App>, page: &adw::PreferencesPage, work: &Rc<RefCell<ContainerConfig>>) {
    page.add(&switches_group(app, work));
    page.add(&list_group(
        app,
        work,
        "Inaccessible (masked) paths",
        Some("Paths that exist in the container image are hidden from it"),
        "Add a path",
        "Path inside the container",
        |c| c.security.mask.clone(),
        |c, v| c.security.mask = v,
    ));
    page.add(&list_group(
        app,
        work,
        "tmpfs mounts",
        Some("Directories backed by a fresh in-memory file system"),
        "Add a mount point",
        "Path inside the container",
        |c| c.security.tmpfs.clone(),
        |c, v| c.security.tmpfs = v,
    ));
    page.add(&list_group(
        app,
        work,
        "Extra systemd-nspawn arguments",
        Some("Raw arguments appended verbatim to the command line"),
        "Add an argument",
        "Argument (for example --cpu-affinity=0)",
        |c| c.extra_args.clone(),
        |c, v| c.extra_args = v,
    ));
}

fn switches_group(
    app: &Rc<App>,
    work: &Rc<RefCell<ContainerConfig>>,
) -> adw::PreferencesGroup {
    let group = widgets::group(
        "Security and sandboxing",
        Some("Hardening options passed to systemd-nspawn"),
    );

    let recommended_row = adw::ActionRow::builder()
        .title("Apply recommended hardening")
        .subtitle("User namespace, seccomp filter, no-new-privileges, dropped capabilities")
        .build();
    let recommended_button = gtk::Button::with_label("Apply");
    recommended_button.add_css_class("flat");
    recommended_row.add_suffix(&recommended_button);
    {
        let app = Rc::clone(app);
        let work = Rc::clone(work);
        recommended_button.connect_clicked(move |_| {
            let snapshot = {
                let mut cfg = work.borrow_mut();
                cfg.security = SecurityConfig::recommended();
                cfg.touch();
                cfg.clone()
            };
            app.save_config(&snapshot, false);
            app.select(&snapshot.name);
            app.notify("Recommended hardening applied");
        });
    }
    group.add(&recommended_row);

    let private_users_row = switch_row(
        "User namespacing",
        "Run the container in a private user namespace (-U)",
        work.borrow().security.private_users,
    );
    {
        let app = Rc::clone(app);
        let work = Rc::clone(work);
        private_users_row.connect_active_notify(move |row| {
            let active = row.is_active();
            edit(&app, &work, move |cfg| cfg.security.private_users = active);
        });
    }
    group.add(&private_users_row);

    let ownership_row = combo_row(
        "Namespace ownership",
        &["off", "auto", "chown", "map"],
        match work.borrow().security.ownership {
            OwnershipMode::Off => 0,
            OwnershipMode::Auto => 1,
            OwnershipMode::Chown => 2,
            OwnershipMode::Map => 3,
        },
    );
    {
        let app = Rc::clone(app);
        let work = Rc::clone(work);
        ownership_row.connect_selected_notify(move |row| {
            let index = row.selected();
            edit(&app, &work, move |cfg| {
                cfg.security.ownership = match index {
                    0 => OwnershipMode::Off,
                    2 => OwnershipMode::Chown,
                    3 => OwnershipMode::Map,
                    _ => OwnershipMode::Auto,
                };
            });
        });
    }
    group.add(&ownership_row);

    let read_only_row = switch_row(
        "Read-only root",
        "Mount the root filesystem read-only",
        work.borrow().security.read_only,
    );
    {
        let app = Rc::clone(app);
        let work = Rc::clone(work);
        read_only_row.connect_active_notify(move |row| {
            let active = row.is_active();
            edit(&app, &work, move |cfg| cfg.security.read_only = active);
        });
    }
    group.add(&read_only_row);

    let ephemeral_row = switch_row(
        "Ephemeral",
        "Discard all changes when the container stops",
        work.borrow().security.ephemeral,
    );
    {
        let app = Rc::clone(app);
        let work = Rc::clone(work);
        ephemeral_row.connect_active_notify(move |row| {
            let active = row.is_active();
            edit(&app, &work, move |cfg| cfg.security.ephemeral = active);
        });
    }
    group.add(&ephemeral_row);

    let nnp_row = switch_row(
        "No new privileges",
        "Set PR_SET_NO_NEW_PRIVS for the container payload",
        work.borrow().security.no_new_privileges,
    );
    {
        let app = Rc::clone(app);
        let work = Rc::clone(work);
        nnp_row.connect_active_notify(move |row| {
            let active = row.is_active();
            edit(&app, &work, move |cfg| cfg.security.no_new_privileges = active);
        });
    }
    group.add(&nnp_row);

    // --- Drop capabilities ---
    let drop_row = adw::ActionRow::builder()
        .title("Drop capabilities")
        .subtitle(summarize(&work.borrow().security.drop_capabilities))
        .build();
    let drop_button = gtk::Button::with_label("Choose");
    drop_button.add_css_class("flat");
    drop_row.add_suffix(&drop_button);
    {
        let app = Rc::clone(app);
        let work = Rc::clone(work);
        let row = drop_row.clone();
        drop_button.connect_clicked(move |_| {
            let selected = work.borrow().security.drop_capabilities.clone();
            let app2 = Rc::clone(&app);
            let work2 = Rc::clone(&work);
            let row2 = row.clone();
            present_capability_dialog(
                &app,
                "Drop capabilities",
                "Checked capabilities are removed from the container's default set",
                selected,
                Rc::new(move |chosen: Vec<String>| {
                    let snapshot = {
                        let mut cfg = work2.borrow_mut();
                        cfg.security.drop_capabilities = chosen.clone();
                        cfg.touch();
                        cfg.clone()
                    };
                    app2.save_config(&snapshot, false);
                    row2.set_subtitle(&summarize(&chosen));
                }),
            );
        });
    }
    group.add(&drop_row);

    // --- Add capabilities ---
    let add_row = adw::ActionRow::builder()
        .title("Add capabilities")
        .subtitle(summarize(&work.borrow().security.add_capabilities))
        .build();
    let add_button = gtk::Button::with_label("Choose");
    add_button.add_css_class("flat");
    add_row.add_suffix(&add_button);
    {
        let app = Rc::clone(app);
        let work = Rc::clone(work);
        let row = add_row.clone();
        add_button.connect_clicked(move |_| {
            let selected = work.borrow().security.add_capabilities.clone();
            let app2 = Rc::clone(&app);
            let work2 = Rc::clone(&work);
            let row2 = row.clone();
            present_capability_dialog(
                &app,
                "Add capabilities",
                "Checked capabilities are retained in addition to the defaults",
                selected,
                Rc::new(move |chosen: Vec<String>| {
                    let snapshot = {
                        let mut cfg = work2.borrow_mut();
                        cfg.security.add_capabilities = chosen.clone();
                        cfg.touch();
                        cfg.clone()
                    };
                    app2.save_config(&snapshot, false);
                    row2.set_subtitle(&summarize(&chosen));
                }),
            );
        });
    }
    group.add(&add_row);

    // --- System call filter ---
    let denied = work.borrow().security.system_call_filter.clone();
    let syscall_row = adw::ActionRow::builder()
        .title("Deny system calls")
        .subtitle(summarize(&denied))
        .build();
    let syscall_button = gtk::Button::with_label("Choose");
    syscall_button.add_css_class("flat");
    syscall_row.add_suffix(&syscall_button);
    {
        let app = Rc::clone(app);
        let work = Rc::clone(work);
        let row = syscall_row.clone();
        syscall_button.connect_clicked(move |_| {
            let selected = work.borrow().security.system_call_filter.clone();
            let app2 = Rc::clone(&app);
            let work2 = Rc::clone(&work);
            let row2 = row.clone();
            present_syscall_dialog(
                &app,
                selected,
                Rc::new(move |chosen: Vec<String>| {
                    let snapshot = {
                        let mut cfg = work2.borrow_mut();
                        cfg.security.system_call_filter = chosen.clone();
                        cfg.touch();
                        cfg.clone()
                    };
                    app2.save_config(&snapshot, false);
                    row2.set_subtitle(&summarize(&chosen));
                }),
            );
        });
    }
    group.add(&syscall_row);

    group
}

// ---------------------------------------------------------------------------
// Checkbox dialogs
// ---------------------------------------------------------------------------

struct CheckList {
    rows: Rc<RefCell<Vec<(String, gtk::CheckButton, adw::ActionRow)>>>,
    view: gtk::ScrolledWindow,
}

fn check_list(options: &'static [DescribedOption], checked: &dyn Fn(&str) -> bool) -> CheckList {
    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let rows: Rc<RefCell<Vec<(String, gtk::CheckButton, adw::ActionRow)>>> =
        Rc::new(RefCell::new(Vec::new()));
    for option in options {
        let row = adw::ActionRow::builder()
            .title(option.id)
            .subtitle(option.description)
            .build();
        let check = gtk::CheckButton::new();
        check.set_active(checked(option.id));
        check.set_valign(gtk::Align::Center);
        row.add_suffix(&check);
        row.set_activatable_widget(Some(&check));
        list.append(&row);
        rows.borrow_mut().push((option.id.to_string(), check, row));
    }
    let view = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .child(&list)
        .build();
    CheckList { rows, view }
}

fn option_window(
    app: &Rc<App>,
    title: &str,
    description: &str,
    view: &gtk::ScrolledWindow,
) -> (adw::Window, gtk::Button, gtk::SearchEntry) {
    let window = adw::Window::builder()
        .transient_for(&app.window)
        .modal(true)
        .title(title)
        .default_width(620)
        .default_height(680)
        .build();
    let header = adw::HeaderBar::new();
    let apply = gtk::Button::with_label("Apply");
    apply.add_css_class("suggested-action");
    header.pack_end(&apply);
    let cancel = gtk::Button::with_label("Cancel");
    header.pack_start(&cancel);
    {
        let window = window.clone();
        cancel.connect_clicked(move |_| window.close());
    }

    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Filter"));
    search.set_margin_top(6);
    search.set_margin_bottom(6);
    search.set_margin_start(12);
    search.set_margin_end(12);

    let description_label = gtk::Label::new(Some(description));
    description_label.set_xalign(0.0);
    description_label.set_wrap(true);
    description_label.add_css_class("dim-label");
    description_label.set_margin_start(12);
    description_label.set_margin_end(12);
    description_label.set_margin_bottom(6);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&search);
    content.append(&description_label);
    content.append(view);
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&content));
    window.set_content(Some(&toolbar));
    (window, apply, search)
}

fn wire_filter(search: &gtk::SearchEntry, rows: &Rc<RefCell<Vec<(String, gtk::CheckButton, adw::ActionRow)>>>) {
    let rows = Rc::clone(rows);
    search.connect_search_changed(move |entry| {
        let needle = entry.text().to_string().to_lowercase();
        for (id, _check, row) in rows.borrow().iter() {
            let haystack = id.to_lowercase();
            row.set_visible(needle.is_empty() || haystack.contains(&needle));
        }
    });
}

fn checked_ids(rows: &Rc<RefCell<Vec<(String, gtk::CheckButton, adw::ActionRow)>>>) -> Vec<String> {
    rows.borrow()
        .iter()
        .filter(|(_, check, _)| check.is_active())
        .map(|(id, _, _)| id.clone())
        .collect()
}

fn present_capability_dialog(
    app: &Rc<App>,
    title: &str,
    description: &str,
    selected: Vec<String>,
    on_apply: Rc<dyn Fn(Vec<String>)>,
) {
    let is_checked = move |id: &str| selected.iter().any(|s| s == id);
    let list = check_list(CAPABILITY_INFO, &is_checked);
    let (window, apply, search) = option_window(app, title, description, &list.view);
    wire_filter(&search, &list.rows);
    {
        let window = window.clone();
        apply.connect_clicked(move |_| {
            on_apply(checked_ids(&list.rows));
            window.close();
        });
    }
    window.present();
}

fn present_syscall_dialog(
    app: &Rc<App>,
    selected: Vec<String>,
    on_apply: Rc<dyn Fn(Vec<String>)>,
) {
    // Selected entries are either ~@group or a raw ~syscall.
    let groups: Vec<String> = selected
        .iter()
        .filter(|s| s.starts_with("~@"))
        .cloned()
        .collect();
    let custom: Vec<String> = selected
        .iter()
        .filter(|s| !s.starts_with("~@"))
        .map(|s| s.trim_start_matches('~').to_string())
        .collect();

    let is_checked = move |id: &str| {
        let want = format!("~{}", id);
        groups.iter().any(|s| s == &want)
    };
    let list = check_list(SYSCALL_GROUP_INFO, &is_checked);
    let (window, apply, search) = option_window(
        app,
        "Deny system calls",
        "Checked groups are removed with --system-call-filter=~@group",
        &list.view,
    );
    wire_filter(&search, &list.rows);

    let custom_row = adw::EntryRow::builder()
        .title("Custom system calls or groups (comma separated)")
        .build();
    custom_row.set_text(&custom.join(", "));
    // Put the custom entry in a small group above the list by rebuilding the
    // content the window already has.
    let content = window
        .content()
        .and_then(|c| c.downcast::<adw::ToolbarView>().ok());
    if let Some(toolbar) = content {
        if let Some(existing) = toolbar.content() {
            if let Some(bx) = existing.downcast_ref::<gtk::Box>() {
                let entry_group = adw::PreferencesGroup::new();
                entry_group.add(&custom_row);
                entry_group.set_margin_start(12);
                entry_group.set_margin_end(12);
                bx.prepend(&entry_group);
            }
        }
    }

    {
        let window = window.clone();
        apply.connect_clicked(move |_| {
            let mut chosen: Vec<String> = checked_ids(&list.rows)
                .into_iter()
                .map(|id| format!("~{}", id))
                .collect();
            for token in custom_row.text().split(',') {
                let token = token.trim();
                if !token.is_empty() {
                    chosen.push(format!("~{}", token));
                }
            }
            on_apply(chosen);
            window.close();
        });
    }
    window.present();
}

// ---------------------------------------------------------------------------
// Simple string list editor (paths / extra arguments)
// ---------------------------------------------------------------------------

fn prompt_string(app: &Rc<App>, title: &str, label: &str, on_add: Rc<dyn Fn(String)>) {
    let window = adw::Window::builder()
        .transient_for(&app.window)
        .modal(true)
        .title(title)
        .default_width(460)
        .default_height(220)
        .build();
    let header = adw::HeaderBar::new();
    let add = gtk::Button::with_label("Add");
    add.add_css_class("suggested-action");
    header.pack_end(&add);
    let cancel = gtk::Button::with_label("Cancel");
    header.pack_start(&cancel);
    {
        let window = window.clone();
        cancel.connect_clicked(move |_| window.close());
    }
    let group = widgets::group(title, None);
    let entry = adw::EntryRow::builder().title(label).build();
    group.add(&entry);
    let page = adw::PreferencesPage::new();
    page.add(&group);
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&page));
    window.set_content(Some(&toolbar));
    {
        let window = window.clone();
        let entry = entry.clone();
        let on_add = Rc::clone(&on_add);
        add.connect_clicked(move |_| {
            let value = entry.text().trim().to_string();
            if !value.is_empty() {
                on_add(value);
            }
            window.close();
        });
    }
    window.present();
}

#[allow(clippy::too_many_arguments)]
fn list_group(
    app: &Rc<App>,
    work: &Rc<RefCell<ContainerConfig>>,
    title: &str,
    description: Option<&str>,
    add_label: &str,
    prompt_label: &str,
    get: fn(&ContainerConfig) -> Vec<String>,
    set: fn(&mut ContainerConfig, Vec<String>),
) -> adw::PreferencesGroup {
    let group = widgets::group(title, description);
    let rows: Rc<RefCell<Vec<gtk::Widget>>> = Rc::new(RefCell::new(Vec::new()));
    let holder: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let app_owned = Rc::clone(app);
    let work_owned = Rc::clone(work);
    let add_label_owned = add_label.to_string();
    let prompt_label_owned = prompt_label.to_string();

    let add_row = adw::ActionRow::builder().title(add_label).build();
    let add_button = gtk::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text("Add")
        .build();
    add_button.add_css_class("flat");
    add_row.add_suffix(&add_button);

    let refresh: Rc<dyn Fn()> = {
        let group = group.clone();
        let rows = Rc::clone(&rows);
        let work = Rc::clone(&work_owned);
        let holder = Rc::clone(&holder);
        let add_row = add_row.clone();
        let app = Rc::clone(&app_owned);
        Rc::new(move || {
            for row in rows.borrow().iter() {
                group.remove(row);
            }
            rows.borrow_mut().clear();
            let items = get(&work.borrow());
            for (index, item) in items.iter().enumerate() {
                let row = adw::ActionRow::builder().title(item.as_str()).build();
                let remove = gtk::Button::builder()
                    .icon_name("list-remove-symbolic")
                    .tooltip_text("Remove")
                    .build();
                remove.add_css_class("flat");
                row.add_suffix(&remove);
                {
                    let app = Rc::clone(&app);
                    let work = Rc::clone(&work);
                    let holder = Rc::clone(&holder);
                    remove.connect_clicked(move |_| {
                        let snapshot = {
                            let mut cfg = work.borrow_mut();
                            let mut list = get(&cfg);
                            if index < list.len() {
                                list.remove(index);
                            }
                            set(&mut cfg, list);
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
        })
    };
    *holder.borrow_mut() = Some(Rc::clone(&refresh));
    refresh();

    {
        let app = Rc::clone(&app_owned);
        let work = Rc::clone(&work_owned);
        let holder = Rc::clone(&holder);
        let app_for_dialog = Rc::clone(&app_owned);
        let add_label_string = add_label_owned.clone();
        let prompt_label_string = prompt_label_owned.clone();
        add_button.connect_clicked(move |_| {
            let work2 = Rc::clone(&work);
            let holder2 = Rc::clone(&holder);
            let app2 = Rc::clone(&app);
            prompt_string(
                &app_for_dialog,
                &add_label_string,
                &prompt_label_string,
                Rc::new(move |value| {
                    let snapshot = {
                        let mut cfg = work2.borrow_mut();
                        let mut list = get(&cfg);
                        list.push(value);
                        set(&mut cfg, list);
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

    group
}

