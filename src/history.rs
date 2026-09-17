use serde_json::{Value, json};
use std::{collections::VecDeque, time::Instant};

pub const CAPACITY: usize = 1800;
pub struct History {
    started: Instant,
    samples: VecDeque<Value>,
}
impl History {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            samples: VecDeque::with_capacity(CAPACITY),
        }
    }
    pub fn push(&mut self, s: &Value) {
        if self.samples.len() == CAPACITY {
            self.samples.pop_front();
        }
        self.samples.push_back(json!({
            "t":self.started.elapsed().as_secs_f64(), "bitrate_mbps":s["bitrate_mbps"],
            "input_fps":s["input_fps"], "output_fps":s["output_fps"],
            "temperature_c":s["temperature_c"], "phase":s["phase"], "hdmi":s["hdmi"],
            "discarded_bytes":s["discarded_bytes"], "decoder_starts":s["decoder_starts"]
        }));
    }
    pub fn snapshot(&self) -> Value {
        json!({"duration_seconds":1800, "sample_interval_seconds":1, "now":self.started.elapsed().as_secs_f64(), "samples":self.samples})
    }
}
pub fn temperature() -> Option<f64> {
    let n: f64 = std::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
        .ok()?
        .trim()
        .parse()
        .ok()?;
    (n.is_finite() && (0.0..150_000.0).contains(&n)).then_some(n / 1000.0)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_is_bounded_and_keeps_latest() {
        let mut h = History::new();
        for n in 0..2000 {
            h.push(&json!({"bitrate_mbps":n}));
        }
        assert_eq!(h.samples.len(), 1800);
        assert_eq!(h.samples.front().unwrap()["bitrate_mbps"], 200);
        assert_eq!(h.samples.back().unwrap()["bitrate_mbps"], 1999);
    }
}
