use crate::WorkerArgs;
use std::{
    io::{BufRead, BufReader, Write},
    os::fd::AsRawFd,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::{SyncSender, TrySendError, sync_channel},
    },
    thread,
    time::{Duration, Instant},
};

struct Chunk {
    data: Vec<u8>,
    received: Instant,
}

pub struct Output {
    tx: SyncSender<Chunk>,
    discontinuity: Arc<AtomicBool>,
    queued_bytes: Arc<AtomicUsize>,
}
impl Output {
    pub fn send(&self, payload: Vec<u8>) {
        // USB reception must not wait for HDMI. Cap both message count and bytes.
        let len = payload.len();
        let previous = self.queued_bytes.fetch_add(len, Ordering::Relaxed);
        if previous.saturating_add(len) > 2 * 1024 * 1024 {
            self.queued_bytes.fetch_sub(len, Ordering::Relaxed);
            self.discontinuity.store(true, Ordering::Relaxed);
            return;
        }
        if let Err(e) = self.tx.try_send(Chunk {
            data: payload,
            received: Instant::now(),
        }) {
            self.queued_bytes.fetch_sub(len, Ordering::Relaxed);
            if matches!(e, TrySendError::Full(_)) {
                self.discontinuity.store(true, Ordering::Relaxed);
            }
        }
    }
}
struct Player(Child, Arc<AtomicU64>, Instant);
impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub fn start(args: &WorkerArgs) -> Output {
    let (tx, rx) = sync_channel::<Chunk>(1024);
    let discontinuity = Arc::new(AtomicBool::new(false));
    let reset = discontinuity.clone();
    let queued_bytes = Arc::new(AtomicUsize::new(0));
    let queued = queued_bytes.clone();
    let args = args.clone();
    thread::spawn(move || {
        let mut child: Option<Player> = None;
        let mut annex = crate::h264::AnnexB::default();
        let mut parameters = crate::h264::Parameters::default();
        let mut display: Option<crate::display::Display> = None;
        let mut display_check = Instant::now();
        let mut last_input: Option<Instant> = None;
        let mut decoder_starts = 0_u64;
        for chunk in &rx {
            let data = chunk.data;
            queued.fetch_sub(data.len(), Ordering::Relaxed);
            if args.output == "none" {
                continue;
            }
            if last_input.is_some_and(|last| {
                chunk.received.duration_since(last) > Duration::from_millis(750)
            }) {
                // A transmission-mode change can interrupt a NAL halfway through.
                // Never concatenate data from before and after that interruption.
                eprintln!("Camera input interrupted for more than 750 ms; waiting for fresh SPS");
                child = None;
                annex = crate::h264::AnnexB::default();
            }
            last_input = Some(chunk.received);
            if child.as_ref().is_some_and(|c| {
                let elapsed = c.2.elapsed().as_millis() as u64;
                elapsed > 3000 && elapsed.saturating_sub(c.1.load(Ordering::Relaxed)) > 1500
            }) {
                eprintln!("Decoder stopped producing frames; waiting for fresh SPS");
                child = None;
            }
            if reset.swap(false, Ordering::Relaxed) {
                child = None;
                annex = crate::h264::AnnexB::default();
                eprintln!("HDMI queue overrun; waiting for fresh SPS");
                while let Ok(data) = rx.try_recv() {
                    queued.fetch_sub(data.data.len(), Ordering::Relaxed);
                }
                continue;
            }
            if args.output == "hdmi" && display_check.elapsed() > Duration::from_secs(1) {
                display_check = Instant::now();
                if display.as_ref().is_some_and(|d| d.needs_upgrade()) {
                    eprintln!("Monitor now advertises 1080p; upgrading HDMI output");
                    child = None;
                    display = None;
                }
            }
            let units = match annex.feed(&data) {
                Ok(units) => units,
                Err(e) => {
                    child = None;
                    eprintln!("{e}");
                    continue;
                }
            };
            for data in units {
                if parameters.changed(&data) {
                    eprintln!("Camera SPS changed; restarting decoder");
                    child = None;
                    println!(
                        "{}",
                        serde_json::json!({"hdmi":"reconfiguring",
                        "message":"Camera stream parameters changed; restarting decoder"})
                    );
                }
                if child
                    .as_mut()
                    .is_some_and(|c| c.0.try_wait().ok().flatten().is_some())
                {
                    child = None;
                }
                let payload = if child.is_none() {
                    if data.get(3).map(|b| b & 31) != Some(7) {
                        continue;
                    }
                    if args.output == "hdmi" && display.is_none() {
                        match crate::display::Display::prepare(args.connector) {
                            Ok(d) => display = Some(d),
                            Err(e) => eprintln!("HDMI mode: {e:#}"),
                        }
                    }
                    let mut cmd = Command::new("gst-launch-1.0");
                    cmd.args([
                        "-v",
                        "fdsrc",
                        "fd=0",
                        "blocksize=65536",
                        "do-timestamp=true",
                        "!",
                        "h264parse",
                        "config-interval=-1",
                        "!",
                        "video/x-h264,stream-format=byte-stream,alignment=au",
                        "!",
                        &args.decoder,
                        "!",
                        "queue",
                        "max-size-buffers=2",
                        "max-size-bytes=0",
                        "max-size-time=0",
                        "leaky=downstream",
                        "!",
                    ]);
                    let sink = if args.output == "hdmi" {
                        let mut sink = "kmssink sync=false".to_string();
                        if let Some(id) = args.connector {
                            sink.push_str(&format!(" connector-id={id}"));
                        }
                        sink
                    } else {
                        "fakesink sync=false".to_string()
                    };
                    cmd.args([
                        "fpsdisplaysink",
                        &format!("video-sink={sink}"),
                        "text-overlay=false",
                        "sync=false",
                        "fps-update-interval=250",
                    ]);
                    match cmd
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::inherit())
                        .spawn()
                    {
                        Ok(mut c) => {
                            let fd = c.stdin.as_ref().unwrap().as_raw_fd();
                            let nonblocking = unsafe {
                                let flags = libc::fcntl(fd, libc::F_GETFL);
                                flags >= 0
                                    && libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) >= 0
                            };
                            if !nonblocking {
                                let _ = c.kill();
                                let _ = c.wait();
                                eprintln!("Cannot make decoder pipe nonblocking");
                                continue;
                            }
                            let bus = c.stdout.take().unwrap();
                            let began = Instant::now();
                            let heartbeat = Arc::new(AtomicU64::new(0));
                            let observed = heartbeat.clone();
                            thread::spawn(move || {
                                for line in BufReader::new(bus).lines().map_while(Result::ok) {
                                    if let Some((width, height)) = input_size(&line) {
                                        println!(
                                            "{}",
                                            serde_json::json!({"input_width":width,"input_height":height})
                                        );
                                    }
                                    if let Some((frames, fps)) = frame_stats(&line) {
                                        observed.store(
                                            began.elapsed().as_millis() as u64,
                                            Ordering::Relaxed,
                                        );
                                        println!(
                                            "{}",
                                            serde_json::json!({
                                                "hdmi":"playing", "output_frames":frames,
                                                "output_fps":fps, "message":"Video frames reaching output"
                                            })
                                        );
                                    }
                                }
                            });
                            child = Some(Player(c, heartbeat, began));
                            decoder_starts += 1;
                            println!(
                                "{}",
                                serde_json::json!({"hdmi":"starting", "decoder_starts":decoder_starts})
                            );
                        }
                        Err(e) => {
                            eprintln!("Start video decoder: {e}");
                            println!(
                                "{}",
                                serde_json::json!({"hdmi":"error","message":e.to_string()})
                            );
                            continue;
                        }
                    }
                    data
                } else {
                    data
                };
                if let Some(c) = child.as_mut()
                    && let Err(e) = write_with_deadline(c.0.stdin.as_mut().unwrap(), &payload)
                {
                    eprintln!("Video pipeline: {e}");
                    child = None;
                    println!(
                        "{}",
                        serde_json::json!({"hdmi":"error","message":e.to_string()})
                    );
                }
            }
        }
    });
    Output {
        tx,
        discontinuity,
        queued_bytes,
    }
}

