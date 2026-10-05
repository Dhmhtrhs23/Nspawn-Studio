//! Optional end-to-end test for account provisioning.
//!
//! Set NSPAWN_STUDIO_TEST_ROOTFS to a bootable container root filesystem to
//! let this test actually create accounts inside it with systemd-nspawn,
//! authenticate through su, and check sudo authorization.

use nspawn_studio_core::generator::provision_script;
use nspawn_studio_core::model::{ContainerConfig, ContainerUser};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn rootfs() -> Option<PathBuf> {
    match std::env::var("NSPAWN_STUDIO_TEST_ROOTFS") {
        Ok(value) if !value.trim().is_empty() => {
            let path = PathBuf::from(value);
            if path.is_dir() {
                Some(path)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn run_in_container(rootfs: &PathBuf, script: &str) -> String {
    let out = Command::new("systemd-nspawn")
        .args([
            "--quiet",
            "-D",
            rootfs.to_str().unwrap(),
            "/bin/sh",
            "-c",
            script,
        ])
        .output()
        .expect("systemd-nspawn must be installed");
    assert!(
        out.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Run a command on a pseudo terminal so su/sudo can prompt.
fn run_pty(command: &str, input: &str) -> String {
    let mut child = Command::new("script")
        .args(["-qec", command, "/dev/null"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("script must be installed");
    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().unwrap();
        let _ = stdin.write_all(input.as_bytes());
    }
    let out = child.wait_with_output().unwrap();
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    text.replace('\r', "")
}

#[test]
fn provision_real_rootfs() {
    let Some(rootfs) = rootfs() else {
        eprintln!("skipping: set NSPAWN_STUDIO_TEST_ROOTFS to run this test");
        return;
    };

    let mut cfg = ContainerConfig::new("provision-test", rootfs.clone());
    cfg.root_password = "rootpw-123".to_string();
    cfg.users
        .push(ContainerUser::new("alice", "alicepw-123", true));
    cfg.users
        .push(ContainerUser::new("bob", "bobpw-123", false));

    let script = provision_script(&cfg);
    run_in_container(&rootfs, &script);

    let report = run_in_container(
        &rootfs,
        "getent passwd alice; getent passwd bob; id -nG alice; id -nG bob; \
         test -f /etc/sudoers.d/90-nspawn-studio && echo SUDOERS_OK; \
         echo -n 'alice-hash:'; getent shadow alice | cut -d: -f2; \
         echo -n 'bob-hash:'; getent shadow bob | cut -d: -f2; \
         echo -n 'root-hash:'; getent shadow root | cut -d: -f2",
    );
    eprintln!("{}", report);

    assert!(report.contains("alice"), "alice missing: {}", report);
    assert!(report.contains("bob"), "bob missing: {}", report);
    assert!(
        report.lines().any(|line| {
            let mut words = line.split_whitespace();
            words.next() == Some("alice") && words.any(|g| g == "sudo")
        }),
        "alice is not in the sudo group: {}",
        report
    );
    assert!(report.contains("SUDOERS_OK"), "sudoers file missing: {}", report);
    for key in ["alice-hash:$", "bob-hash:$", "root-hash:$"] {
        assert!(
            report.lines().any(|l| l.starts_with(key)),
            "{} missing a password hash: {}",
            key,
            report
        );
    }

    // Password authentication really works, and sudo is limited to alice.
    if nspawn_studio_core::command::which("script").is_some() {
        let base = format!("systemd-nspawn --quiet -D {}", rootfs.display());

        let whoami = run_pty(
            &format!("{} /bin/su - alice -c 'id -un'", base),
            "alicepw-123\n",
        );
        assert!(
            whoami.lines().any(|l| l.trim() == "alice"),
            "su authentication failed: {}",
            whoami
        );

        let sudo = run_pty(
            &format!("{} /bin/su - alice -c 'sudo -S id -un'", base),
            "alicepw-123\nalicepw-123\n",
        );
        assert!(
            sudo.lines().any(|l| l.trim() == "root"),
            "alice must be able to sudo: {}",
            sudo
        );

        let denied = run_pty(
            &format!("{} /bin/su - bob -c 'sudo -S id -un'", base),
            "bobpw-123\nbobpw-123\n",
        );
        assert!(
            denied.contains("not in the sudoers file"),
            "bob must not be allowed to sudo: {}",
            denied
        );
    }
}
