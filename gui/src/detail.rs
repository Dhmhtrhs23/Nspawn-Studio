//! The container detail / editor page.

use crate::state::App;
use crate::task;
use crate::widgets;
use gtk4 as gtk;
use libadwaita as adw;
use adw::prelude::*;
use nspawn_studio_core::command;
use nspawn_studio_core::generator;
use nspawn_studio_core::model::{
    BindMount, BootMode, ConsoleMode, ContainerConfig, NetworkMode, PortForward, ResolvConfMode,
    TimezoneMode, X11Access,
};
use nspawn_studio_core::validate;
use std::cell::RefCell;
use std::rc::Rc;

fn combo_row(title: &str, options: &[&str], selected: u32) -> adw::ComboRow {
    let model = gtk::StringList::new(options);
    let row = adw::ComboRow::builder().title(title).build();
    row.set_model(Some(&model));
    row.set_selected(selected);
    row
}

fn switch_row(title: &str, subtitle: &str, active: bool) -> adw::SwitchRow {
    let row = adw::SwitchRow::builder().title(title).subtitle(subtitle).build();
    row.set_active(active);
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

fn text_window(app: &Rc<App>, title: &str, text: &str, copy_label: &str) {
    let window = adw::Window::builder()
        .transient_for(&app.window)
        .modal(true)
        .title(title)
        .default_width(860)
        .default_height(620)
        .build();

    let header = adw::HeaderBar::new();
    let copy = gtk::Button::builder()
        .icon_name("edit-copy-symbolic")
        .tooltip_text(copy_label)
        .build();
    header.pack_end(&copy);
    {
        let text = text.to_string();
        let window = window.clone();
        let app = app.clone();
        copy.connect_clicked(move |_| {
            app.window.clipboard().set_text(&text);
            app.notify("Copied to clipboard");
            window.close();
        });
    }

    let view = widgets::code_view(text);
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&view));
    window.set_content(Some(&toolbar));
    window.present();
}

