//! Data model for Nspawn Studio containers.
//!
//! The model is intentionally plain data: it can be serialised to JSON,
//! diffed and passed around without any GTK dependency. Everything that
//! turns the model into an actual systemd-nspawn invocation lives in
//! crate::generator.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Where the root filesystem came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// Created by debootstrap (can be re-created from the stored spec).
    Debootstrap,
    /// An existing directory the user pointed us at.
    Custom,
}

impl Default for SourceKind {
    fn default() -> Self {
        SourceKind::Debootstrap
    }
}

impl SourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SourceKind::Debootstrap => "debootstrap",
            SourceKind::Custom => "custom",
        }
    }
}

/// How the container is entered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BootMode {
    /// Boot an init system inside the container (systemd-nspawn -b).
    Boot,
    /// Run an explicit command inside the container.
    Command,
}

impl Default for BootMode {
    fn default() -> Self {
        BootMode::Boot
    }
}

/// X11 integration level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum X11Access {
    /// No X11 access at all.
    None,
    /// Bind only the X11 listening sockets (/tmp/.X11-unix).
    Socket,
    /// Bind the sockets and the user's xauth cookie, and export DISPLAY.
    Authority,
    /// Run a private nested Xephyr server and point the container at it.
    Xephyr,
}

impl Default for X11Access {
    fn default() -> Self {
        X11Access::None
    }
}

impl X11Access {
    pub fn as_str(self) -> &'static str {
        match self {
            X11Access::None => "none",
            X11Access::Socket => "socket",
            X11Access::Authority => "authority",
            X11Access::Xephyr => "xephyr",
        }
    }
}

/// How the container console is wired up when run interactively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConsoleMode {
    Interactive,
    ReadOnly,
    Passive,
    /// Pipe stdin/stdout/stderr (used for non-interactive service runs).
    Pipe,
}

impl Default for ConsoleMode {
    fn default() -> Self {
        ConsoleMode::Interactive
    }
}

impl ConsoleMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ConsoleMode::Interactive => "interactive",
            ConsoleMode::ReadOnly => "read-only",
            ConsoleMode::Passive => "passive",
            ConsoleMode::Pipe => "pipe",
        }
    }
}

/// Network isolation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkMode {
    /// Share the host network namespace (default; simplest, least isolated).
    Host,
    /// --private-network: loopback only.
    Private,
    /// --network-veth: a virtual ethernet link to the host.
    Veth,
}

impl Default for NetworkMode {
    fn default() -> Self {
        NetworkMode::Host
    }
}

impl NetworkMode {
    pub fn as_str(self) -> &'static str {
        match self {
            NetworkMode::Host => "host",
            NetworkMode::Private => "private",
            NetworkMode::Veth => "veth",
        }
    }
}

/// --resolv-conf= handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResolvConfMode {
    /// Let systemd-nspawn decide (the default).
    Auto,
    Off,
    CopyHost,
    CopyStatic,
    Delete,
}

impl Default for ResolvConfMode {
    fn default() -> Self {
        ResolvConfMode::Auto
    }
}

impl ResolvConfMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ResolvConfMode::Auto => "auto",
            ResolvConfMode::Off => "off",
            ResolvConfMode::CopyHost => "copy-host",
            ResolvConfMode::CopyStatic => "copy-static",
            ResolvConfMode::Delete => "delete",
        }
    }
}

/// --timezone= handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TimezoneMode {
    Off,
    Copy,
    Bind,
    Symlink,
}

impl Default for TimezoneMode {
    fn default() -> Self {
        TimezoneMode::Copy
    }
}

impl TimezoneMode {
    pub fn as_str(self) -> &'static str {
        match self {
            TimezoneMode::Off => "off",
            TimezoneMode::Copy => "copy",
            TimezoneMode::Bind => "bind",
            TimezoneMode::Symlink => "symlink",
        }
    }
}

/// --private-users-ownership= handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OwnershipMode {
    Off,
    Auto,
    Chown,
    Map,
}

impl Default for OwnershipMode {
    fn default() -> Self {
        OwnershipMode::Auto
    }
}

impl OwnershipMode {
    pub fn as_str(self) -> &'static str {
        match self {
            OwnershipMode::Off => "off",
            OwnershipMode::Auto => "auto",
            OwnershipMode::Chown => "chown",
            OwnershipMode::Map => "map",
        }
    }
}

