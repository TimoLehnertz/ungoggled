//! Piezo beeper and the power/shutdown button.
//!
//! Defaults match the NuclearHazard V5 hat: GPIO19 for its power button
//! (active low, matching the pin the RotorHazard installer wires for
//! "nuclear" boards). The hat's own beeper is soldered to its onboard STM32
//! co-processor and driven over the RotorHazard node protocol on the UART
//! pins (GPIO14/15), not a plain Pi GPIO line, so there is no default beeper
//! pin: wire a separate piezo to any free GPIO and configure it in the web
//! interface.
use crate::settings::Store;
use rppal::gpio::{Gpio, InputPin, OutputPin};
use serde_json::Value;
use std::{
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

pub const DEFAULT_POWER_BUTTON_PIN: u8 = 19;

pub type StatusFn = Arc<dyn Fn() -> Value + Send + Sync>;

#[derive(Clone, Copy)]
struct Tone {
    hz: u32,
    on_ms: u32,
    off_ms: u32,
}
type Pattern = &'static [Tone];

const BOOT: Pattern = &[
    Tone {
        hz: 988,
        on_ms: 70,
        off_ms: 25,
    },
    Tone {
        hz: 1245,
        on_ms: 70,
        off_ms: 25,
    },
    Tone {
        hz: 1568,
        on_ms: 130,
        off_ms: 0,
    },
];
const GOGGLES_UP: Pattern = &[
    Tone {
        hz: 988,
        on_ms: 60,
        off_ms: 35,
    },
    Tone {
        hz: 1319,
        on_ms: 100,
        off_ms: 0,
    },
];
const GOGGLES_DOWN: Pattern = &[
    Tone {
        hz: 988,
        on_ms: 80,
        off_ms: 35,
    },
    Tone {
        hz: 659,
        on_ms: 140,
        off_ms: 0,
    },
];
const VIDEO_UP: Pattern = &[Tone {
    hz: 1568,
    on_ms: 140,
    off_ms: 0,
}];
const VIDEO_DOWN: Pattern = &[Tone {
    hz: 494,
    on_ms: 200,
    off_ms: 0,
}];
const HDMI_UP: Pattern = &[
    Tone {
        hz: 1245,
        on_ms: 55,
        off_ms: 35,
    },
    Tone {
        hz: 1245,
        on_ms: 55,
        off_ms: 35,
    },
    Tone {
        hz: 1568,
        on_ms: 90,
        off_ms: 0,
    },
];
const HDMI_DOWN: Pattern = &[
    Tone {
        hz: 784,
        on_ms: 55,
        off_ms: 35,
    },
    Tone {
        hz: 784,
        on_ms: 55,
        off_ms: 35,
    },
    Tone {
        hz: 392,
        on_ms: 130,
        off_ms: 0,
    },
];
const BUTTON_PRESSED: Pattern = &[Tone {
    hz: 1200,
    on_ms: 120,
    off_ms: 0,
}];
const TEST: Pattern = &[
    Tone {
        hz: 880,
        on_ms: 120,
        off_ms: 70,
    },
    Tone {
        hz: 880,
        on_ms: 120,
        off_ms: 0,
    },
];

// Phases the gadget only reaches once the accessory handshake has succeeded.
// Mirrors web/src/App.tsx's `usbPhases` so both sides agree on "attached".
const USB_PHASES: [&str; 5] = [
    "usb_connected",
    "accessory",
    "aoa_negotiation",
    "waiting_video",
    "streaming",
];
fn goggles_attached(status: &Value) -> bool {
    match status["usb_state"].as_str() {
        Some(state) => state != "not attached",
        None => status["phase"]
            .as_str()
            .is_some_and(|p| USB_PHASES.contains(&p)),
    }
}
fn video_live(status: &Value) -> bool {
    status["phase"].as_str() == Some("streaming")
        && status["bitrate_mbps"].as_f64().unwrap_or(0.0) > 0.0
}
fn hdmi_connected(status: &Value) -> bool {
    status["hdmi_connected"].as_bool().unwrap_or(false)
}

// Opens (or drops) a pin when the desired GPIO changes, reporting the
// outcome under `error_key` in the shared status so failures are visible in
// the web UI without needing a shell on the Pi.
fn reconfigure<T>(
    wanted: Option<u8>,
    open: impl FnOnce(u8) -> Result<T, String>,
    report: &crate::video::Report,
    error_key: &str,
) -> Option<T> {
    let (value, error) = match wanted.map(open) {
        None => (None, Value::Null),
        Some(Ok(v)) => (Some(v), Value::Null),
        Some(Err(e)) => (None, Value::String(e)),
    };
    let mut event = serde_json::Map::new();
    event.insert(error_key.to_string(), error);
    report(Value::Object(event));
    value
}

pub fn start(
    status: StatusFn,
    store: Store,
    shutdown: Arc<AtomicBool>,
    test_beep: Arc<AtomicBool>,
    report: crate::video::Report,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let Ok(gpio) = Gpio::new() else {
            eprintln!("GPIO unavailable; beeper, LEDs and power button are disabled this run");
            for key in [
                "beeper_error",
                "power_button_error",
                "goggles_led_error",
                "video_led_error",
                "hdmi_led_error",
            ] {
                let mut event = serde_json::Map::new();
                event.insert(key.to_string(), Value::String("GPIO unavailable".into()));
                report(Value::Object(event));
            }
            return;
        };
        let mut beeper_want: Option<u8> = None;
        let mut beeper: Option<OutputPin> = None;
        let mut button_want: Option<u8> = None;
        let mut button: Option<InputPin> = None;
        let mut goggles_led_want: Option<u8> = None;
        let mut goggles_led: Option<OutputPin> = None;
        let mut video_led_want: Option<u8> = None;
        let mut video_led: Option<OutputPin> = None;
        let mut hdmi_led_want: Option<u8> = None;
        let mut hdmi_led: Option<OutputPin> = None;
        let mut prev_goggles: Option<bool> = None;
        let mut prev_video: Option<bool> = None;
        let mut prev_hdmi: Option<bool> = None;
        let mut low_ticks = 0u8;
        let mut shutdown_triggered = false;
        let mut booted = false;
        while !shutdown.load(Ordering::Relaxed) {
            let settings = store.get();
            if settings.beeper_pin != beeper_want {
                beeper_want = settings.beeper_pin;
                beeper = reconfigure(
                    beeper_want,
                    |p| open_output(&gpio, p, "beeper"),
                    &report,
                    "beeper_error",
                );
            }
            if settings.power_button_pin != button_want {
                button_want = settings.power_button_pin;
                button = reconfigure(
                    button_want,
                    |p| open_input_pullup(&gpio, p, "power button"),
                    &report,
                    "power_button_error",
                );
                low_ticks = 0;
            }
            if settings.goggles_led_pin != goggles_led_want {
                goggles_led_want = settings.goggles_led_pin;
                goggles_led = reconfigure(
                    goggles_led_want,
                    |p| open_output(&gpio, p, "goggles LED"),
                    &report,
                    "goggles_led_error",
                );
            }
            if settings.video_led_pin != video_led_want {
                video_led_want = settings.video_led_pin;
                video_led = reconfigure(
                    video_led_want,
                    |p| open_output(&gpio, p, "video LED"),
                    &report,
                    "video_led_error",
                );
            }
            if settings.hdmi_led_pin != hdmi_led_want {
                hdmi_led_want = settings.hdmi_led_pin;
                hdmi_led = reconfigure(
                    hdmi_led_want,
                    |p| open_output(&gpio, p, "HDMI LED"),
                    &report,
                    "hdmi_led_error",
                );
            }
            if !booted {
                booted = true;
                if let Some(pin) = &mut beeper {
                    play(pin, BOOT);
                }
            }
            if test_beep.swap(false, Ordering::Relaxed)
                && let Some(pin) = &mut beeper
            {
                play(pin, TEST);
            }

            let s = status();
            let goggles = goggles_attached(&s);
            let video = video_live(&s);
            let hdmi = hdmi_connected(&s);
            if let Some(pin) = &mut beeper {
                if prev_goggles.is_some_and(|prev| prev != goggles) {
                    play(pin, if goggles { GOGGLES_UP } else { GOGGLES_DOWN });
                }
                if prev_video.is_some_and(|prev| prev != video) {
                    play(pin, if video { VIDEO_UP } else { VIDEO_DOWN });
                }
                if prev_hdmi.is_some_and(|prev| prev != hdmi) {
                    play(pin, if hdmi { HDMI_UP } else { HDMI_DOWN });
                }
            }
            prev_goggles = Some(goggles);
            prev_video = Some(video);
            prev_hdmi = Some(hdmi);

            for (pin, on) in [
                (&mut goggles_led, goggles),
                (&mut video_led, video),
                (&mut hdmi_led, hdmi),
            ] {
                if let Some(pin) = pin {
                    if on {
                        pin.set_high();
                    } else {
                        pin.set_low();
                    }
                }
            }

            if let Some(pin) = &button {
                low_ticks = if pin.is_low() {
                    low_ticks.saturating_add(1)
                } else {
                    0
                };
                if low_ticks == 3 && !shutdown_triggered {
                    shutdown_triggered = true;
                    if let Some(pin) = &mut beeper {
                        play(pin, BUTTON_PRESSED);
                    }
                    let _ = Command::new("systemctl").arg("poweroff").spawn();
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
    })
}
fn open_output(gpio: &Gpio, pin: u8, name: &str) -> Result<OutputPin, String> {
    match gpio.get(pin) {
        Ok(p) => {
            let mut out = p.into_output();
            out.set_low();
            Ok(out)
        }
        Err(e) => {
            let msg = format!("Cannot claim GPIO{pin} for the {name}: {e}");
            eprintln!("{msg}");
            Err(msg)
        }
    }
}
fn open_input_pullup(gpio: &Gpio, pin: u8, name: &str) -> Result<InputPin, String> {
    match gpio.get(pin) {
        Ok(p) => Ok(p.into_input_pullup()),
        Err(e) => {
            let msg = format!("Cannot claim GPIO{pin} for the {name}: {e}");
            eprintln!("{msg}");
            Err(msg)
        }
    }
}
fn play(pin: &mut OutputPin, pattern: Pattern) {
    for tone in pattern {
        square_wave(pin, tone.hz, tone.on_ms);
        pin.set_low();
        if tone.off_ms > 0 {
            thread::sleep(Duration::from_millis(tone.off_ms as u64));
        }
    }
}
fn square_wave(pin: &mut OutputPin, hz: u32, ms: u32) {
    let half = Duration::from_secs_f64(0.5 / hz.max(1) as f64);
    let end = Instant::now() + Duration::from_millis(ms as u64);
    let mut high = false;
    while Instant::now() < end {
        high = !high;
        if high {
            pin.set_high();
        } else {
            pin.set_low();
        }
        thread::sleep(half);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn derives_states_from_the_same_fields_the_web_header_uses() {
        assert!(goggles_attached(&json!({"usb_state":"configured"})));
        assert!(!goggles_attached(&json!({"usb_state":"not attached"})));
        assert!(goggles_attached(&json!({"phase":"streaming"})));
        assert!(!goggles_attached(&json!({"phase":"stopped"})));
        assert!(video_live(&json!({"phase":"streaming","bitrate_mbps":5.0})));
        assert!(!video_live(
            &json!({"phase":"streaming","bitrate_mbps":0.0})
        ));
        assert!(!video_live(
            &json!({"phase":"waiting_video","bitrate_mbps":5.0})
        ));
        assert!(hdmi_connected(&json!({"hdmi_connected":true})));
        assert!(!hdmi_connected(&json!({})));
    }
}
