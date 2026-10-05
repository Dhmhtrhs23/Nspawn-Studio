//! On-disk storage of container configurations, launcher scripts and units.

use crate::generator::{generate_script, generate_unit};
use crate::model::ContainerConfig;
use crate::validate::validate_container_name;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Errors produced by the configuration store.
#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    Json(serde_json::Error),
    NotFound(String),
    InvalidName(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "{}", e),
            StoreError::Json(e) => write!(f, "invalid JSON: {}", e),
            StoreError::NotFound(n) => write!(f, "container '{}' does not exist", n),
            StoreError::InvalidName(n) => write!(f, "invalid container name '{}'", n),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<io::Error> for StoreError {
    fn from(value: io::Error) -> Self {
        StoreError::Io(value)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(value: serde_json::Error) -> Self {
        StoreError::Json(value)
    }
}

/// True when the effective uid is 0.
pub fn running_as_root() -> bool {
    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("Uid:") {
                if let Some(first) = rest.split_whitespace().next() {
                    return first == "0";
                }
            }
        }
    }
    false
}

/// Where Nspawn Studio keeps its state.
///
/// NSPAWN_STUDIO_HOME wins (handy for tests), then /var/lib/nspawn-studio for
/// root, then the per-user data directory.
pub fn default_data_root() -> PathBuf {
    if let Some(value) = std::env::var_os("NSPAWN_STUDIO_HOME") {
        if !value.is_empty() {
            return PathBuf::from(value);
        }
    }
    if running_as_root() {
        return PathBuf::from("/var/lib/nspawn-studio");
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/root"));
    home.join(".local/share/nspawn-studio")
}

/// Filesystem backed configuration store.
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

impl Store {
    pub fn new() -> Self {
        Self::with_root(default_data_root())
    }

    pub fn with_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn containers_dir(&self) -> PathBuf {
        self.root.join("containers")
    }

    pub fn scripts_dir(&self) -> PathBuf {
        self.root.join("scripts")
    }

    pub fn units_dir(&self) -> PathBuf {
        self.root.join("units")
    }

    pub fn machines_dir(&self) -> PathBuf {
        self.root.join("machines")
    }

    /// Create every directory the store needs.
    pub fn ensure(&self) -> io::Result<()> {
        for dir in [
            self.containers_dir(),
            self.scripts_dir(),
            self.units_dir(),
            self.machines_dir(),
        ] {
            fs::create_dir_all(&dir)?;
        }
        Ok(())
    }

    pub fn config_path(&self, name: &str) -> PathBuf {
        self.containers_dir().join(format!("{}.json", name))
    }

    pub fn script_path(&self, name: &str) -> PathBuf {
        self.scripts_dir().join(format!("{}.sh", name))
    }

    pub fn unit_path(&self, name: &str) -> PathBuf {
        self.units_dir().join(format!("nspawn-studio-{}.service", name))
    }

    /// Default root filesystem location for a new container.
    pub fn default_rootfs(&self, name: &str) -> PathBuf {
        self.machines_dir().join(name)
    }

    pub fn exists(&self, name: &str) -> bool {
        self.config_path(name).is_file()
    }

    /// Names of all stored containers, sorted.
    pub fn list_names(&self) -> io::Result<Vec<String>> {
        let mut names = Vec::new();
        let dir = self.containers_dir();
        if !dir.exists() {
            return Ok(names);
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    names.push(stem.to_string());
                }
            }
        }
        names.sort();
        Ok(names)
    }

    /// Load every stored container, sorted by name. Files that cannot be
    /// parsed are skipped so one broken configuration cannot brick the app.
    pub fn list(&self) -> io::Result<Vec<ContainerConfig>> {
        let mut out = Vec::new();
        for name in self.list_names()? {
            match self.load(&name) {
                Ok(cfg) => out.push(cfg),
                Err(e) => eprintln!("nspawn-studio: skipping container '{}': {}", name, e),
            }
        }
        Ok(out)
    }

    /// Load one container.
    pub fn load(&self, name: &str) -> Result<ContainerConfig, StoreError> {
        if !validate_container_name(name).is_empty() {
            return Err(StoreError::InvalidName(name.to_string()));
        }
        let path = self.config_path(name);
        if !path.is_file() {
            return Err(StoreError::NotFound(name.to_string()));
        }
        let text = fs::read_to_string(&path)?;
        let cfg: ContainerConfig = serde_json::from_str(&text)?;
        Ok(cfg)
    }

    /// Save a configuration and regenerate its launcher script.
    pub fn save(&self, cfg: &ContainerConfig) -> Result<(), StoreError> {
        let problems = validate_container_name(&cfg.name);
        if !problems.is_empty() {
            return Err(StoreError::InvalidName(cfg.name.clone()));
        }
        self.ensure()?;
        let json = serde_json::to_string_pretty(cfg)?;
        let path = self.config_path(&cfg.name);
        write_atomic(&path, json.as_bytes())?;
        // The configuration can contain account passwords, keep it root-only.
        let mut perms = fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&path, perms)?;
        self.write_script(cfg)?;
        Ok(())
    }

    /// Write just the launcher script (0755) and return its path.
    pub fn write_script(&self, cfg: &ContainerConfig) -> Result<PathBuf, StoreError> {
        self.ensure()?;
        let script = generate_script(cfg);
        let path = self.script_path(&cfg.name);
        write_atomic(&path, script.as_bytes())?;
        let mut perms = fs::metadata(&path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms)?;
        Ok(path)
    }

    /// Write the optional systemd unit and return its path.
    pub fn write_unit(&self, cfg: &ContainerConfig) -> Result<PathBuf, StoreError> {
        self.ensure()?;
        let script = self.script_path(&cfg.name);
        let script_str = script.to_string_lossy().to_string();
        let unit = generate_unit(cfg, &script_str);
        let path = self.unit_path(&cfg.name);
        write_atomic(&path, unit.as_bytes())?;
        Ok(path)
    }

    /// Remove the configuration, launcher script and unit. The root
    /// filesystem is left untouched.
    pub fn delete(&self, name: &str) -> Result<(), StoreError> {
        if !self.exists(name) {
            return Err(StoreError::NotFound(name.to_string()));
        }
        fs::remove_file(self.config_path(name))?;
        let script = self.script_path(name);
        if script.exists() {
            fs::remove_file(script)?;
        }
        let unit = self.unit_path(name);
        if unit.exists() {
            fs::remove_file(unit)?;
        }
        Ok(())
    }

    /// Delete the root filesystem of a container. Refuses to touch anything
    /// that is not inside the store's machines directory.
    pub fn remove_rootfs(&self, name: &str) -> Result<(), StoreError> {
        let dir = self.default_rootfs(name);
        if !dir.exists() {
            return Ok(());
        }
        let base = self.machines_dir();
        if !is_within(&dir, &base) {
            return Err(StoreError::InvalidName(format!(
                "refusing to delete rootfs outside {}",
                base.display()
            )));
        }
        fs::remove_dir_all(&dir)?;
        Ok(())
    }
}

