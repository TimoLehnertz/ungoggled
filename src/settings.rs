use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub hdmi_mode: String,
    pub fallback_image: Option<String>,
    pub preview_enabled: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            hdmi_mode: "auto".into(),
            fallback_image: Some("no-signal.png".into()),
            preview_enabled: true,
        }
    }
}
#[derive(Clone)]
pub struct Store {
    pub dir: PathBuf,
    pub value: Arc<Mutex<Settings>>,
}
impl Store {
    pub fn open(dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(dir.join("images"))?;
        let first_run = !dir.join("settings.json").exists();
        if first_run {
            atomic_write(
                &dir.join("images/no-signal.png"),
                include_bytes!("../assets/no-signal.png"),
            )?;
        }
        let value = match fs::read(dir.join("settings.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Settings::default(),
            Err(e) => return Err(e.into()),
        };
        if first_run {
            atomic_write(
                &dir.join("settings.json"),
                &serde_json::to_vec_pretty(&value)?,
            )?;
        }
        Ok(Self {
            dir,
            value: Arc::new(Mutex::new(value)),
        })
    }
    pub fn get(&self) -> Settings {
        self.value.lock().unwrap().clone()
    }
    pub fn save(&self, value: Settings) -> Result<()> {
        if !valid_mode(&value.hdmi_mode) {
            bail!("Invalid HDMI mode");
        }
        if let Some(id) = &value.fallback_image
            && (!valid_id(id) || !self.dir.join("images").join(id).is_file())
        {
            bail!("Unknown fallback image");
        }
        // Serialize concurrent saves together with the file replacement.
        let mut current = self.value.lock().unwrap();
        atomic_write(
            &self.dir.join("settings.json"),
            &serde_json::to_vec_pretty(&value)?,
        )?;
        *current = value;
        Ok(())
    }
}
pub fn valid_id(id: &str) -> bool {
    id.ends_with(".png")
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
        && !id.contains("..")
}
pub fn valid_mode(mode: &str) -> bool {
    if mode == "auto" {
        return true;
    }
    let Some((size, hz)) = mode.split_once('@') else {
        return false;
    };
    let Some((w, h)) = size.split_once('x') else {
        return false;
    };
    match (w.parse::<u32>(), h.parse::<u32>(), hz.parse::<u32>()) {
        (Ok(w), Ok(h), Ok(hz)) => {
            (320..=1920).contains(&w) && (240..=1080).contains(&h) && (24..=60).contains(&hz)
        }
        _ => false,
    }
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut f = fs::File::create(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    fs::rename(tmp, path)?;
    fs::File::open(path.parent().unwrap())?.sync_all()?;
    Ok(())
}
/// Stable updater entry point. Each release owns migration into its own schema.
/// Preserve recognized compatible values; report fields that need defaults.
pub fn migrate(data_dir: &Path, output: &Path) -> Result<Vec<String>> {
    let old: serde_json::Value = match fs::read(data_dir.join("settings.json")) {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
        Err(e) => return Err(e.into()),
    };
    let object = old
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Settings file must contain an object"))?;
    let mut next = Settings::default();
    let mut warnings = Vec::new();
    for (key, value) in object {
        let accepted = match key.as_str() {
            "hdmi_mode" => value
                .as_str()
                .filter(|v| valid_mode(v))
                .map(|v| next.hdmi_mode = v.into())
                .is_some(),
            "preview_enabled" => value.as_bool().map(|v| next.preview_enabled = v).is_some(),
            "fallback_image" if value.is_null() => {
                next.fallback_image = None;
                true
            }
            "fallback_image" => value
                .as_str()
                .filter(|v| valid_id(v) && data_dir.join("images").join(v).is_file())
                .map(|v| next.fallback_image = Some(v.into()))
                .is_some(),
            _ => false,
        };
        if !accepted {
            warnings.push(format!(
                "{key}: not compatible with this release; default used"
            ));
        }
    }
    if next
        .fallback_image
        .as_ref()
        .is_some_and(|id| !data_dir.join("images").join(id).is_file())
    {
        next.fallback_image = None;
    }
    atomic_write(output, &serde_json::to_vec_pretty(&next)?)?;
    Ok(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_paths_and_invalid_modes() {
        for s in ["../a.png", "/a.png", "a/b.png", "a.png\n", "a..png"] {
            assert!(!valid_id(s));
        }
        assert!(valid_id("fallback-123.png"));
        assert!(valid_mode("1920x1080@60"));
        for s in ["3840x2160@60", "1920x1080@0", "1920x1080@600", "auto\n"] {
            assert!(!valid_mode(s));
        }
    }
    #[test]
    fn migration_preserves_images_and_reports_incompatible_values() {
        let f = crate::update::tests::Fixture::new();
        fs::create_dir(f.0.join("images")).unwrap();
        fs::write(f.0.join("images/custom.png"), "image").unwrap();
        let source =
            br#"{"hdmi_mode":"invalid","preview_enabled":false,"fallback_image":"custom.png"}"#;
        fs::write(f.0.join("settings.json"), source).unwrap();
        let output = f.0.join("migrated.json");
        let warnings = migrate(&f.0, &output).unwrap();
        assert_eq!(warnings.len(), 1);
        let migrated: Settings = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        assert_eq!(migrated.hdmi_mode, "auto");
        assert!(!migrated.preview_enabled);
        assert_eq!(migrated.fallback_image.as_deref(), Some("custom.png"));
        assert_eq!(fs::read(f.0.join("settings.json")).unwrap(), source);
    }
}