/// A single host -> container bind mount.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindMount {
    /// Path on the host.
    pub host: String,
    /// Path inside the container.
    pub container: String,
    /// Mount read-only (--bind-ro=) instead of read-write (--bind=).
    #[serde(default)]
    pub read_only: bool,
}

impl BindMount {
    pub fn new(host: impl Into<String>, container: impl Into<String>, read_only: bool) -> Self {
        Self {
            host: host.into(),
            container: container.into(),
            read_only,
        }
    }
}

/// An account to create inside the container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ContainerUser {
    /// POSIX user name.
    pub name: String,
    /// Initial password (stored in the root-only configuration file).
    pub password: String,
    /// Add the user to the sudo group.
    pub sudo: bool,
}

impl Default for ContainerUser {
    fn default() -> Self {
        Self {
            name: String::new(),
            password: String::new(),
            sudo: false,
        }
    }
}

impl ContainerUser {
    pub fn new(name: impl Into<String>, password: impl Into<String>, sudo: bool) -> Self {
        Self {
            name: name.into(),
            password: password.into(),
            sudo,
        }
    }
}

/// A single TCP/UDP port forwarded from the host into the container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortForward {
    /// tcp or udp.
    #[serde(default = "default_protocol")]
    pub protocol: String,
    pub host_port: u16,
    pub container_port: u16,
}

fn default_protocol() -> String {
    "tcp".to_string()
}

impl PortForward {
    pub fn new(protocol: impl Into<String>, host_port: u16, container_port: u16) -> Self {
        Self {
            protocol: protocol.into(),
            host_port,
            container_port,
        }
    }

    /// Render as the value accepted by --port=.
    pub fn to_arg(&self) -> String {
        format!("{}:{}:{}", self.protocol, self.host_port, self.container_port)
    }
}

/// The specification used to (re)build a debootstrap root filesystem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DebootstrapSpec {
    /// Debian suite, e.g. trixie, bookworm, stable.
    pub suite: String,
    /// Mirror base URL.
    pub mirror: String,
    /// Target architecture, e.g. amd64, arm64.
    pub arch: String,
    /// debootstrap variant, e.g. minbase.
    pub variant: String,
    /// Extra packages to install during bootstrap.
    pub include: Vec<String>,
    /// Extra apt components (passed as a comma separated --components=).
    pub components: Vec<String>,
    /// Optional debootstrap --keyring= path.
    pub keyring: Option<String>,
    /// Raw extra arguments appended verbatim to debootstrap.
    pub extra_args: Vec<String>,
}

impl Default for DebootstrapSpec {
    fn default() -> Self {
        Self {
            suite: default_suite(),
            mirror: "http://deb.debian.org/debian".to_string(),
            arch: default_arch(),
            variant: "minbase".to_string(),
            include: vec![
                "systemd".to_string(),
                "systemd-sysv".to_string(),
                "dbus".to_string(),
                "iproute2".to_string(),
                "iputils-ping".to_string(),
                "ca-certificates".to_string(),
                "sudo".to_string(),
                "nano".to_string(),
                "less".to_string(),
            ],
            components: Vec::new(),
            keyring: None,
            extra_args: Vec::new(),
        }
    }
}

pub fn default_suite() -> String {
    // Debian codename of the running host, falling back to stable.
    host_codename().unwrap_or_else(|| "stable".to_string())
}

pub fn default_arch() -> String {
    match std::env::consts::ARCH {
        "x86_64" => "amd64".to_string(),
        "aarch64" => "arm64".to_string(),
        "x86" => "i386".to_string(),
        "arm" => "armhf".to_string(),
        "riscv64" => "riscv64".to_string(),
        "powerpc64" => "ppc64el".to_string(),
        other => other.to_string(),
    }
}

fn host_codename() -> Option<String> {
    let text = std::fs::read_to_string("/etc/os-release").ok()?;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("VERSION_CODENAME=") {
            let value = value.trim().trim_matches('"');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Audio backend selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    pub pipewire: bool,
    pub pulseaudio: bool,
    pub alsa: bool,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            pipewire: false,
            pulseaudio: false,
            alsa: false,
        }
    }
}

impl AudioConfig {
    pub fn any(&self) -> bool {
        self.pipewire || self.pulseaudio || self.alsa
    }
}

/// Network configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkConfig {
    pub mode: NetworkMode,
    /// Bridge interface for NetworkMode::Veth, e.g. br0.
    pub bridge: Option<String>,
    pub port_forwards: Vec<PortForward>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            mode: NetworkMode::Host,
            bridge: None,
            port_forwards: Vec::new(),
        }
    }
}

