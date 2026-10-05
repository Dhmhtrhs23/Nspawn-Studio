//! A small modal dialog that runs an external command and streams its
//! combined output into a text view.

use crate::state::App;
use gtk4 as gtk;
use libadwaita as adw;
use adw::prelude::*;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;

/// Run program/args and show a progress dialog. on_success is invoked on the
/// main thread when the command exits with status 0.
pub fn run_command_dialog<F>(
    app: &Rc<App>,
    title: &str,
    program: &str,
    args: Vec<String>,
    on_success: F,
) where
    F: Fn(&Rc<App>) + 'static,
{
    let window = adw::Window::builder()
        .transient_for(&app.window)
        .modal(true)
        .title(title)
        .default_width(780)
        .default_height(540)
        .build();

    let header = adw::HeaderBar::new();
    let spinner = gtk::Spinner::new();
    header.pack_end(&spinner);
    let close_button = gtk::Button::with_label("Close");
    header.pack_start(&close_button);
    {
        let window = window.clone();
        close_button.connect_clicked(move |_| window.close());
    }

    let status = gtk::Label::new(Some(&format!("Running {} ...", program)));
    status.set_xalign(0.0);
    status.set_margin_top(6);
    status.set_margin_bottom(6);
    status.set_margin_start(12);
    status.set_margin_end(12);
    status.add_css_class("dim-label");

    let view = gtk::TextView::builder()
        .editable(false)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::WordChar)
        .build();
    view.buffer()
        .set_text(&format!("$ {} {}\n\n", program, args.join(" ")));
    let scrolled = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .child(&view)
        .build();

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&status);
    content.append(&scrolled);
    toolbar.set_content(Some(&content));
    window.set_content(Some(&toolbar));

    let output: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let exit: Arc<Mutex<Option<i32>>> = Arc::new(Mutex::new(None));

    {
        let output = output.clone();
        let exit = exit.clone();
        let program = program.to_string();
        thread::spawn(move || {
            let mut child = match Command::new(&program)
                .args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(child) => child,
                Err(e) => {
                    output
                        .lock()
                        .unwrap()
                        .push_str(&format!("could not execute {}: {}\n", program, e));
                    *exit.lock().unwrap() = Some(127);
                    return;
                }
            };
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();
            let mut handles = Vec::new();
            if let Some(stdout) = stdout {
                let output = output.clone();
                handles.push(thread::spawn(move || {
                    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                        let mut out = output.lock().unwrap();
                        out.push_str(&line);
                        out.push('\n');
                    }
                }));
            }
            if let Some(stderr) = stderr {
                let output = output.clone();
                handles.push(thread::spawn(move || {
                    for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                        let mut out = output.lock().unwrap();
                        out.push_str(&line);
                        out.push('\n');
                    }
                }));
            }
            for handle in handles {
                let _ = handle.join();
            }
            let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
            *exit.lock().unwrap() = Some(code);
        });
    }

    window.present();
    spinner.start();

    let seen = Rc::new(std::cell::Cell::new(0usize));
    let on_success = Rc::new(on_success);
    let finished = Rc::new(std::cell::Cell::new(false));
    let spinner2 = spinner.clone();
    let buffer2 = view.buffer();
    let view2 = view.clone();
    let title_owned = title.to_string();
    let app2 = app.clone();
    let status2 = status.clone();
    let window2 = window.clone();
    let exit2 = exit.clone();

    adw::glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
        let text = output.lock().unwrap().clone();
        let previous = seen.get();
        if text.len() > previous {
            buffer2.set_text(&text);
            let mut end = buffer2.end_iter();
            view2.scroll_to_iter(&mut end, 0.0, false, 0.0, 0.0);
            seen.set(text.len());
        }
        if let Some(code) = *exit2.lock().unwrap() {
            if !finished.get() {
                finished.set(true);
                spinner2.stop();
                if code == 0 {
                    status2.set_text("Finished successfully");
                    on_success(&app2);
                } else {
                    let message = format!("{} failed (exit {})", title_owned, code);
                    status2.set_text(&message);
                    window2.set_title(Some(message.as_str()));
                }
            }
            return adw::glib::ControlFlow::Break;
        }
        adw::glib::ControlFlow::Continue
    });
}