/// Write a file atomically (temp file + rename).
pub fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp-nspawn-studio");
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(contents)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

/// True when child is inside (or equal to) base.
pub fn is_within(child: &Path, base: &Path) -> bool {
    let child = child.canonicalize().unwrap_or_else(|_| child.to_path_buf());
    let base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    child.starts_with(&base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BindMount, SecurityConfig};

    fn temp_store() -> (Store, PathBuf) {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("nspawn-studio-store-{}", nanos));
        let store = Store::with_root(&dir);
        (store, dir)
    }

    #[test]
    fn ensure_creates_directories() {
        let (store, dir) = temp_store();
        store.ensure().unwrap();
        assert!(store.containers_dir().is_dir());
        assert!(store.scripts_dir().is_dir());
        assert!(store.units_dir().is_dir());
        assert!(store.machines_dir().is_dir());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn save_load_roundtrip() {
        let (store, dir) = temp_store();
        let mut cfg = ContainerConfig::new("demo", store.default_rootfs("demo"));
        cfg.mounts.push(BindMount::new("/home", "/mnt/home", true));
        cfg.security = SecurityConfig::recommended();
        store.save(&cfg).unwrap();

        assert!(store.exists("demo"));
        assert!(store.script_path("demo").is_file());
        let mode = fs::metadata(store.script_path("demo"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o755);

        let back = store.load("demo").unwrap();
        assert_eq!(cfg, back);
        assert!(back.security.no_new_privileges);

        let names = store.list_names().unwrap();
        assert_eq!(names, vec!["demo".to_string()]);
        let all = store.list().unwrap();
        assert_eq!(all.len(), 1);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn load_missing_is_not_found() {
        let (store, dir) = temp_store();
        match store.load("nope") {
            Err(StoreError::NotFound(_)) => {}
            other => panic!("unexpected: {:?}", other),
        }
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn invalid_names_are_rejected() {
        let (store, dir) = temp_store();
        let cfg = ContainerConfig::new("../evil", PathBuf::from("/tmp/x"));
        assert!(matches!(
            store.save(&cfg),
            Err(StoreError::InvalidName(_))
        ));
        assert!(matches!(
            store.load("../evil"),
            Err(StoreError::InvalidName(_))
        ));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn unit_is_written() {
        let (store, dir) = temp_store();
        let cfg = ContainerConfig::new("demo", store.default_rootfs("demo"));
        let path = store.write_unit(&cfg).unwrap();
        assert_eq!(
            path.file_name().unwrap().to_string_lossy(),
            "nspawn-studio-demo.service"
        );
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("[Service]"));
        assert!(text.contains("scripts/demo.sh"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn delete_removes_config_and_script_only() {
        let (store, dir) = temp_store();
        let cfg = ContainerConfig::new("demo", store.default_rootfs("demo"));
        store.save(&cfg).unwrap();
        fs::create_dir_all(store.default_rootfs("demo")).unwrap();
        store.delete("demo").unwrap();
        assert!(!store.exists("demo"));
        assert!(!store.script_path("demo").exists());
        assert!(store.default_rootfs("demo").is_dir());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn remove_rootfs_is_contained() {
        let (store, dir) = temp_store();
        let rootfs = store.default_rootfs("demo");
        fs::create_dir_all(&rootfs).unwrap();
        fs::write(rootfs.join("marker"), b"x").unwrap();
        store.remove_rootfs("demo").unwrap();
        assert!(!rootfs.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn is_within_works() {
        let base = std::env::temp_dir();
        assert!(is_within(&base, &base));
        assert!(is_within(&base.join("child"), &base));
        assert!(!is_within(Path::new("/usr"), &base) || base == Path::new("/usr"));
    }
}