/// Security and sandboxing knobs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityConfig {
    /// -U: user namespacing.
    pub private_users: bool,
    /// --private-users-ownership=.
    pub ownership: OwnershipMode,
    /// --read-only
    pub read_only: bool,
    /// --ephemeral
    pub ephemeral: bool,
    /// --no-new-privileges=yes
    pub no_new_privileges: bool,
    /// --drop-capability= entries.
    pub drop_capabilities: Vec<String>,
    /// --capability= entries.
    pub add_capabilities: Vec<String>,
    /// --system-call-filter= entries.
    pub system_call_filter: Vec<String>,
    /// --mask= entries.
    pub mask: Vec<String>,
    /// --tmpfs= entries.
    pub tmpfs: Vec<String>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            private_users: false,
            ownership: OwnershipMode::Auto,
            read_only: false,
            ephemeral: false,
            no_new_privileges: false,
            drop_capabilities: Vec::new(),
            add_capabilities: Vec::new(),
            system_call_filter: Vec::new(),
            mask: Vec::new(),
            tmpfs: Vec::new(),
        }
    }
}

impl SecurityConfig {
    /// A sensible, strong sandbox profile used by the "Apply recommended
    /// hardening" button. It deliberately mirrors the example in
    /// systemd-nspawn(1) but keeps the root filesystem writable so a booted
    /// container can still function.
    pub fn recommended() -> Self {
        Self {
            private_users: true,
            ownership: OwnershipMode::Auto,
            read_only: false,
            ephemeral: false,
            no_new_privileges: true,
            // CAP_SYS_ADMIN is deliberately kept: a booted init needs it for
            // mounts inside the container. The seccomp filter and the other
            // dropped capabilities carry the hardening instead.
            drop_capabilities: vec![
                "CAP_SYS_MODULE".into(),
                "CAP_SYS_PTRACE".into(),
                "CAP_SYS_RAWIO".into(),
                "CAP_SYS_BOOT".into(),
                "CAP_MKNOD".into(),
            ],
            add_capabilities: Vec::new(),
            system_call_filter: vec![
                "~@clock".into(),
                "~@cpu-emulation".into(),
                "~@debug".into(),
                "~@module".into(),
                "~@obsolete".into(),
                "~@privileged".into(),
                "~@raw".into(),
                "~@reboot".into(),
                "~@swap".into(),
            ],
            // NOTE: --inaccessible= paths must exist inside the container's root
            // filesystem. Kernel-provided files such as /proc/kcore do not, so the
            // recommended profile leaves this empty and relies on the seccomp
            // system call filter instead.
            mask: Vec::new(),
            tmpfs: Vec::new(),
        }
    }
}

/// Full description of one managed container.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ContainerConfig {
    /// Human friendly, filesystem-safe identifier. Also the config file name.
    pub name: String,
    /// Name registered with systemd (machinectl). Defaults to name.
    pub machine_name: String,
    /// Absolute path to the root filesystem directory.
    pub rootfs: PathBuf,
    pub source: SourceKind,
    pub debootstrap: DebootstrapSpec,
    pub boot: BootMode,
    /// Command line for BootMode::Command (whitespace split, no quoting).
    pub command: String,
    pub hostname: Option<String>,
    /// Password to assign to root inside the container (empty = leave as is).
    pub root_password: String,
    /// Extra accounts to create inside the container.
    pub users: Vec<ContainerUser>,
    pub x11: X11Access,
    /// Screen geometry for Xephyr, e.g. 1280x800.
    pub xephyr_screen: String,
    /// Run xhost +si:localuser:root on the host before starting.
    pub xhost_local: bool,
    pub wayland: bool,
    /// Bind /dev/dri for GPU acceleration.
    pub gpu: bool,
    pub audio: AudioConfig,
    pub mounts: Vec<BindMount>,
    pub network: NetworkConfig,
    pub security: SecurityConfig,
    /// --resolv-conf= handling.
    pub resolv_conf: ResolvConfMode,
    /// --timezone= handling.
    pub timezone: TimezoneMode,
    pub console: ConsoleMode,
    /// Raw extra arguments appended verbatim to systemd-nspawn.
    pub extra_args: Vec<String>,
    /// Unix timestamp of creation.
    pub created_unix: u64,
    /// Unix timestamp of the last modification.
    pub updated_unix: u64,
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            machine_name: String::new(),
            rootfs: PathBuf::new(),
            source: SourceKind::Debootstrap,
            debootstrap: DebootstrapSpec::default(),
            boot: BootMode::Boot,
            command: "/bin/bash -l".to_string(),
            hostname: None,
            root_password: String::new(),
            users: Vec::new(),
            x11: X11Access::None,
            xephyr_screen: "1280x800".to_string(),
            xhost_local: false,
            wayland: false,
            gpu: false,
            audio: AudioConfig::default(),
            mounts: Vec::new(),
            network: NetworkConfig::default(),
            security: SecurityConfig::default(),
            resolv_conf: ResolvConfMode::default(),
            timezone: TimezoneMode::default(),
            console: ConsoleMode::Interactive,
            extra_args: Vec::new(),
            created_unix: 0,
            updated_unix: 0,
        }
    }
}

