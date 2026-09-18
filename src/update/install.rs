use super::{Lock, Paths, Phase, package};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Duration,
};

const CURRENT: &str = "opt/dji-hdmi/current";
const FILES: &[&str] = &[
    "etc/systemd/system/dji-hdmi.service",
    "usr/local/lib/dji-hdmi/prepare-pi.sh",
    "var/lib/dji-hdmi/settings.json",
];
#[derive(Serialize, Deserialize)]
struct Journal {
    previous: Option<PathBuf>,
    files: Vec<bool>,
    target: PathBuf,
}

pub trait Host {
    fn service(&self, verb: &str) -> Result<()>;
    fn migrate(&self, binary: &Path, data: &Path, output: &Path) -> Result<Vec<String>>;
    fn healthy(&self, version: &str, build: &str) -> bool;
}
struct System;
impl Host for System {
    fn service(&self, verb: &str) -> Result<()> {
        let mut c = Command::new("timeout");
        c.args(["60", "systemctl", verb]);
        if verb != "daemon-reload" {
            c.arg("dji-hdmi.service");
        }
        ensure!(c.status()?.success(), "systemctl {verb} failed");
        Ok(())
    }
    fn migrate(&self, binary: &Path, data: &Path, output: &Path) -> Result<Vec<String>> {
        let out = Command::new("timeout")
            .arg("20")
            .arg(binary)
            .arg("migrate-settings")
            .arg("--data-dir")
            .arg(data)
            .arg("--output")
            .arg(output)
            .output()?;
        ensure!(
            out.status.success(),
            "Settings migration failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).context("Invalid migration report")
    }
    fn healthy(&self, version: &str, build: &str) -> bool {
        let mut consecutive = 0;
        for _ in 0..40 {
            let valid = Command::new("curl")
                .args(["-fsS", "--max-time", "1", "http://127.0.0.1/api/status"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| serde_json::from_slice::<serde_json::Value>(&o.stdout).ok())
                .is_some_and(|s| {
                    s["version"] == version
                        && s["build_id"] == build
                        && s["uptime_seconds"].as_u64().unwrap_or(0) >= 3
                });
            if valid {
                consecutive += 1;
                if consecutive >= 3 {
                    return true;
                }
            } else {
                consecutive = 0;
            }
            thread::sleep(Duration::from_millis(500));
        }
        false
    }
}
fn lock_wait(paths: &Paths) -> Result<Lock> {
    for _ in 0..20 {
        if let Ok(l) = Lock::acquire(paths) {
            return Ok(l);
        }
        thread::sleep(Duration::from_millis(250));
    }
    Lock::acquire(paths)
}
pub fn install(id: &str) -> Result<()> {
    let paths = Paths::default();
    let _lock = lock_wait(&paths)?;
    let result = apply(&paths, id, package::architecture(), &System);
    if let Err(e) = &result {
        let mut state = paths.state()?;
        if state.phase != Phase::RolledBack {
            state.phase = Phase::Failed;
            state.message = format!("{e:#}");
            paths.save(&state)?;
        }
    }
    result
}
fn atomic_copy(source: &Path, destination: &Path, mode: u32) -> Result<()> {
    fs::create_dir_all(destination.parent().unwrap())?;
    // Files are bounded by package validation; copy through a synced file.
    let temp = destination.with_extension("updating");
    let mut input = fs::File::open(source)?;
    let mut output = fs::File::create(&temp)?;
    std::io::copy(&mut input, &mut output)?;
    output.set_permissions(fs::Permissions::from_mode(mode))?;
    output.sync_all()?;
    fs::rename(temp, destination)?;
    sync_parent(destination)
}
fn sync_parent(path: &Path) -> Result<()> {
    fs::File::open(path.parent().unwrap())?.sync_all()?;
    Ok(())
}
fn set_current(paths: &Paths, target: &Path) -> Result<()> {
    let current = paths.at(CURRENT);
    fs::create_dir_all(current.parent().unwrap())?;
    let next = current.with_extension("next");
    let _ = fs::remove_file(&next);
    symlink(target, &next)?;
    fs::rename(next, &current)?;
    sync_parent(&current)
}
fn remove_file(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => sync_parent(path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}
fn setup_recovery(paths: &Paths) -> Result<()> {
    let helper = paths.at("usr/local/lib/dji-hdmi/update-helper");
    atomic_copy(&std::env::current_exe()?, &helper, 0o755)?;
    let unit = paths.at("etc/systemd/system/ungoggled-update-recovery.service");
    fs::create_dir_all(unit.parent().unwrap())?;
    crate::settings::atomic_write(
        &unit,
        include_bytes!("../../deploy/ungoggled-update-recovery.service"),
    )?;
    // Enable directly: no dependency restart while the transaction is in progress.
    let enabled =
        paths.at("etc/systemd/system/multi-user.target.wants/ungoggled-update-recovery.service");
    fs::create_dir_all(enabled.parent().unwrap())?;
    if enabled.symlink_metadata().is_err() {
        symlink("../ungoggled-update-recovery.service", &enabled)?;
        sync_parent(&enabled)?;
    }
    Ok(())
}
fn apply(paths: &Paths, id: &str, arch: &str, host: &impl Host) -> Result<()> {
    let mut state = paths.state()?;
    ensure!(
        state.id.as_deref() == Some(id) && matches!(state.phase, Phase::Ready | Phase::Queued),
        "No matching prepared update"
    );
    let job = paths.job(id)?;
    let source = job.join("package");
    let manifest = package::validate(&source, arch)?;
    let build = manifest.release_id()?;
    let target = paths.at(&format!("opt/dji-hdmi/releases/{build}"));
    let previous = fs::read_link(paths.at(CURRENT)).ok();
    ensure!(
        previous.as_ref() != Some(&target)
            && fs::canonicalize(paths.at(CURRENT)).ok().as_ref() != Some(&target),
        "This exact build is already installed"
    );
    state.phase = Phase::Installing;
    state.message = "Preparing application and backing up settings".into();
    paths.save(&state)?;
    // A leftover incomplete release can only be removed if it is not the active release.
    if target.exists() {
        fs::remove_dir_all(&target)?;
    }
    fs::create_dir_all(target.join("bin"))?;
    for name in manifest.files.keys().filter(|p| p.starts_with("web/")) {
        atomic_copy(&source.join(name), &target.join(name), 0o644)?;
    }
    atomic_copy(
        &source.join(format!("bin/{arch}/ungoggled")),
        &target.join("bin/ungoggled"),
        0o755,
    )?;
    atomic_copy(
        &source.join("manifest.json"),
        &target.join("manifest.json"),
        0o644,
    )?;
    setup_recovery(paths)?;
    let backup = job.join("backup");
    fs::create_dir_all(&backup)?;
    // Stop before taking the settings snapshot, so no request can race the backup.
    if let Err(e) = host.service("stop") {
        let _ = host.service("start");
        return Err(e);
    }
    let mut files = Vec::new();
    let snapshot = (|| -> Result<()> {
        for (i, name) in FILES.iter().enumerate() {
            let path = paths.at(name);
            let exists = path.exists();
            files.push(exists);
            if exists {
                atomic_copy(
                    &path,
                    &backup.join(i.to_string()),
                    if i == 1 { 0o755 } else { 0o600 },
                )?;
            }
        }
        let journal = Journal {
            previous: previous.clone(),
            files,
            target: target.clone(),
        };
        crate::settings::atomic_write(&job.join("journal.json"), &serde_json::to_vec(&journal)?)?;
        Ok(())
    })();
    if let Err(e) = snapshot {
        let _ = host.service("start");
        return Err(e);
    }
    let attempt = (|| -> Result<()> {
        let migrated = job.join("settings.json");
        state.warnings = host.migrate(
            &target.join("bin/ungoggled"),
            &paths.at("var/lib/dji-hdmi"),
            &migrated,
        )?;
        atomic_copy(&migrated, &paths.at(FILES[2]), 0o600)?;
        atomic_copy(&source.join("dji-hdmi.service"), &paths.at(FILES[0]), 0o644)?;
        atomic_copy(&source.join("prepare-pi.sh"), &paths.at(FILES[1]), 0o755)?;
        if !paths.at("etc/default/dji-hdmi").exists() {
            fs::create_dir_all(paths.at("etc/default"))?;
            crate::settings::atomic_write(
                &paths.at("etc/default/dji-hdmi"),
                b"DJI_HDMI_DECODER=v4l2h264dec\n",
            )?;
        }
        set_current(paths, &target)?;
        state.phase = Phase::Restarting;
        state.message = "Restarting and checking the new release".into();
        paths.save(&state)?;
        host.service("daemon-reload")?;
        host.service("enable")?;
        host.service("start")?;
        ensure!(
            host.healthy(&manifest.version, &build),
            "New release did not pass its health check"
        );
        // Only a healthy application becomes the recovery helper for the next update.
        atomic_copy(
            &target.join("bin/ungoggled"),
            &paths.at("usr/local/lib/dji-hdmi/update-helper"),
            0o755,
        )?;
        state.phase = Phase::Succeeded;
        state.message = format!("Updated to {}", manifest.version);
        paths.save(&state)?;
        Ok(())
    })();
    if let Err(error) = attempt {
        if let Err(rollback) = restore(paths, &job, host, false) {
            bail!("Update failed: {error:#}. Automatic rollback failed: {rollback:#}");
        }
        state.phase = Phase::RolledBack;
        state.message = format!("Update failed; previous version and settings restored: {error:#}");
        paths.save(&state)?;
        return Err(error);
    }
    // The success record is the commit point. Cleanup cannot roll it back.
    let _ = remove_file(&job.join("journal.json"));
    // Keep the active and preceding releases; bounded storage matters on small cards.
    for entry in fs::read_dir(paths.at("opt/dji-hdmi/releases"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        let managed = name.rsplit_once('-').is_some_and(|(version, hash)| {
            semver::Version::parse(version).is_ok()
                && hash.len() == 12
                && hash.bytes().all(|b| b.is_ascii_hexdigit())
        });
        if managed
            && entry.file_type().is_ok_and(|t| t.is_dir())
            && entry.path() != target
            && Some(entry.path()) != previous
        {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
    let _ = fs::remove_dir_all(source);
    Ok(())
}
fn restore(paths: &Paths, job: &Path, host: &impl Host, boot: bool) -> Result<()> {
    let journal: Journal = serde_json::from_slice(&fs::read(job.join("journal.json"))?)?;
    ensure!(
        journal.files.len() == FILES.len(),
        "Unknown recovery journal format"
    );
    if !boot {
        host.service("stop")?;
    }
    for (i, name) in FILES.iter().enumerate() {
        if journal.files[i] {
            atomic_copy(
                &job.join("backup").join(i.to_string()),
                &paths.at(name),
                if i == 1 {
                    0o755
                } else if i == 2 {
                    0o600
                } else {
                    0o644
                },
            )?;
        } else {
            remove_file(&paths.at(name))?;
        }
    }
    match journal.previous {
        Some(previous) => set_current(paths, &previous)?,
        None => remove_file(&paths.at(CURRENT))?,
    }
    host.service("daemon-reload")?;
    if !boot {
        host.service("start")?;
    }
    remove_file(&job.join("journal.json"))?;
    Ok(())
}
pub fn recover() -> Result<()> {
    let paths = Paths::default();
    let _lock = Lock::acquire(&paths)?;
    recover_with(&paths, &System)
}
fn recover_with(paths: &Paths, host: &impl Host) -> Result<()> {
    let mut state = paths.state()?;
    let Some(id) = &state.id else { return Ok(()) };
    let job = paths.job(id)?;
    if state.phase == Phase::Succeeded {
        remove_file(&job.join("journal.json"))?;
        return Ok(());
    }
    if job.join("journal.json").exists() {
        restore(paths, &job, host, true)?;
        state.phase = Phase::RolledBack;
        state.message =
            "Interrupted update recovered; previous version and settings restored".into();
        paths.save(&state)?;
    } else if state.phase.busy() {
        state.phase = Phase::Failed;
        state.message =
            "Update interrupted before installation; previous application retained".into();
        paths.save(&state)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::{
        State,
        tests::{Fixture, bundle},
    };
    use std::cell::RefCell;
    struct FakeHost {
        healthy: bool,
        calls: RefCell<Vec<String>>,
    }
    impl Host for FakeHost {
        fn service(&self, verb: &str) -> Result<()> {
            self.calls.borrow_mut().push(verb.into());
            Ok(())
        }
        fn migrate(&self, _: &Path, data: &Path, output: &Path) -> Result<Vec<String>> {
            crate::settings::migrate(data, output)
        }
        fn healthy(&self, _: &str, _: &str) -> bool {
            self.healthy
        }
    }
    fn setup() -> (Fixture, Paths, PathBuf) {
        let f = Fixture::new();
        let paths = f.paths();
        let old = paths.at("opt/dji-hdmi/releases/old");
        fs::create_dir_all(&old).unwrap();
        set_current(&paths, &old).unwrap();
        for name in FILES {
            let path = paths.at(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, if name.ends_with(".json") { r#"{"hdmi_mode":"1920x1080@30","preview_enabled":false,"fallback_image":null,"retired_option":true}"# } else { "old file" }).unwrap();
        }
        for name in [
            "etc/default/dji-hdmi",
            "etc/shadow",
            "etc/NetworkManager/system-connections/dji-hdmi.nmconnection",
            "var/lib/dji-hdmi/images/custom.png",
        ] {
            let p = paths.at(name);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, "preserve exactly").unwrap();
        }
        bundle(&paths.job("123").unwrap().join("package"));
        paths
            .save(&State {
                id: Some("123".into()),
                phase: Phase::Ready,
                ..State::default()
            })
            .unwrap();
        (f, paths, old)
    }
    #[test]
    fn commits_healthy_release_and_preserves_configuration() {
        let (_f, paths, old) = setup();
        let host = FakeHost {
            healthy: true,
            calls: Default::default(),
        };
        apply(&paths, "123", "aarch64", &host).unwrap();
        assert_ne!(fs::read_link(paths.at(CURRENT)).unwrap(), old);
        assert!(old.exists());
        let state = paths.state().unwrap();
        assert_eq!(state.phase, Phase::Succeeded);
        assert_eq!(state.warnings.len(), 1);
        let settings: serde_json::Value =
            serde_json::from_slice(&fs::read(paths.at(FILES[2])).unwrap()).unwrap();
        assert_eq!(settings["hdmi_mode"], "1920x1080@30");
        assert_eq!(settings["preview_enabled"], false);
        for name in [
            "etc/default/dji-hdmi",
            "etc/shadow",
            "etc/NetworkManager/system-connections/dji-hdmi.nmconnection",
            "var/lib/dji-hdmi/images/custom.png",
        ] {
            assert_eq!(
                fs::read_to_string(paths.at(name)).unwrap(),
                "preserve exactly"
            );
        }
        assert!(!paths.job("123").unwrap().join("journal.json").exists());
    }
    #[test]
    fn failed_health_check_restores_previous_release_and_exact_settings() {
        let (_f, paths, old) = setup();
        let before = fs::read(paths.at(FILES[2])).unwrap();
        let host = FakeHost {
            healthy: false,
            calls: Default::default(),
        };
        assert!(apply(&paths, "123", "aarch64", &host).is_err());
        assert_eq!(paths.state().unwrap().phase, Phase::RolledBack);
        assert_eq!(fs::read_link(paths.at(CURRENT)).unwrap(), old);
        assert_eq!(fs::read(paths.at(FILES[2])).unwrap(), before);
        assert_eq!(fs::read_to_string(paths.at(FILES[0])).unwrap(), "old file");
        assert_eq!(fs::read_to_string(paths.at(FILES[1])).unwrap(), "old file");
        assert_eq!(host.calls.borrow().last().unwrap(), "start");
    }
    #[test]
    fn corrupt_settings_abort_and_restore() {
        let (_f, paths, old) = setup();
        fs::write(paths.at(FILES[2]), "invalid JSON").unwrap();
        let host = FakeHost {
            healthy: true,
            calls: Default::default(),
        };
        assert!(apply(&paths, "123", "aarch64", &host).is_err());
        assert_eq!(paths.state().unwrap().phase, Phase::RolledBack);
        assert_eq!(fs::read_link(paths.at(CURRENT)).unwrap(), old);
        assert_eq!(
            fs::read_to_string(paths.at(FILES[2])).unwrap(),
            "invalid JSON"
        );
    }
    #[test]
    fn boot_recovers_uncommitted_transaction_without_starting_dependent_service() {
        let (_f, paths, old) = setup();
        let job = paths.job("123").unwrap();
        fs::create_dir(job.join("backup")).unwrap();
        for (i, name) in FILES.iter().enumerate() {
            fs::copy(paths.at(name), job.join("backup").join(i.to_string())).unwrap();
            fs::write(paths.at(name), "interrupted replacement").unwrap();
        }
        let target = paths.at("opt/dji-hdmi/releases/interrupted");
        set_current(&paths, &target).unwrap();
        fs::write(
            job.join("journal.json"),
            serde_json::to_vec(&Journal {
                previous: Some(old.clone()),
                files: vec![true; FILES.len()],
                target,
            })
            .unwrap(),
        )
        .unwrap();
        let mut state = paths.state().unwrap();
        state.phase = Phase::Restarting;
        paths.save(&state).unwrap();
        let host = FakeHost {
            healthy: true,
            calls: Default::default(),
        };
        recover_with(&paths, &host).unwrap();
        assert_eq!(fs::read_link(paths.at(CURRENT)).unwrap(), old);
        assert_eq!(paths.state().unwrap().phase, Phase::RolledBack);
        assert_eq!(*host.calls.borrow(), vec!["daemon-reload"]);
        assert!(!job.join("journal.json").exists());
    }
    #[test]
    fn committed_transaction_never_rolls_back_during_boot_cleanup() {
        let (_f, paths, old) = setup();
        let job = paths.job("123").unwrap();
        fs::write(job.join("journal.json"), "cleanup pending").unwrap();
        let mut state = paths.state().unwrap();
        state.phase = Phase::Succeeded;
        paths.save(&state).unwrap();
        let host = FakeHost {
            healthy: true,
            calls: Default::default(),
        };
        recover_with(&paths, &host).unwrap();
        assert_eq!(paths.state().unwrap().phase, Phase::Succeeded);
        assert_eq!(fs::read_link(paths.at(CURRENT)).unwrap(), old);
        assert!(host.calls.borrow().is_empty());
        assert!(!job.join("journal.json").exists());
    }
}
