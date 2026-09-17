use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub ssid: String,
    pub password: String,
}
pub fn validate(c: &Change) -> Result<()> {
    if c.ssid.is_empty() || c.ssid.len() > 32 || c.ssid.chars().any(char::is_control) {
        bail!("SSID must contain 1–32 UTF-8 bytes, without control characters");
    }
    if !(8..=63).contains(&c.password.len()) || !c.password.bytes().all(|b| (32..=126).contains(&b))
    {
        bail!("Password must contain 8–63 printable ASCII characters");
    }
    Ok(())
}
fn path() -> Option<(PathBuf, bool)> {
    let nm = PathBuf::from("/etc/NetworkManager/system-connections/dji-hdmi.nmconnection");
    if nm.exists() {
        return Some((nm, true));
    }
    let ap = PathBuf::from("/etc/dji-hdmi/hostapd.conf");
    ap.exists().then_some((ap, false))
}
fn field<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    s.lines().find_map(|l| l.strip_prefix(key))
}
fn decode_hex(s: &str) -> Option<String> {
    let b: Option<Vec<_>> = (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect();
    String::from_utf8(b?).ok()
}
pub fn status() -> Value {
    let Some((p, nm)) = path() else {
        return json!({"available":false});
    };
    let s = fs::read_to_string(p).unwrap_or_default();
    let ssid = if nm {
        field(&s, "ssid=").and_then(|s| {
            if s.ends_with(';') {
                let b: Option<Vec<u8>> = s
                    .trim_end_matches(';')
                    .split(';')
                    .map(|n| n.parse().ok())
                    .collect();
                String::from_utf8(b?).ok()
            } else {
                Some(s.to_owned())
            }
        })
    } else {
        field(&s, "ssid2=")
            .and_then(decode_hex)
            .or_else(|| field(&s, "ssid=").map(str::to_owned))
    };
    json!({"available":true,"ssid":ssid,"backend":if nm {"NetworkManager"}else{"hostapd"}})
}
pub fn apply(c: Change) -> Result<()> {
    validate(&c)?;
    let (path, nm) = path().context("No managed access point configured")?;
    let original = fs::read_to_string(&path)?;
    let next = if nm {
        let ssid = c.ssid.bytes().map(|b| format!("{b};")).collect::<String>();
        // Keyfile strings escape backslashes and leading/trailing spaces.
        let password = c.password.replace('\\', "\\\\").replace(' ', "\\s");
        replace_fields(&original, &[("ssid=", ssid), ("psk=", password)])
    } else {
        let ssid = c
            .ssid
            .bytes()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let s = original
            .lines()
            .filter(|l| {
                !l.starts_with("ssid=")
                    && !l.starts_with("ssid2=")
                    && !l.starts_with("wpa_passphrase=")
                    && !l.starts_with("wpa_psk=")
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("{s}\nssid2={ssid}\nwpa_passphrase={}\n", c.password)
    };
    private_write(&path, next.as_bytes())?;
    let result = restart(nm);
    if result.is_err() {
        private_write(&path, original.as_bytes())?;
        let _ = restart(nm);
    }
    result
}
fn replace_fields(s: &str, fields: &[(&str, String)]) -> String {
    s.lines()
        .map(|l| {
            fields
                .iter()
                .find(|(key, _)| l.starts_with(key))
                .map_or_else(|| l.to_owned(), |(key, v)| format!("{key}{v}"))
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}
fn private_write(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let temp = path.with_extension("new");
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temp)?;
    f.set_permissions(fs::Permissions::from_mode(0o600))?;
    f.write_all(bytes)?;
    f.sync_all()?;
    fs::rename(temp, path)?;
    Ok(())
}
fn restart(nm: bool) -> Result<()> {
    if nm {
        run("nmcli", &["connection", "reload"])?;
        run("nmcli", &["--wait", "20", "connection", "up", "dji-hdmi"])
    } else {
        run("systemctl", &["restart", "dji-hdmi-ap.service"])
    }
}
fn run(program: &str, args: &[&str]) -> Result<()> {
    let out = Command::new("timeout")
        .arg("30")
        .arg(program)
        .args(args)
        .output()?;
    if !out.status.success() {
        bail!("Wi-Fi could not be applied; previous configuration restored");
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_wifi_without_shell_interpretation() {
        assert!(
            validate(&Change {
                ssid: "My Wi-Fi".into(),
                password: "12345678".into()
            })
            .is_ok()
        );
        assert!(
            validate(&Change {
                ssid: "bad\nssid".into(),
                password: "12345678".into()
            })
            .is_err()
        );
        assert!(
            validate(&Change {
                ssid: "x".into(),
                password: "short".into()
            })
            .is_err()
        );
        assert!(
            validate(&Change {
                ssid: "é".repeat(17),
                password: "12345678".into()
            })
            .is_err()
        );
        assert_eq!(decode_hex("4d792057694669"), Some("My WiFi".into()));
    }
}