impl ContainerConfig {
    /// Build a new configuration with sane defaults. rootfs should normally
    /// come from crate::store::Store::default_rootfs.
    pub fn new(name: impl Into<String>, rootfs: PathBuf) -> Self {
        let name = name.into();
        let now = now_unix();
        Self {
            machine_name: name.clone(),
            name,
            rootfs,
            created_unix: now,
            updated_unix: now,
            ..Default::default()
        }
    }

    /// The systemd machine name, falling back to the container name.
    pub fn machine_name(&self) -> &str {
        if self.machine_name.trim().is_empty() {
            &self.name
        } else {
            &self.machine_name
        }
    }

    /// The installable unit name (used by the generated .service file).
    pub fn service_unit(&self) -> String {
        service_unit_name(self.machine_name())
    }

    /// The transient unit name used by the Start buttons. It deliberately
    /// differs from the installable unit so that a unit fragment installed in
    /// /etc/systemd/system can never make systemd-run refuse to start.
    pub fn transient_unit(&self) -> String {
        transient_unit_name(self.machine_name())
    }

    pub fn touch(&mut self) {
        self.updated_unix = now_unix();
    }
}

/// nspawn-studio-<machine>.service (the installable unit)
pub fn service_unit_name(machine: &str) -> String {
    format!("nspawn-studio-{}.service", machine)
}

/// nspawn-studio-run-<machine>.service (the transient unit)
pub fn transient_unit_name(machine: &str) -> String {
    format!("nspawn-studio-run-{}.service", machine)
}

pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_reasonable() {
        let c = ContainerConfig::new("demo", PathBuf::from("/var/lib/nspawn-studio/machines/demo"));
        assert_eq!(c.name, "demo");
        assert_eq!(c.machine_name(), "demo");
        assert_eq!(c.boot, BootMode::Boot);
        assert_eq!(c.console, ConsoleMode::Interactive);
        assert_eq!(c.network.mode, NetworkMode::Host);
        assert!(c.created_unix > 0);
        assert_eq!(c.service_unit(), "nspawn-studio-demo.service");
        assert_eq!(c.transient_unit(), "nspawn-studio-run-demo.service");
    }

    #[test]
    fn json_roundtrip() {
        let mut c = ContainerConfig::new("demo", PathBuf::from("/tmp/demo"));
        c.audio.pipewire = true;
        c.x11 = X11Access::Authority;
        c.mounts.push(BindMount::new("/home", "/mnt/home", true));
        c.network.port_forwards.push(PortForward::new("tcp", 8080, 80));
        c.security = SecurityConfig::recommended();
        let json = serde_json::to_string_pretty(&c).unwrap();
        let back: ContainerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn missing_fields_get_defaults() {
        let json = r#"{ "name": "old", "rootfs": "/tmp/old" }"#;
        let c: ContainerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(c.name, "old");
        assert_eq!(c.machine_name(), "old");
        assert_eq!(c.network.mode, NetworkMode::Host);
        assert_eq!(c.debootstrap.variant, "minbase");
    }

    #[test]
    fn port_forward_arg() {
        assert_eq!(PortForward::new("tcp", 8080, 80).to_arg(), "tcp:8080:80");
        assert_eq!(PortForward::new("udp", 53, 53).to_arg(), "udp:53:53");
    }

    #[test]
    fn recommended_profile_is_strong() {
        let s = SecurityConfig::recommended();
        assert!(s.private_users);
        assert!(s.no_new_privileges);
        assert!(s.drop_capabilities.contains(&"CAP_SYS_MODULE".to_string()));
        assert!(s.system_call_filter.iter().any(|f| f.starts_with("~@")));
    }
}
