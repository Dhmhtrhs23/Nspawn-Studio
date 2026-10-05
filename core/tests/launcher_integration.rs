//! End-to-end tests for the generated launcher script.
//!
//! These tests execute the real bash launcher with a fake "systemd-nspawn"
//! on PATH. That way every runtime-conditional branch (X11, sockets, mounts,
//! hardening) is exercised exactly as it would be on a live system, without
//! needing to actually create a container.

use nspawn_studio_core::generator::generate_script;
use nspawn_studio_core::model::{
    AudioConfig, BindMount, BootMode, ContainerConfig, NetworkConfig, NetworkMode, PortForward,
    SecurityConfig, X11Access,
};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::Command;

fn unique_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("nspawn-studio-it-{}-{}", tag, nanos));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn is_root() -> bool {
    nspawn_studio_core::store::running_as_root()
}

fn write_script(dir: &Path, cfg: &ContainerConfig) -> PathBuf {
    let path = dir.join("launch.sh");
    fs::write(&path, generate_script(cfg)).unwrap();
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).unwrap();
    path
}

fn make_fake_nspawn(dir: &Path) -> PathBuf {
    let bin = dir.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let fake = bin.join("systemd-nspawn");
    fs::write(
        &fake,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$NSPAWN_CAPTURE\"\n",
    )
    .unwrap();
    let mut perms = fs::metadata(&fake).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake, perms).unwrap();
    bin
}

fn base_config(dir: &Path) -> ContainerConfig {
    let rootfs = dir.join("rootfs");
    fs::create_dir_all(&rootfs).unwrap();
    ContainerConfig::new("itest", rootfs)
}

#[test]
fn every_generated_script_is_valid_bash() {
    let dir = unique_dir("syntax");
    let mut variants: Vec<ContainerConfig> = Vec::new();
    variants.push(base_config(&dir));

    let mut command_mode = base_config(&dir);
    command_mode.boot = BootMode::Command;
    command_mode.command = "/bin/bash -l".into();
    variants.push(command_mode);

    let mut hardened = base_config(&dir);
    hardened.security = SecurityConfig::recommended();
    hardened.x11 = X11Access::Authority;
    hardened.audio = AudioConfig {
        pipewire: true,
        pulseaudio: true,
        alsa: true,
    };
    hardened.wayland = true;
    hardened.gpu = true;
    hardened.hostname = Some("box".into());
    hardened
        .mounts
        .push(BindMount::new("/srv/data", "/data", false));
    hardened.network = NetworkConfig {
        mode: NetworkMode::Veth,
        bridge: Some("br0".into()),
        port_forwards: vec![PortForward::new("tcp", 8080, 80)],
    };
    hardened.extra_args.push("--setenv=FOO=bar".into());
    variants.push(hardened);

    for (i, cfg) in variants.iter().enumerate() {
        let path = dir.join(format!("variant-{}.sh", i));
        fs::write(&path, generate_script(cfg)).unwrap();
        let out = Command::new("bash")
            .arg("-n")
            .arg(&path)
            .output()
            .expect("bash must be installed");
        assert!(
            out.status.success(),
            "variant {} failed bash -n: {}",
            i,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    fs::remove_dir_all(dir).ok();
}

#[test]
fn launcher_passes_hardened_flags_to_nspawn() {
    if !is_root() {
        eprintln!("skipping: requires root");
        return;
    }
    let dir = unique_dir("flags");
    let bin = make_fake_nspawn(&dir);
    let runtime = dir.join("run");
    fs::create_dir_all(&runtime).unwrap();
    // Fake sockets and xauth cookie so every runtime branch is taken.
    let _pipewire = UnixListener::bind(runtime.join("pipewire-0")).unwrap();
    let _manager = UnixListener::bind(runtime.join("pipewire-0-manager")).unwrap();
    fs::create_dir_all(runtime.join("pulse")).unwrap();
    let _pulse = UnixListener::bind(runtime.join("pulse/native")).unwrap();
    let _wayland = UnixListener::bind(runtime.join("wayland-0")).unwrap();
    let xauth = dir.join(".Xauthority");
    fs::write(&xauth, b"cookie").unwrap();
    let mount_host = dir.join("data");
    fs::create_dir_all(&mount_host).unwrap();

    let mut cfg = base_config(&dir);
    cfg.machine_name = "itest".into();
    cfg.hostname = Some("box".into());
    cfg.x11 = X11Access::Authority;
    cfg.wayland = true;
    cfg.audio = AudioConfig {
        pipewire: true,
        pulseaudio: true,
        alsa: false,
    };
    cfg.mounts
        .push(BindMount::new(mount_host.to_str().unwrap(), "/mnt/data", true));
    cfg.network = NetworkConfig {
        mode: NetworkMode::Private,
        bridge: None,
        port_forwards: vec![PortForward::new("tcp", 8080, 80)],
    };
    cfg.security = SecurityConfig::recommended();
    cfg.security.mask.push("/srv/secret".into());

    let script = write_script(&dir, &cfg);
    let capture = dir.join("captured.txt");

    let output = Command::new("bash")
        .arg(&script)
        .env("PATH", format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default()))
        .env("NSPAWN_CAPTURE", &capture)
        .env("NSPAWN_STUDIO_HOST_UID", "0")
        .env("XDG_RUNTIME_DIR", &runtime)
        .env("XAUTHORITY", &xauth)
        .env("DISPLAY", ":7")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "launcher failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let captured = fs::read_to_string(&capture).unwrap();
    let args: Vec<&str> = captured.lines().collect();
    let has = |needle: &str| args.iter().any(|a| a == &needle);
    let has_prefix = |needle: &str| args.iter().any(|a| a.starts_with(needle));

    assert!(has("--machine=itest"), "machine name: {:?}", args);
    assert!(has_prefix("--directory="), "rootfs directory: {:?}", args);
    assert!(has("--hostname=box"));
    assert!(has("-b"), "boot mode: {:?}", args);
    assert!(has("--console=interactive"));
    assert!(has("-U"), "private users");
    assert!(has("--inaccessible=/srv/secret"));
    assert!(has("--no-new-privileges=yes"));
    assert!(has("--drop-capability=CAP_SYS_MODULE"));
    assert!(has("--private-network"));
    assert!(has("--port=tcp:8080:80"));
    assert!(has_prefix("--bind-ro="));
    assert!(has_prefix("--bind-ro=") && args.iter().any(|a| a.contains("/mnt/data")));
    assert!(has("--setenv=DISPLAY=:7"));
    assert!(has_prefix("--bind-ro=") && args.iter().any(|a| a.contains("/root/.Xauthority")));
    assert!(args.iter().any(|a| a.contains("pipewire-0")));
    assert!(
        args.iter()
            .any(|a| a.starts_with("--setenv=PULSE_SERVER=unix:")),
        "pulse server: {:?}",
        args
    );
    assert!(
        args.iter().any(|a| a.contains("wayland-0")),
        "wayland: {:?}",
        args
    );
    assert!(has("--timezone=copy"));
    assert!(has("--resolv-conf=auto"));

    fs::remove_dir_all(dir).ok();
}

#[test]
fn command_mode_launcher_passes_command_after_double_dash() {
    if !is_root() {
        eprintln!("skipping: requires root");
        return;
    }
    let dir = unique_dir("cmd");
    let bin = make_fake_nspawn(&dir);
    let mut cfg = base_config(&dir);
    cfg.boot = BootMode::Command;
    cfg.command = "/bin/bash -l".into();
    let script = write_script(&dir, &cfg);
    let capture = dir.join("captured.txt");

    let output = Command::new("bash")
        .arg(&script)
        .env("PATH", format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default()))
        .env("NSPAWN_CAPTURE", &capture)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    let captured = fs::read_to_string(&capture).unwrap();
    let args: Vec<&str> = captured.lines().collect();
    assert!(!args.contains(&"-b"));
    let dd = args.iter().position(|a| *a == "--").expect("-- separator");
    assert_eq!(&args[dd + 1..], &["/bin/bash", "-l"]);
    fs::remove_dir_all(dir).ok();
}

