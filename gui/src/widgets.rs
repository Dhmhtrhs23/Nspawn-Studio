//! Small reusable GTK widgets and helpers.

use gtk4 as gtk;
use libadwaita as adw;
use adw::prelude::*;
use nspawn_studio_core::command::MachineState;

/// Apply the right Adwaita style class for a machine state.
pub fn set_dot(dot: &gtk::Label, state: MachineState) {
    for class in ["success", "error", "dim-label", "warning"] {
        dot.remove_css_class(class);
    }
    dot.set_text("●");
    match state {
        MachineState::Running => dot.add_css_class("success"),
        MachineState::Failed => dot.add_css_class("error"),
        MachineState::Stopped => dot.add_css_class("dim-label"),
        MachineState::Unknown => dot.add_css_class("warning"),
    }
}

/// One sidebar row: status dot, container name, state subtitle.
pub fn sidebar_row(name: &str) -> (gtk::ListBoxRow, gtk::Label, gtk::Label) {
    let dot = gtk::Label::new(Some("●"));
    dot.add_css_class("dim-label");

    let title = gtk::Label::new(Some(name));
    title.set_xalign(0.0);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);

    let subtitle = gtk::Label::new(Some("stopped"));
    subtitle.set_xalign(0.0);
    subtitle.add_css_class("dim-label");
    subtitle.add_css_class("caption");
    subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);

    let text = gtk::Box::new(gtk::Orientation::Vertical, 0);
    text.append(&title);
    text.append(&subtitle);

    let bx = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    bx.set_margin_top(8);
    bx.set_margin_bottom(8);
    bx.set_margin_start(6);
    bx.set_margin_end(6);
    bx.append(&dot);
    bx.append(&text);

    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&bx));
    (row, dot, subtitle)
}

/// A preferences group with a title.
pub fn group(title: &str, description: Option<&str>) -> adw::PreferencesGroup {
    let g = adw::PreferencesGroup::builder().title(title).build();
    if let Some(text) = description {
        g.set_description(Some(text));
    }
    g
}


/// A monospaced, read-only text view inside a scrolled window.
pub fn code_view(text: &str) -> gtk::ScrolledWindow {
    let view = gtk::TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::None)
        .build();
    view.buffer().set_text(text);
    let scrolled = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .child(&view)
        .build();
    scrolled
}

/// Parse a comma separated list, trimming empty entries.
pub fn parse_list(text: &str) -> Vec<String> {
    text.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Join a list for display in an entry row.
pub fn join_list(values: &[String]) -> String {
    values.join(", ")
}
