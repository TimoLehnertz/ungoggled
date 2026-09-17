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
}