#[test]
fn service_mode_forces_read_only_console() {
    if !is_root() {
        eprintln!("skipping: requires root");
        return;
    }
    let dir = unique_dir("service");
    let bin = make_fake_nspawn(&dir);
    let cfg = base_config(&dir);
    let script = write_script(&dir, &cfg);
    let capture = dir.join("captured.txt");

    let output = Command::new("bash")
        .arg(&script)
        .env("PATH", format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default()))
        .env("NSPAWN_CAPTURE", &capture)
        .env("NSPAWN_STUDIO_SERVICE", "1")
        .output()
        .unwrap();
    assert!(output.status.success());
    let captured = fs::read_to_string(&capture).unwrap();
    assert!(captured.lines().any(|a| a == "--console=read-only"), "{}", captured);
    fs::remove_dir_all(dir).ok();
}

#[test]
fn launcher_survives_a_minimal_environment() {
    if !is_root() {
        eprintln!("skipping: requires root");
        return;
    }
    let dir = unique_dir("env");
    let bin = make_fake_nspawn(&dir);
    let cfg = base_config(&dir);
    let script = write_script(&dir, &cfg);
    let capture = dir.join("captured.txt");

    // Run with an empty environment: no HOME, no DISPLAY, no XDG_RUNTIME_DIR.
    let output = Command::new("env")
        .arg("-i")
        .arg(format!("PATH={}:/usr/bin:/bin", bin.display()))
        .arg(format!("NSPAWN_CAPTURE={}", capture.display()))
        .arg("/bin/bash")
        .arg(&script)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "launcher failed with a minimal environment: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let captured = fs::read_to_string(&capture).unwrap();
    assert!(captured.contains("--machine=itest"));
    fs::remove_dir_all(dir).ok();
}

#[test]
fn launcher_refuses_missing_rootfs() {
    if !is_root() {
        eprintln!("skipping: requires root");
        return;
    }
    let dir = unique_dir("missing");
    let mut cfg = base_config(&dir);
    cfg.rootfs = dir.join("does-not-exist");
    let script = write_script(&dir, &cfg);
    let output = Command::new("bash").arg(&script).output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("root filesystem not found"), "{}", stderr);
    fs::remove_dir_all(dir).ok();
}
