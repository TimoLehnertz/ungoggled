//! Filesystem fixtures shared by archive and transaction tests; no real services.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
pub struct Fixture(pub PathBuf);
impl Fixture {
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ungoggled-test-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    pub fn paths(&self) -> Paths {
        let paths = Paths {
            root: self.0.join("root"),
            updates: self.0.join("updates"),
        };
        fs::create_dir_all(&paths.updates).unwrap();
        paths
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
pub fn bundle(dir: &Path) -> package::Manifest {
    use sha2::{Digest, Sha256};
    fs::create_dir_all(dir).unwrap();
    let files = [
        "bin/aarch64/ungoggled",
        "web/index.html",
        "prepare-pi.sh",
        "dji-hdmi.service",
        "ungoggled-update-recovery.service",
        "install.sh",
        "RELEASE_NOTES.md",
    ];
    let mut manifest = package::Manifest {
        format: 1,
        product: "ungoggled".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        minimum_updater: "0.3.0".into(),
        settings_schema: 1,
        files: Default::default(),
    };
    for name in files {
        let content = format!("new {name}");
        let p = dir.join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, &content).unwrap();
        manifest.files.insert(
            name.into(),
            package::FileInfo {
                size: content.len() as u64,
                sha256: package::hex(&Sha256::digest(content.as_bytes())),
            },
        );
    }
    fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    manifest
}
