//! Set the HDMI scanout mode independently of the changing camera resolution.
use anyhow::{Context, Result, bail};
use drm::{
    Device as _,
    buffer::DrmFourcc,
    control::{Device as _, Mode, ModeFlags, connector},
};
use std::{
    fs::{self, File, OpenOptions},
    os::fd::{AsFd, BorrowedFd},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

struct Card(File);
impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}
impl drm::Device for Card {}
impl drm::control::Device for Card {}

pub struct Display {
    _card: Arc<Card>,
    upgrade: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    // The DRM file owns the black primary framebuffer and its backing buffer.
    // Keep it open across decoder restarts; kmssink owns only the video overlay.
}

fn select_mode(modes: &[Mode]) -> Option<Mode> {
    let progressive = |m: &&Mode| !m.flags().contains(ModeFlags::INTERLACE);
    modes
        .iter()
        .filter(progressive)
        .filter(|m| m.size() == (1920, 1080) && m.vrefresh() <= 60)
        .max_by_key(|m| m.vrefresh())
        .copied()
        .or_else(|| {
            modes
                .iter()
                .filter(progressive)
                .filter(|m| m.size().0 <= 1920 && m.size().1 <= 1080)
                .max_by_key(|m| (u32::from(m.size().0) * u32::from(m.size().1), m.vrefresh()))
                .copied()
        })
}

impl Display {
    pub fn prepare(requested: Option<u32>) -> Result<Self> {
        let mut paths: Vec<_> = fs::read_dir("/dev/dri")?
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("card"))
            .map(|e| e.path())
            .collect();
        paths.sort();
        for path in paths {
            let card = Card(OpenOptions::new().read(true).write(true).open(path)?);
            let Ok(resources) = card.resource_handles() else {
                continue;
            };
            for &handle in resources.connectors() {
                let con = card.get_connector(handle, true)?;
                if requested.is_some_and(|id| id != u32::from(handle)) {
                    continue;
                }
                if requested.is_none()
                    && (con.interface() != connector::Interface::HDMIA || con.interface_id() != 1)
                {
                    continue;
                }
                if con.state() != connector::State::Connected {
                    continue;
                }
                let mode =
                    select_mode(con.modes()).context("No supported progressive HDMI mode")?;
                let encoder = con
                    .current_encoder()
                    .or_else(|| con.encoders().first().copied())
                    .context("HDMI encoder missing")?;
                let encoder = card.get_encoder(encoder)?;
                let crtc = encoder
                    .crtc()
                    .or_else(|| {
                        resources
                            .filter_crtcs(encoder.possible_crtcs())
                            .first()
                            .copied()
                    })
                    .context("HDMI CRTC missing")?;
                let current = card.get_crtc(crtc)?;
                if current.mode() != Some(mode) {
                    card.acquire_master_lock()
                        .context("Acquire DRM for HDMI mode change")?;
                    let result = (|| -> Result<()> {
                        let (w, h) = mode.size();
                        let mut buffer =
                            card.create_dumb_buffer((w.into(), h.into()), DrmFourcc::Xrgb8888, 32)?;
                        card.map_dumb_buffer(&mut buffer)?.as_mut().fill(0);
                        let fb = card.add_framebuffer(&buffer, 24, 32)?;
                        card.set_crtc(crtc, Some(fb), (0, 0), &[handle], Some(mode))?;
                        Ok(())
                    })();
                    let release = card.release_master_lock();
                    result.context("Set HDMI mode")?;
                    release?;
                } else {
                    // Opening the first card fd can itself confer DRM master.
                    let _ = card.release_master_lock();
                }
                println!(
                    "{}",
                    serde_json::json!({"hdmi_width":mode.size().0,
                    "hdmi_height":mode.size().1,"hdmi_hz":mode.vrefresh()})
                );
                let card = Arc::new(card);
                let upgrade = Arc::new(AtomicBool::new(false));
                let stop = Arc::new(AtomicBool::new(false));
                if mode.size() != (1920, 1080) {
                    // EDID probing may block for a second on this legacy kernel.
                    // Keep it off the decoder thread and reuse the same DRM fd.
                    let probe = card.clone();
                    let pending = upgrade.clone();
                    let stopped = stop.clone();
                    thread::spawn(move || {
                        while !stopped.load(Ordering::Relaxed) {
                            thread::sleep(Duration::from_secs(1));
                            if stopped.load(Ordering::Relaxed) {
                                break;
                            }
                            if probe
                                .get_connector(handle, true)
                                .ok()
                                .and_then(|c| select_mode(c.modes()))
                                .is_some_and(|m| m.size() == (1920, 1080))
                            {
                                pending.store(true, Ordering::Relaxed);
                                break;
                            }
                        }
                    });
                }
                return Ok(Self {
                    _card: card,
                    upgrade,
                    stop,
                });
            }
        }
        bail!("No connected HDMI0 connector")
    }

    pub fn needs_upgrade(&self) -> bool {
        // A headless boot may expose only VGA. Upgrade when EDID arrives, but
        // keep full HD scanout stable when the monitor is temporarily unplugged.
        self.upgrade.load(Ordering::Relaxed)
    }
}
impl Drop for Display {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