fn write_with_deadline(writer: &mut impl Write, mut bytes: &[u8]) -> std::io::Result<()> {
    let started = Instant::now();
    while !bytes.is_empty() {
        match writer.write(bytes) {
            Ok(0) => return Err(std::io::ErrorKind::WriteZero.into()),
            Ok(n) => bytes = &bytes[n..],
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if started.elapsed() > Duration::from_secs(3) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "Decoder stalled for 3 seconds",
                    ));
                }
                thread::sleep(Duration::from_millis(2));
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

// fpsdisplaysink reports rendered buffers, unlike pipeline state alone, which
// can say PLAYING even when the decoder never produces an image.
fn frame_stats(line: &str) -> Option<(u64, f64)> {
    let (_, rest) = line.split_once("last-message = rendered: ")?;
    let frames = rest.split(',').next()?.trim().parse().ok()?;
    let (_, rest) = rest.split_once("current: ")?;
    let fps: f64 = rest.split(',').next()?.trim().parse().ok()?;
    fps.is_finite().then_some((frames, fps))
}

fn input_size(line: &str) -> Option<(u32, u32)> {
    if !line.contains("h264parse0.GstPad:src: caps =") {
        return None;
    }
    let number = |name: &str| -> Option<u32> {
        line.split_once(name)?
            .1
            .split(',')
            .next()?
            .trim()
            .parse()
            .ok()
    };
    Some((number("width=(int)")?, number("height=(int)")?))
}

#[cfg(test)]
mod tests {
    use super::frame_stats;
    #[test]
    fn output_stats_require_rendered_frames() {
        assert_eq!(frame_stats("New clock: GstSystemClock"), None);
        assert_eq!(
            frame_stats(
                "fpsdisplaysink0: last-message = rendered: 42, dropped: 0, current: 29.98, average: 30.01"
            ),
            Some((42, 29.98))
        );
        assert_eq!(
            frame_stats("last-message = rendered: 0, current: NaN"),
            None
        );
    }
}
