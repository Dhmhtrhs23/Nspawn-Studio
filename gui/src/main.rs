mod create;
mod detail;
mod sandbox;
mod state;
mod task;
mod users;
mod widgets;

use libadwaita as adw;
use adw::prelude::*;

/// Headless helper: `nspawn-studio --render <name>` regenerates the launcher
/// script for a container and prints its path. Useful for scripting and for
/// verifying the generator without a display.
fn render(name: &str) -> adw::glib::ExitCode {
    let store = nspawn_studio_core::store::Store::new();
    match store.load(name).and_then(|cfg| store.write_script(&cfg)) {
        Ok(path) => {
            println!("{}", path.display());
            adw::glib::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("nspawn-studio: {}", e);
            adw::glib::ExitCode::FAILURE
        }
    }
}

fn main() -> adw::glib::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 2 && (args[1] == "--version" || args[1] == "-V") {
        println!(
            "nspawn-studio {} ({})",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::ARCH
        );
        return adw::glib::ExitCode::SUCCESS;
    }
    if args.len() == 3 && args[1] == "--render" {
        return render(&args[2]);
    }
    let app = adw::Application::builder()
        .application_id("io.github.nspawnstudio.NspawnStudio")
        .flags(adw::gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    app.connect_activate(|app| {
        state::App::build(app);
    });
    app.run()
}
