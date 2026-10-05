//! Validation helpers shared by the GUI and the generator.
//!
//! Every function returns a list of human readable problems; an empty list
//! means the value is acceptable.

use crate::model::{BootMode, ContainerConfig, NetworkMode, X11Access};

/// Linux capabilities known to systemd-nspawn (Linux 6.x).
pub const CAPABILITIES: &[&str] = &[
    "CAP_AUDIT_CONTROL",
    "CAP_AUDIT_READ",
    "CAP_AUDIT_WRITE",
    "CAP_BLOCK_SUSPEND",
    "CAP_BPF",
    "CAP_CHECKPOINT_RESTORE",
    "CAP_CHOWN",
    "CAP_DAC_OVERRIDE",
    "CAP_DAC_READ_SEARCH",
    "CAP_FOWNER",
    "CAP_FSETID",
    "CAP_IPC_LOCK",
    "CAP_IPC_OWNER",
    "CAP_KILL",
    "CAP_LEASE",
    "CAP_LINUX_IMMUTABLE",
    "CAP_MAC_ADMIN",
    "CAP_MAC_OVERRIDE",
    "CAP_MKNOD",
    "CAP_NET_ADMIN",
    "CAP_NET_BIND_SERVICE",
    "CAP_NET_BROADCAST",
    "CAP_NET_RAW",
    "CAP_PERFMON",
    "CAP_SETFCAP",
    "CAP_SETGID",
    "CAP_SETPCAP",
    "CAP_SETUID",
    "CAP_SYS_ADMIN",
    "CAP_SYS_BOOT",
    "CAP_SYS_CHROOT",
    "CAP_SYS_MODULE",
    "CAP_SYS_NICE",
    "CAP_SYS_PACCT",
    "CAP_SYS_PTRACE",
    "CAP_SYS_RAWIO",
    "CAP_SYS_RESOURCE",
    "CAP_SYS_TIME",
    "CAP_SYS_TTY_CONFIG",
    "CAP_SYSLOG",
    "CAP_WAKE_ALARM",
];

/// True if the string is an acceptable container/config identifier.
pub fn valid_container_name(name: &str) -> bool {
    validate_container_name(name).is_empty()
}

/// Validate the container/config identifier.
pub fn validate_container_name(name: &str) -> Vec<String> {
    let mut errors = Vec::new();
    if name.is_empty() {
        errors.push("name must not be empty".to_string());
        return errors;
    }
    if name.len() > 48 {
        errors.push("name must be at most 48 characters".to_string());
    }
    if name == "." || name == ".." {
        errors.push("name may not be '.' or '..'".to_string());
    }
    if !name
        .chars()
        .next()
        .map(|c| c.is_ascii_alphanumeric())
        .unwrap_or(false)
    {
        errors.push("name must start with a letter or digit".to_string());
    }
    for c in name.chars() {
        if !(c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.') {
            errors.push(format!("name contains an invalid character: '{}'", c));
            break;
        }
    }
    errors
}

/// Validate a systemd machine name (slightly stricter than container name).
pub fn validate_machine_name(name: &str) -> Vec<String> {
    let mut errors = validate_container_name(name);
    if !name.is_empty() && name.len() > 64 {
        errors.push("machine name must be at most 64 characters".to_string());
    }
    errors
}

/// Validate that a path is absolute and non-empty.
pub fn validate_absolute_path(path: &str, label: &str) -> Vec<String> {
    let mut errors = Vec::new();
    if path.trim().is_empty() {
        errors.push(format!("{} must not be empty", label));
    } else if !path.starts_with('/') {
        errors.push(format!("{} must be an absolute path (got '{}')", label, path));
    }
    errors
}

/// Validate a capability name against the known list.
pub fn validate_capability(cap: &str) -> bool {
    CAPABILITIES.contains(&cap)
}