pub fn build(app: &Rc<App>, cfg: ContainerConfig) -> gtk::Widget {
    let work = Rc::new(RefCell::new(cfg));

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // ---------------------------------------------------------------- action bar
    let status_dot = gtk::Label::new(Some("●"));
    status_dot.add_css_class("dim-label");
    let status_text = gtk::Label::new(Some("unknown"));
    status_text.add_css_class("dim-label");

    let name_label = gtk::Label::new(Some(&work.borrow().name));
    name_label.add_css_class("title-4");

    let start_button = gtk::Button::builder()
        .icon_name("media-playback-start-symbolic")
        .tooltip_text("Start the container in a terminal")
        .build();
    start_button.add_css_class("suggested-action");
    let service_button = gtk::Button::builder()
        .icon_name("system-run-symbolic")
        .tooltip_text("Start the container in the background (systemd service)")
        .build();
    let stop_button = gtk::Button::builder()
        .icon_name("media-playback-stop-symbolic")
        .tooltip_text("Stop the container")
        .build();
    let logs_button = gtk::Button::builder()
        .icon_name("document-open-recent-symbolic")
        .tooltip_text("Show logs")
        .build();
    let script_button = gtk::Button::builder()
        .icon_name("text-x-script-symbolic")
        .tooltip_text("View the generated launcher script")
        .build();
    let regen_button = gtk::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Regenerate the launcher script")
        .build();
    let delete_button = gtk::Button::builder()
        .icon_name("user-trash-symbolic")
        .tooltip_text("Delete this container")
        .build();
    delete_button.add_css_class("destructive-action");

    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    bar.set_margin_top(6);
    bar.set_margin_bottom(6);
    bar.set_margin_start(12);
    bar.set_margin_end(12);
    bar.append(&name_label);
    bar.append(&status_dot);
    bar.append(&status_text);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    bar.append(&spacer);
    bar.append(&logs_button);
    bar.append(&script_button);
    bar.append(&regen_button);
    bar.append(&stop_button);
    bar.append(&service_button);
    bar.append(&start_button);
    bar.append(&delete_button);

    // ---------------------------------------------------------------- page
    let page = adw::PreferencesPage::new();
    page.set_vexpand(true);

    // General
    let general = widgets::group("General", None);
    let machine_row = adw::EntryRow::builder().title("Machine name").build();
    machine_row.set_text(work.borrow().machine_name());
    {
        let app = app.clone();
        let work = work.clone();
        machine_row.connect_changed(move |row| {
            let value = row.text().to_string();
            edit(&app, &work, move |cfg| cfg.machine_name = value);
        });
    }
    general.add(&machine_row);

    let rootfs_row = adw::EntryRow::builder().title("Root filesystem").build();
    rootfs_row.set_text(&work.borrow().rootfs.to_string_lossy());
    {
        let app = app.clone();
        let work = work.clone();
        rootfs_row.connect_changed(move |row| {
            let value = row.text().to_string();
            edit(&app, &work, move |cfg| cfg.rootfs = value.into());
        });
    }
    let folder_button = gtk::Button::builder()
        .icon_name("folder-open-symbolic")
        .tooltip_text("Choose a directory")
        .build();
    rootfs_row.add_suffix(&folder_button);
    {
        let app = app.clone();
        let work = work.clone();
        let rootfs_row = rootfs_row.clone();
        folder_button.connect_clicked(move |_| {
            let dialog = gtk::FileDialog::builder()
                .title("Select the container root filesystem")
                .build();
            let app = app.clone();
            let work = work.clone();
            let row = rootfs_row.clone();
            let parent = app.window.clone();
            dialog.select_folder(Some(&parent), None::<&gtk::gio::Cancellable>, move |res| {
                if let Ok(file) = res {
                    if let Some(path) = file.path() {
                        row.set_text(&path.to_string_lossy());
                        edit(&app, &work, move |cfg| cfg.rootfs = path);
                    }
                }
            });
        });
    }
    general.add(&rootfs_row);

    let boot_row = combo_row(
        "Start mode",
        &["Boot the container init (systemd)", "Run a command"],
        if work.borrow().boot == BootMode::Boot { 0 } else { 1 },
    );
    general.add(&boot_row);

    let command_row = adw::EntryRow::builder().title("Command").build();
    command_row.set_text(&work.borrow().command);
    {
        let app = app.clone();
        let work = work.clone();
        command_row.connect_changed(move |row| {
            let value = row.text().to_string();
            edit(&app, &work, move |cfg| cfg.command = value);
        });
    }
    general.add(&command_row);
    command_row.set_visible(work.borrow().boot == BootMode::Command);

    let hostname_row = adw::EntryRow::builder().title("Hostname (optional)").build();
    hostname_row.set_text(work.borrow().hostname.clone().unwrap_or_default().as_str());
    {
        let app = app.clone();
        let work = work.clone();
        hostname_row.connect_changed(move |row| {
            let value = row.text().to_string();
            edit(&app, &work, move |cfg| {
                cfg.hostname = if value.trim().is_empty() { None } else { Some(value) };
            });
        });
    }
    general.add(&hostname_row);

    let console_row = combo_row(
        "Console",
        &["Interactive", "Read-only", "Passive", "Pipe"],
        match work.borrow().console {
            ConsoleMode::Interactive => 0,
            ConsoleMode::ReadOnly => 1,
            ConsoleMode::Passive => 2,
            ConsoleMode::Pipe => 3,
        },
    );
    {
        let app = app.clone();
        let work = work.clone();
        console_row.connect_selected_notify(move |row| {
            let index = row.selected();
            edit(&app, &work, move |cfg| {
                cfg.console = match index {
                    1 => ConsoleMode::ReadOnly,
                    2 => ConsoleMode::Passive,
                    3 => ConsoleMode::Pipe,
                    _ => ConsoleMode::Interactive,
                };
            });
        });
    }
    general.add(&console_row);

    {
        // boot row toggles command row visibility and value
        let command_row = command_row.clone();
        let app = app.clone();
        let work = work.clone();
        boot_row.connect_selected_notify(move |row| {
            let index = row.selected();
            command_row.set_visible(index == 1);
            edit(&app, &work, move |cfg| {
                cfg.boot = if index == 1 { BootMode::Command } else { BootMode::Boot };
            });
        });
    }

    let resolv_row = combo_row(
        "resolv.conf",
        &["auto", "off", "copy-host", "copy-static", "delete"],
        match work.borrow().resolv_conf {
            ResolvConfMode::Auto => 0,
            ResolvConfMode::Off => 1,
            ResolvConfMode::CopyHost => 2,
            ResolvConfMode::CopyStatic => 3,
            ResolvConfMode::Delete => 4,
        },
    );
    {
        let app = app.clone();
        let work = work.clone();
        resolv_row.connect_selected_notify(move |row| {
            let index = row.selected();
            edit(&app, &work, move |cfg| {
                cfg.resolv_conf = match index {
                    0 => ResolvConfMode::Auto,
                    1 => ResolvConfMode::Off,
                    3 => ResolvConfMode::CopyStatic,
                    4 => ResolvConfMode::Delete,
                    _ => ResolvConfMode::CopyHost,
                };
            });
        });
    }
    general.add(&resolv_row);

    let timezone_row = combo_row(
        "Timezone",
        &["copy", "off", "bind", "symlink"],
        match work.borrow().timezone {
            TimezoneMode::Copy => 0,
            TimezoneMode::Off => 1,
            TimezoneMode::Bind => 2,
            TimezoneMode::Symlink => 3,
        },
    );
    {
        let app = app.clone();
        let work = work.clone();
        timezone_row.connect_selected_notify(move |row| {
            let index = row.selected();
            edit(&app, &work, move |cfg| {
                cfg.timezone = match index {
                    1 => TimezoneMode::Off,
                    2 => TimezoneMode::Bind,
                    3 => TimezoneMode::Symlink,
                    _ => TimezoneMode::Copy,
                };
            });
        });
    }
    general.add(&timezone_row);
    page.add(&general);

    // Display
    let display = widgets::group("Display", None);
    let x11_row = combo_row(
        "X11 access",
        &[
            "None",
            "Sockets only (needs xhost)",
            "Sockets and xauth cookie (recommended)",
            "Xephyr (isolated nested X server)",
        ],
        match work.borrow().x11 {
            X11Access::None => 0,
            X11Access::Socket => 1,
            X11Access::Authority => 2,
            X11Access::Xephyr => 3,
        },
    );
    let xephyr_row = adw::EntryRow::builder().title("Xephyr screen size").build();
    xephyr_row.set_text(&work.borrow().xephyr_screen);
    xephyr_row.set_visible(work.borrow().x11 == X11Access::Xephyr);
    {
        let app = app.clone();
        let work = work.clone();
        xephyr_row.connect_changed(move |row| {
            let value = row.text().to_string();
            edit(&app, &work, move |cfg| cfg.xephyr_screen = value);
        });
    }
    {
        let app = app.clone();
        let work = work.clone();
        let xephyr_row = xephyr_row.clone();
        x11_row.connect_selected_notify(move |row| {
            let index = row.selected();
            xephyr_row.set_visible(index == 3);
            edit(&app, &work, move |cfg| {
                cfg.x11 = match index {
                    1 => X11Access::Socket,
                    2 => X11Access::Authority,
                    3 => X11Access::Xephyr,
                    _ => X11Access::None,
                };
            });
        });
    }
    display.add(&x11_row);
    display.add(&xephyr_row);

    let xhost_row = switch_row(
        "Run xhost on start",
        "Allow the container user to connect (xhost +si:localuser:root)",
        work.borrow().xhost_local,
    );
    {
        let app = app.clone();
        let work = work.clone();
        xhost_row.connect_active_notify(move |row| {
            let active = row.is_active();
            edit(&app, &work, move |cfg| cfg.xhost_local = active);
        });
    }
    display.add(&xhost_row);

    let wayland_row = switch_row(
        "Wayland socket",
        "Share WAYLAND_DISPLAY and the Wayland socket",
        work.borrow().wayland,
    );
    {
        let app = app.clone();
        let work = work.clone();
        wayland_row.connect_active_notify(move |row| {
            let active = row.is_active();
            edit(&app, &work, move |cfg| cfg.wayland = active);
        });
    }
    display.add(&wayland_row);

    let gpu_row = switch_row(
        "GPU",
        "Bind /dev/dri for hardware acceleration",
        work.borrow().gpu,
    );
    {
        let app = app.clone();
        let work = work.clone();
        gpu_row.connect_active_notify(move |row| {
            let active = row.is_active();
            edit(&app, &work, move |cfg| cfg.gpu = active);
        });
    }
    display.add(&gpu_row);
    page.add(&display);

    // Audio
    let audio = widgets::group("Audio", None);
    let pipewire_row = switch_row("PipeWire", "Share the PipeWire socket", work.borrow().audio.pipewire);
    let pulse_row = switch_row("PulseAudio", "Share the PulseAudio native socket", work.borrow().audio.pulseaudio);
    let alsa_row = switch_row("ALSA", "Bind /dev/snd", work.borrow().audio.alsa);
    {
        let app = app.clone();
        let work = work.clone();
        pipewire_row.connect_active_notify(move |row| {
            let active = row.is_active();
            edit(&app, &work, move |cfg| cfg.audio.pipewire = active);
        });
    }
    {
        let app = app.clone();
        let work = work.clone();
        pulse_row.connect_active_notify(move |row| {
            let active = row.is_active();
            edit(&app, &work, move |cfg| cfg.audio.pulseaudio = active);
        });
    }
    {
        let app = app.clone();
        let work = work.clone();
        alsa_row.connect_active_notify(move |row| {
            let active = row.is_active();
            edit(&app, &work, move |cfg| cfg.audio.alsa = active);
        });
    }
    audio.add(&pipewire_row);
    audio.add(&pulse_row);
    audio.add(&alsa_row);
    page.add(&audio);

    // Mounts
    let mounts_group = widgets::group(
        "Bind mounts",
        Some("Expose host folders inside the container"),
    );
    let mount_rows: Rc<RefCell<Vec<gtk::Widget>>> = Rc::new(RefCell::new(Vec::new()));
    let mount_refresh_holder: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let refresh_mounts: Rc<dyn Fn()> = {
        let app = app.clone();
        let work = work.clone();
        let group = mounts_group.clone();
        let rows = mount_rows.clone();
        let holder = mount_refresh_holder.clone();
        Rc::new(move || {
            for row in rows.borrow().iter() {
                group.remove(row);
            }
            rows.borrow_mut().clear();
            let cfg = work.borrow().clone();
            for (index, mount) in cfg.mounts.iter().enumerate() {
                let row = adw::ActionRow::builder()
                    .title(mount.host.as_str())
                    .subtitle(format!(
                        "-> {} ({})",
                        mount.container,
                        if mount.read_only { "read-only" } else { "read-write" }
                    ))
                    .build();
                let remove = gtk::Button::builder()
                    .icon_name("list-remove-symbolic")
                    .tooltip_text("Remove this mount")
                    .build();
                remove.add_css_class("flat");
                row.add_suffix(&remove);
                {
                    let app = app.clone();
                    let work = work.clone();
                    let holder = holder.clone();
                    remove.connect_clicked(move |_| {
                        let snapshot = {
                            let mut c = work.borrow_mut();
                            if index < c.mounts.len() {
                                c.mounts.remove(index);
                            }
                            c.touch();
                            c.clone()
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
        })
    };
    *mount_refresh_holder.borrow_mut() = Some(refresh_mounts.clone());
    refresh_mounts();

    let add_mount_row = adw::ActionRow::builder()
        .title("Add a bind mount")
        .build();
    let add_mount_button = gtk::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text("Add")
        .build();
    add_mount_button.add_css_class("flat");
    add_mount_row.add_suffix(&add_mount_button);
    mounts_group.add(&add_mount_row);
    {
        let app = app.clone();
        let work = work.clone();
        let refresh = refresh_mounts.clone();
        add_mount_button.connect_clicked(move |_| {
            present_mount_dialog(&app, &work, refresh.clone());
        });
    }
    page.add(&mounts_group);

    // Network
    let network = widgets::group("Network", None);
    let mode_row = combo_row(
        "Mode",
        &["Host network", "Private (loopback only)", "Veth to host"],
        match work.borrow().network.mode {
            NetworkMode::Host => 0,
            NetworkMode::Private => 1,
            NetworkMode::Veth => 2,
        },
    );
    network.add(&mode_row);
    let bridge_row = adw::EntryRow::builder().title("Bridge interface").build();
    bridge_row.set_text(work.borrow().network.bridge.clone().unwrap_or_default().as_str());
    bridge_row.set_visible(work.borrow().network.mode == NetworkMode::Veth);
    {
        let app = app.clone();
        let work = work.clone();
        bridge_row.connect_changed(move |row| {
            let value = row.text().to_string();
            edit(&app, &work, move |cfg| {
                cfg.network.bridge = if value.trim().is_empty() { None } else { Some(value) };
            });
        });
    }
    network.add(&bridge_row);
    {
        let app = app.clone();
        let work = work.clone();
        let bridge_row = bridge_row.clone();
        mode_row.connect_selected_notify(move |row| {
            let index = row.selected();
            bridge_row.set_visible(index == 2);
            edit(&app, &work, move |cfg| {
                cfg.network.mode = match index {
                    1 => NetworkMode::Private,
                    2 => NetworkMode::Veth,
                    _ => NetworkMode::Host,
                };
            });
        });
    }
    let forward_rows: Rc<RefCell<Vec<gtk::Widget>>> = Rc::new(RefCell::new(Vec::new()));
    let forward_refresh_holder: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let refresh_forwards: Rc<dyn Fn()> = {
        let app = app.clone();
        let work = work.clone();
        let group = network.clone();
        let rows = forward_rows.clone();
        let holder = forward_refresh_holder.clone();
        Rc::new(move || {
            for row in rows.borrow().iter() {
                group.remove(row);
            }
            rows.borrow_mut().clear();
            let cfg = work.borrow().clone();
            for (index, fwd) in cfg.network.port_forwards.iter().enumerate() {
                let row = adw::ActionRow::builder()
                    .title(format!("{}:{}", fwd.host_port, fwd.container_port))
                    .subtitle(format!("{} port forward", fwd.protocol))
                    .build();
                let remove = gtk::Button::builder()
                    .icon_name("list-remove-symbolic")
                    .tooltip_text("Remove")
                    .build();
                remove.add_css_class("flat");
                row.add_suffix(&remove);
                {
                    let app = app.clone();
                    let work = work.clone();
                    let holder = holder.clone();
                    remove.connect_clicked(move |_| {
                        let snapshot = {
                            let mut c = work.borrow_mut();
                            if index < c.network.port_forwards.len() {
                                c.network.port_forwards.remove(index);
                            }
                            c.touch();
                            c.clone()
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
        })
    };
    *forward_refresh_holder.borrow_mut() = Some(refresh_forwards.clone());
    refresh_forwards();
    let add_forward_row = adw::ActionRow::builder().title("Add a port forward").build();
    let add_forward_button = gtk::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text("Add")
        .build();
    add_forward_button.add_css_class("flat");
    add_forward_row.add_suffix(&add_forward_button);
    network.add(&add_forward_row);
    {
        let app = app.clone();
        let work = work.clone();
        let refresh = refresh_forwards.clone();
        add_forward_button.connect_clicked(move |_| {
            present_forward_dialog(&app, &work, refresh.clone());
        });
    }
    page.add(&network);

    crate::sandbox::install(app, &page, &work);

    let users_group = crate::users::build_group(app, &work);
    page.add(&users_group);

    // Debootstrap
    if work.borrow().source == nspawn_studio_core::model::SourceKind::Debootstrap {
        let deb = widgets::group("Debootstrap", Some("Used to build the root filesystem"));
        let spec = work.borrow().debootstrap.clone();
        let suite_row = adw::EntryRow::builder().title("Suite").build();
        suite_row.set_text(&spec.suite);
        let mirror_row = adw::EntryRow::builder().title("Mirror").build();
        mirror_row.set_text(&spec.mirror);
        let arch_row = adw::EntryRow::builder().title("Architecture").build();
        arch_row.set_text(&spec.arch);
        let variant_row = adw::EntryRow::builder().title("Variant").build();
        variant_row.set_text(&spec.variant);
        let include_row = adw::EntryRow::builder().title("Packages (comma separated)").build();
        include_row.set_text(&widgets::join_list(&spec.include));
        {
            let app = app.clone();
            let work = work.clone();
            suite_row.connect_changed(move |row| {
                let value = row.text().to_string();
                edit(&app, &work, move |cfg| cfg.debootstrap.suite = value);
            });
        }
        {
            let app = app.clone();
            let work = work.clone();
            mirror_row.connect_changed(move |row| {
                let value = row.text().to_string();
                edit(&app, &work, move |cfg| cfg.debootstrap.mirror = value);
            });
        }
        {
            let app = app.clone();
            let work = work.clone();
            arch_row.connect_changed(move |row| {
                let value = row.text().to_string();
                edit(&app, &work, move |cfg| cfg.debootstrap.arch = value);
            });
        }
        {
            let app = app.clone();
            let work = work.clone();
            variant_row.connect_changed(move |row| {
                let value = row.text().to_string();
                edit(&app, &work, move |cfg| cfg.debootstrap.variant = value);
            });
        }
        {
            let app = app.clone();
            let work = work.clone();
            include_row.connect_changed(move |row| {
                let value = widgets::parse_list(&row.text());
                edit(&app, &work, move |cfg| cfg.debootstrap.include = value);
            });
        }
        deb.add(&suite_row);
        deb.add(&mirror_row);
        deb.add(&arch_row);
        deb.add(&variant_row);
        deb.add(&include_row);

        let rebuild_row = adw::ActionRow::builder()
            .title("Rebuild the root filesystem")
            .subtitle("Runs debootstrap and overwrites the directory contents")
            .build();
        let rebuild_button = gtk::Button::with_label("Rebuild");
        rebuild_button.add_css_class("flat");
        rebuild_row.add_suffix(&rebuild_button);
        {
            let app = app.clone();
            let work = work.clone();
            rebuild_button.connect_clicked(move |_| {
                let cfg = work.borrow().clone();
                let errors = validate::validate_config(&cfg);
                if !errors.is_empty() {
                    app.notify(&errors.join("; "));
                    return;
                }
                let argv = generator::debootstrap_argv(&cfg);
                let program = argv[0].clone();
                let args = argv[1..].to_vec();
                task::run_command_dialog(&app, "Rebuilding root filesystem", &program, args, |app| {
                    app.notify("Root filesystem rebuilt");
                });
            });
        }
        deb.add(&rebuild_row);
        page.add(&deb);
    }

    // ---------------------------------------------------------------- actions
    let update_status = {
        let work = work.clone();
        let dot = status_dot.clone();
        let text = status_text.clone();
        let start = start_button.clone();
        let service = service_button.clone();
        let stop = stop_button.clone();
        move || {
            let cfg = work.borrow();
            let state = command::container_state(cfg.machine_name(), &cfg.transient_unit());
            drop(cfg);
            widgets::set_dot(&dot, state);
            text.set_text(state.label());
            start.set_sensitive(!state.is_running());
            service.set_sensitive(!state.is_running());
            stop.set_sensitive(state.is_running());
        }
    };
    update_status();
    app.set_detail_refresher(Box::new(update_status));

    {
        let app = app.clone();
        let work = work.clone();
        start_button.connect_clicked(move |_| {
            let cfg = work.borrow().clone();
            let errors = validate::validate_config(&cfg);
            if !errors.is_empty() {
                app.notify(&errors.join("; "));
                return;
            }
            let script = match app.store.write_script(&cfg) {
                Ok(path) => path,
                Err(e) => {
                    app.notify(&format!("Could not write launcher: {}", e));
                    return;
                }
            };
            let script_str = script.to_string_lossy().to_string();
            match command::terminal_command(&script_str) {
                Some(argv) => {
                    let program = argv[0].clone();
                    let args: Vec<&str> = argv[1..].iter().map(|s| s.as_str()).collect();
                    match command::spawn_detached(&program, &args) {
                        Ok(_) => app.notify(&format!("Starting {} in a terminal", cfg.name)),
                        Err(e) => app.notify(&format!("Could not start {}: {}", program, e)),
                    }
                }
                None => {
                    app.notify("No terminal emulator found; starting in the background instead");
                    match command::start_service(&cfg.transient_unit(), &script_str) {
                        Ok(_) => app.refresh_states(),
                        Err(e) => app.notify(&e),
                    }
                }
            }
        });
    }
    {
        let app = app.clone();
        let work = work.clone();
        stop_button.connect_clicked(move |_| {
            let cfg = work.borrow().clone();
            let unit = cfg.transient_unit();
            if let Err(e) = command::stop_service(&unit) {
                app.notify(&e);
                let _ = command::terminate_machine(cfg.machine_name());
            } else {
                app.notify(&format!("Stopped {}", cfg.name));
            }
            app.refresh_states();
        });
    }
    {
        let app = app.clone();
        let work = work.clone();
        service_button.connect_clicked(move |_| {
            let cfg = work.borrow().clone();
            let errors = validate::validate_config(&cfg);
            if !errors.is_empty() {
                app.notify(&errors.join("; "));
                return;
            }
            let script = match app.store.write_script(&cfg) {
                Ok(path) => path,
                Err(e) => {
                    app.notify(&format!("Could not write launcher: {}", e));
                    return;
                }
            };
            match command::start_service(&cfg.transient_unit(), &script.to_string_lossy()) {
                Ok(_) => {
                    app.notify(&format!("Starting {} in the background", cfg.name));
                    app.refresh_states();
                }
                Err(e) => app.notify(&e),
            }
        });
    }
    {
        let app = app.clone();
        let work = work.clone();
        logs_button.connect_clicked(move |_| {
            let unit = work.borrow().service_unit();
            let text = command::journal(&unit, 400);
            text_window(&app, "Container logs", &text, "Copy logs");
        });
    }
    {
        let app = app.clone();
        let work = work.clone();
        script_button.connect_clicked(move |_| {
            let text = generator::generate_script(&work.borrow());
            text_window(&app, "Launcher script", &text, "Copy script");
        });
    }
    {
        let app = app.clone();
        let work = work.clone();
        regen_button.connect_clicked(move |_| {
            let cfg = work.borrow().clone();
            match app.store.write_script(&cfg) {
                Ok(path) => app.notify(&format!("Wrote {}", path.display())),
                Err(e) => app.notify(&format!("Could not write launcher: {}", e)),
            }
        });
    }
    {
        let app = app.clone();
        let work = work.clone();
        delete_button.connect_clicked(move |_| {
            let name = work.borrow().name.clone();
            let dialog = adw::MessageDialog::builder()
                .transient_for(&app.window)
                .heading("Delete this container?")
                .body(format!(
                    "This removes the configuration and launcher script for '{}'. The root filesystem is kept unless you choose to delete it.",
                    name
                ))
                .build();
            dialog.add_response("cancel", "Cancel");
            dialog.add_response("config", "Delete configuration");
            dialog.add_response("all", "Delete everything");
            dialog.set_response_appearance("config", adw::ResponseAppearance::Destructive);
            dialog.set_response_appearance("all", adw::ResponseAppearance::Destructive);
            let app2 = app.clone();
            let name2 = name.clone();
            dialog.connect_response(None, move |_, response| match response {
                "config" => {
                    match app2.store.delete(&name2) {
                        Ok(()) => app2.notify("Container deleted"),
                        Err(e) => app2.notify(&format!("Could not delete: {}", e)),
                    }
                    *app2.selected.borrow_mut() = None;
                    app2.reload();
                }
                "all" => {
                    let _ = app2.store.delete(&name2);
                    match app2.store.remove_rootfs(&name2) {
                        Ok(()) => app2.notify("Container and root filesystem deleted"),
                        Err(e) => app2.notify(&format!("Could not delete rootfs: {}", e)),
                    }
                    *app2.selected.borrow_mut() = None;
                    app2.reload();
                }
                _ => {}
            });
            dialog.present();
        });
    }

    root.append(&bar);
    let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
    root.append(&separator);
    root.append(&page);
    root.upcast()
}

fn present_mount_dialog(
    app: &Rc<App>,
    work: &Rc<RefCell<ContainerConfig>>,
    refresh: Rc<dyn Fn()>,
) {
    let window = adw::Window::builder()
        .transient_for(&app.window)
        .modal(true)
        .title("Add a bind mount")
        .default_width(520)
        .default_height(340)
        .build();
    let header = adw::HeaderBar::new();
    let add = gtk::Button::with_label("Add");
    add.add_css_class("suggested-action");
    header.pack_end(&add);
    let page = adw::PreferencesPage::new();
    let group = widgets::group("Bind mount", None);
    let host_row = adw::EntryRow::builder().title("Host path").build();
    let container_row = adw::EntryRow::builder().title("Container path").build();
    let readonly_row = adw::SwitchRow::builder().title("Read-only").build();
    group.add(&host_row);
    group.add(&container_row);
    group.add(&readonly_row);
    page.add(&group);
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&page));
    window.set_content(Some(&toolbar));

    let presets = widgets::group("Presets", None);
    for (title, host, container, ro) in [
        ("Whole host filesystem", "/", "/mnt/host", true),
        ("Home directory", "~", "/mnt/home", true),
    ] {
        let row = adw::ActionRow::builder().title(title).subtitle(host).build();
        let use_button = gtk::Button::with_label("Use");
        use_button.add_css_class("flat");
        row.add_suffix(&use_button);
        let host_row2 = host_row.clone();
        let container_row2 = container_row.clone();
        let readonly_row2 = readonly_row.clone();
        use_button.connect_clicked(move |_| {
            host_row2.set_text(host);
            container_row2.set_text(container);
            readonly_row2.set_active(ro);
        });
        presets.add(&row);
    }
    page.add(&presets);

    {
        let app = app.clone();
        let work = work.clone();
        let window = window.clone();
        let refresh = refresh.clone();
        add.connect_clicked(move |_| {
            let host = host_row.text().to_string();
            let container = container_row.text().to_string();
            let read_only = readonly_row.is_active();
            if host.trim().is_empty() || container.trim().is_empty() {
                app.notify("Both paths are required");
                return;
            }
            let snapshot = {
                let mut cfg = work.borrow_mut();
                cfg.mounts.push(BindMount {
                    host,
                    container,
                    read_only,
                });
                cfg.touch();
                cfg.clone()
            };
            app.save_config(&snapshot, false);
            refresh();
            window.close();
        });
    }
    window.present();
}

fn present_forward_dialog(
    app: &Rc<App>,
    work: &Rc<RefCell<ContainerConfig>>,
    refresh: Rc<dyn Fn()>,
) {
    let window = adw::Window::builder()
        .transient_for(&app.window)
        .modal(true)
        .title("Add a port forward")
        .default_width(480)
        .default_height(300)
        .build();
    let header = adw::HeaderBar::new();
    let add = gtk::Button::with_label("Add");
    add.add_css_class("suggested-action");
    header.pack_end(&add);
    let page = adw::PreferencesPage::new();
    let group = widgets::group("Port forward", Some("Requires private or veth networking"));
    let proto_row = combo_row("Protocol", &["tcp", "udp"], 0);
    let host_row = adw::EntryRow::builder().title("Host port").build();
    let container_row = adw::EntryRow::builder().title("Container port").build();
    group.add(&proto_row);
    group.add(&host_row);
    group.add(&container_row);
    page.add(&group);
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&page));
    window.set_content(Some(&toolbar));

    {
        let app = app.clone();
        let work = work.clone();
        let window = window.clone();
        let refresh = refresh.clone();
        add.connect_clicked(move |_| {
            let protocol = if proto_row.selected() == 1 { "udp" } else { "tcp" }.to_string();
            let host_port: u16 = match host_row.text().parse() {
                Ok(value) => value,
                Err(_) => {
                    app.notify("Host port must be a number");
                    return;
                }
            };
            let container_port: u16 = match container_row.text().parse() {
                Ok(value) => value,
                Err(_) => {
                    app.notify("Container port must be a number");
                    return;
                }
            };
            let snapshot = {
                let mut cfg = work.borrow_mut();
                cfg.network.port_forwards.push(PortForward {
                    protocol,
                    host_port,
                    container_port,
                });
                cfg.touch();
                cfg.clone()
            };
            app.save_config(&snapshot, false);
            refresh();
            window.close();
        });
    }
    window.present();
}