/// True if the string is a valid POSIX user name.
pub fn valid_username(name: &str) -> bool {
    if name.is_empty() || name.len() > 32 || name == "root" {
        return false;
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !(first.is_ascii_lowercase() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// Validate the user accounts of a container configuration.
pub fn validate_users(cfg: &ContainerConfig) -> Vec<String> {
    let mut errors = Vec::new();
    if cfg.root_password.contains('\n') {
        errors.push("root password must not contain a newline".to_string());
    }
    let mut seen = std::collections::HashSet::new();
    for user in &cfg.users {
        if !valid_username(&user.name) {
            errors.push(format!(
                "'{}' is not a valid user name (lowercase letters, digits, '_' and '-' only, max 32)",
                user.name
            ));
        } else if !seen.insert(user.name.clone()) {
            errors.push(format!("duplicate user '{}'", user.name));
        }
        if user.password.contains('\n') {
            errors.push(format!("password for '{}' must not contain a newline", user.name));
        }
    }
    errors
}

/// Validate a whole container configuration.
pub fn validate_config(cfg: &ContainerConfig) -> Vec<String> {
    let mut errors = Vec::new();
    errors.extend(validate_container_name(&cfg.name));
    errors.extend(validate_machine_name(cfg.machine_name()));
    errors.extend(validate_absolute_path(
        &cfg.rootfs.to_string_lossy(),
        "root filesystem",
    ));

    if let Some(hostname) = &cfg.hostname {
        if !hostname.is_empty() && !valid_hostname(hostname) {
            errors.push(format!("hostname '{}' is not a valid host name", hostname));
        }
    }

    if cfg.x11 == X11Access::Xephyr && !valid_screen_size(&cfg.xephyr_screen) {
        errors.push(format!(
            "'{}' is not a valid Xephyr screen size (use WIDTHxHEIGHT, e.g. 1280x800)",
            cfg.xephyr_screen
        ));
    }

    if cfg.boot == BootMode::Command && cfg.command.trim().is_empty() {
        errors.push("command mode requires a command to run".to_string());
    }

    if cfg.network.mode == NetworkMode::Host && !cfg.network.port_forwards.is_empty() {
        errors.push(
            "port forwards are not available with host networking; switch to private or veth"
                .to_string(),
        );
    }

    for (i, fwd) in cfg.network.port_forwards.iter().enumerate() {
        let proto = fwd.protocol.to_ascii_lowercase();
        if proto != "tcp" && proto != "udp" {
            errors.push(format!("port forward {}: protocol must be tcp or udp", i + 1));
        }
        if fwd.host_port == 0 || fwd.container_port == 0 {
            errors.push(format!("port forward {}: ports must be non-zero", i + 1));
        }
    }

    if let Some(bridge) = &cfg.network.bridge {
        if cfg.network.mode != NetworkMode::Veth && !bridge.is_empty() {
            errors.push("a bridge can only be used with veth networking".to_string());
        }
        if !bridge.is_empty() && !valid_ifname(bridge) {
            errors.push(format!("bridge '{}' is not a valid interface name", bridge));
        }
    }

    for (i, m) in cfg.mounts.iter().enumerate() {
        errors.extend(validate_absolute_path(
            &m.host,
            &format!("mount {} host path", i + 1),
        ));
        errors.extend(validate_absolute_path(
            &m.container,
            &format!("mount {} container path", i + 1),
        ));
    }

    for cap in cfg
        .security
        .drop_capabilities
        .iter()
        .chain(cfg.security.add_capabilities.iter())
    {
        if !validate_capability(cap) {
            errors.push(format!("unknown capability '{}'", cap));
        }
    }

    for entry in &cfg.security.system_call_filter {
        if entry.trim().is_empty() {
            errors.push("system call filter entries must not be empty".to_string());
        }
    }

    for entry in &cfg.security.mask {
        errors.extend(validate_absolute_path(entry, "mask path"));
    }

    for entry in &cfg.security.tmpfs {
        let path = entry.split(':').next().unwrap_or("");
        errors.extend(validate_absolute_path(path, "tmpfs path"));
    }

    errors.extend(validate_users(cfg));

    errors
}

/// Validate the debootstrap specification of a container.
pub fn validate_debootstrap(cfg: &ContainerConfig) -> Vec<String> {
    let mut errors = Vec::new();
    let spec = &cfg.debootstrap;
    if spec.suite.trim().is_empty() {
        errors.push("debootstrap suite must not be empty".to_string());
    }
    if spec.mirror.trim().is_empty() {
        errors.push("debootstrap mirror must not be empty".to_string());
    }
    if spec.arch.trim().is_empty() {
        errors.push("debootstrap architecture must not be empty".to_string());
    }
    if spec.variant.trim().is_empty() {
        errors.push("debootstrap variant must not be empty".to_string());
    }
    for pkg in &spec.include {
        if pkg.trim().is_empty() {
            errors.push("debootstrap include list contains an empty package".to_string());
            break;
        }
    }
    errors
}

fn valid_hostname(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 253
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
        && !name.starts_with('-')
        && !name.ends_with('-')
}

fn valid_screen_size(value: &str) -> bool {
    match value.split_once('x') {
        Some((w, h)) => match (w.parse::<u32>(), h.parse::<u32>()) {
            (Ok(w), Ok(h)) => w > 0 && h > 0,
            _ => false,
        },
        None => false,
    }
}

fn valid_ifname(name: &str) -> bool {
    !name.is_empty() && name.len() <= 15 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == ':')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BindMount, PortForward};
    use std::path::PathBuf;

    #[test]
    fn container_names() {
        assert!(valid_container_name("demo"));
        assert!(valid_container_name("demo-1.2_x"));
        assert!(!valid_container_name(""));
        assert!(!valid_container_name("-demo"));
        assert!(!valid_container_name(".hidden"));
        assert!(!valid_container_name("demo/../x"));
        assert!(!valid_container_name("demo name"));
        assert!(!valid_container_name(".."));
    }

    #[test]
    fn paths_must_be_absolute() {
        assert!(validate_absolute_path("/var/lib/x", "x").is_empty());
        assert!(!validate_absolute_path("relative/x", "x").is_empty());
        assert!(!validate_absolute_path("", "x").is_empty());
    }

    #[test]
    fn ports_require_isolation() {
        let mut cfg = ContainerConfig::new("c", PathBuf::from("/tmp/c"));
        cfg.network.port_forwards.push(PortForward::new("tcp", 8080, 80));
        assert!(!validate_config(&cfg).is_empty(), "host net + port forward must fail");
        cfg.network.mode = NetworkMode::Private;
        assert!(validate_config(&cfg).is_empty(), "private net + port forward must pass");
        cfg.network.port_forwards[0].protocol = "sctp".into();
        assert!(!validate_config(&cfg).is_empty());
    }

    #[test]
    fn capabilities_are_checked() {
        let mut cfg = ContainerConfig::new("c", PathBuf::from("/tmp/c"));
        cfg.security.drop_capabilities.push("CAP_SYS_ADMIN".into());
        assert!(validate_config(&cfg).is_empty());
        cfg.security.drop_capabilities.push("CAP_NOT_REAL".into());
        assert!(!validate_config(&cfg).is_empty());
    }

    #[test]
    fn mounts_are_checked() {
        let mut cfg = ContainerConfig::new("c", PathBuf::from("/tmp/c"));
        cfg.mounts.push(BindMount::new("/home", "/mnt/home", false));
        assert!(validate_config(&cfg).is_empty());
        cfg.mounts.push(BindMount::new("relative", "/mnt/x", false));
        assert!(!validate_config(&cfg).is_empty());
    }

    #[test]
    fn debootstrap_spec_is_checked() {
        let cfg = ContainerConfig::new("c", PathBuf::from("/tmp/c"));
        assert!(validate_debootstrap(&cfg).is_empty());
        let mut broken = cfg.clone();
        broken.debootstrap.suite = String::new();
        assert!(!validate_debootstrap(&broken).is_empty());
    }

    #[test]
    fn usernames_are_checked() {
        assert!(valid_username("alice"));
        assert!(valid_username("_svc"));
        assert!(valid_username("build-1"));
        assert!(!valid_username("Alice"));
        assert!(!valid_username("1bob"));
        assert!(!valid_username("root"));
        assert!(!valid_username("a b"));
        assert!(!valid_username(""));
        assert!(!valid_username("x".repeat(40).as_str()));
    }

    #[test]
    fn duplicate_users_are_rejected() {
        let mut cfg = ContainerConfig::new("c", PathBuf::from("/tmp/c"));
        cfg.users.push(crate::model::ContainerUser::new("alice", "x", true));
        cfg.users.push(crate::model::ContainerUser::new("alice", "y", false));
        assert!(!validate_config(&cfg).is_empty());
        cfg.users.pop();
        assert!(validate_config(&cfg).is_empty());
    }

    #[test]
    fn recommended_profile_validates() {
        let mut cfg = ContainerConfig::new("hardened", PathBuf::from("/tmp/hardened"));
        cfg.security = crate::model::SecurityConfig::recommended();
        assert!(validate_config(&cfg).is_empty());
    }
}
