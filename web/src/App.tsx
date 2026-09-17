import { useEffect, useState } from "react";
import type { Status, Settings, ImageEntry, History } from "./types";
import { api, headers, usePolling } from "./api";
import { number, size } from "./format";
import { Preview } from "./components/Preview";
import { Timeline } from "./components/Timeline";
import { WifiPanel } from "./components/WifiPanel";
const phases: Record<string, string> = {
  streaming: "Receiving video",
  waiting_video: "Waiting for camera",
  waiting_usb: "Waiting for goggles",
  stopped: "Stopped",
  starting: "Starting",
  retrying: "Reconnecting",
  accessory: "Goggles connected",
  aoa_negotiation: "Connecting goggles",
  usb_connected: "Goggles connected",
};

export default function App() {
  const { value: status, error: connectionError } = usePolling<Status>(
    "status",
    1000,
  );
  const { value: history } = usePolling<History>("history", 5000);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [images, setImages] = useState<ImageEntry[]>([]);
  const [mode, setMode] = useState("auto");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  useEffect(() => {
    void Promise.all([
      api<Settings>("settings"),
      api<{ images: ImageEntry[] }>("images"),
    ])
      .then(([s, i]) => {
        setSettings(s);
        setMode(s.hdmi_mode);
        setImages(i.images);
      })
      .catch((e) => setError(String(e)));
  }, []);
  async function run(task: () => Promise<void>) {
    setBusy(true);
    setError("");
    try {
      await task();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }
  async function save(next: Settings) {
    await api("settings", {
      method: "POST",
      headers,
      body: JSON.stringify(next),
    });
    setSettings(next);
    setMode(next.hdmi_mode);
  }
  const live =
    status?.phase === "streaming" &&
    (status?.bitrate_mbps ?? 0) > 0 &&
    !connectionError;
  const selected = images.find((i) => i.id === settings?.fallback_image);
  const modes = Array.from(new Set(status?.hdmi_modes ?? []));
  return (
    <main>
      <header>
        <h1>DJI HDMI</h1>
        <span
          className={`connection ${connectionError ? "bad" : live ? "good" : ""}`}
        >
          <i />
          {connectionError
            ? "Disconnected"
            : status
              ? (phases[status.phase] ?? status.phase)
              : "Connecting"}
        </span>
        <span className="version">
          {status?.version && `v${status.version}`}
        </span>
      </header>
      {(error || connectionError) && (
        <div className="alert" role="alert">
          {error || "Cannot reach the receiver. Check your network connection."}
        </div>
      )}
      {notice && (
        <div className="notice" role="status">
          {notice}
          <button
            aria-label="Dismiss notification"
            onClick={() => setNotice("")}
          >
            ×
          </button>
        </div>
      )}
      <section className="metrics" aria-label="Video status">
        <Metric
          title="Goggles input"
          primary={size(status?.input_width, status?.input_height)}
          secondary={`${number(status?.input_fps)} fps · ${number(status?.bitrate_mbps, 2)} Mbps`}
        />
        <Metric
          title="HDMI output"
          primary={size(status?.hdmi_width, status?.hdmi_height)}
          secondary={`${number(status?.hdmi_hz, 0)} Hz · ${status?.hdmi === "playing" ? number(status?.output_fps) : "0"} rendered fps`}
        />
        <Metric
          title="Pi temperature"
          primary={`${number(status?.temperature_c)} °C`}
          secondary={
            status
              ? `Uptime ${Math.floor(status.uptime_seconds / 3600)}h ${Math.floor(status.uptime_seconds / 60) % 60}m`
              : "—"
          }
        />
      </section>
      <div className="columns">
        <section className="panel">
          <div className="section-title">
            <h2>Preview</h2>
            <label className="toggle">
              <input
                type="checkbox"
                checked={settings?.preview_enabled ?? false}
                disabled={busy || !settings}
                onChange={(e) => {
                  if (settings)
                    void run(() =>
                      save({ ...settings, preview_enabled: e.target.checked }),
                    );
                }}
              />
              Live preview
            </label>
          </div>
          <Preview
            enabled={!!settings?.preview_enabled && !!live}
            fallback={selected?.url}
            state={
              live
                ? "live"
                : status?.hdmi === "fallback"
                  ? "fallback"
                  : "waiting"
            }
          />
          <div className="preview-caption">
            <span>
              {live
                ? settings?.preview_enabled
                  ? "640 × 360 · up to 5 fps"
                  : "Preview disabled"
                : status?.hdmi === "fallback"
                  ? "Fallback on HDMI"
                  : "Waiting for video"}
            </span>
          </div>
          <div className="actions">
            <button
              className="primary"
              disabled={busy || !status || !!connectionError}
              onClick={() =>
                void run(async () => {
                  await api(status?.enabled ? "stop" : "start", {
                    method: "POST",
                    headers,
                  });
                })
              }
            >
              {status?.enabled ? "Stop receiver" : "Start receiver"}
            </button>
            <button
              disabled={busy || !status || !!connectionError}
              onClick={() =>
                void run(async () => {
                  await api("restart", { method: "POST", headers });
                })
              }
            >
              Reconnect goggles
            </button>
          </div>
        </section>
        <section className="panel settings">
          <h2>HDMI</h2>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (settings)
                void run(() => save({ ...settings, hdmi_mode: mode }));
            }}
          >
            <label htmlFor="hdmi-mode">Output mode</label>
            <div className="input-row">
              <select
                id="hdmi-mode"
                value={mode}
                onChange={(e) => setMode(e.target.value)}
              >
                <option value="auto">Automatic · prefer 1080p</option>
                {!modes.includes(mode) && mode !== "auto" && (
                  <option value={mode}>{mode} (saved)</option>
                )}
                {modes.map((m) => (
                  <option key={m} value={m}>
                    {m.replace("x", " × ").replace("@", " · ")} Hz
                  </option>
                ))}
              </select>
              <button
                disabled={busy || !settings || mode === settings.hdmi_mode}
              >
                Apply
              </button>
            </div>
          </form>
          <p className="help">
            Refresh rate is the HDMI signal timing. Rendered fps is the rate of
            camera frames reaching the display.
          </p>
          <hr />
          <h2>Fallback image</h2>
          <div className="fallback-preview">
            {selected ? (
              <img src={selected.url} alt="Selected HDMI fallback" />
            ) : (
              <span>Solid dark background</span>
            )}
          </div>
          <div className="image-library" aria-label="Fallback images">
            <button
              className={!settings?.fallback_image ? "selected" : ""}
              title="Use solid dark background"
              aria-label="Use solid dark background"
              disabled={busy}
              onClick={() => {
                if (settings)
                  void run(() => save({ ...settings, fallback_image: null }));
              }}
            >
              <span>None</span>
            </button>
            {images.map((img, i) => (
              <button
                key={img.id}
                className={
                  settings?.fallback_image === img.id ? "selected" : ""
                }
                aria-label={`Select fallback image ${i + 1}`}
                disabled={busy}
                onClick={() => {
                  if (settings)
                    void run(() =>
                      save({ ...settings, fallback_image: img.id }),
                    );
                }}
              >
                <img src={img.url} alt={`Fallback ${i + 1}`} />
              </button>
            ))}
          </div>
          <div className="actions">
            <label className={`button ${busy ? "disabled" : ""}`}>
              Upload image
              <input
                className="file-input"
                type="file"
                accept="image/png,image/jpeg,image/webp"
                disabled={busy || !settings}
                onChange={(e) => {
                  const file = e.target.files?.[0];
                  e.target.value = "";
                  if (!file || !settings) return;
                  void run(async () => {
                    const data = new FormData();
                    data.append("image", file);
                    const uploaded = await api<ImageEntry>("images", {
                      method: "POST",
                      headers: { "X-DJI-Control": "1" },
                      body: data,
                    });
                    setImages(
                      (await api<{ images: ImageEntry[] }>("images")).images,
                    );
                    await save({ ...settings, fallback_image: uploaded.id });
                  });
                }}
              />
            </label>
            {selected && (
              <button
                disabled={busy}
                onClick={() =>
                  void run(async () => {
                    await save({ ...settings!, fallback_image: null });
                    await api(`images/${selected.id}`, {
                      method: "DELETE",
                      headers,
                    });
                    setImages(
                      (await api<{ images: ImageEntry[] }>("images")).images,
                    );
                  })
                }
              >
                Delete selected
              </button>
            )}
          </div>
          <p className="help">
            Shown when video stops or the receiver is stopped. PNG, JPEG or
            WebP, up to 12 MiB. Converted automatically; aspect ratio preserved.
          </p>
        </section>
      </div>
      <Timeline history={history} />
      <WifiPanel notify={setNotice} />
      <details className="panel diagnostics">
        <summary>Diagnostics</summary>
        <dl>
          <dt>Receiver</dt>
          <dd>{status?.phase ?? "—"}</dd>
          <dt>HDMI</dt>
          <dd>{status?.hdmi ?? "—"}</dd>
          <dt>Received</dt>
          <dd>{number((status?.video_bytes ?? 0) / 1e6)} MB</dd>
          <dt>USB resync bytes</dt>
          <dd>{status?.discarded_bytes ?? 0}</dd>
          <dt>Decoder starts</dt>
          <dd>{status?.decoder_starts ?? 0}</dd>
          <dt>Advertised camera fps</dt>
          <dd>{number(status?.input_nominal_fps)}</dd>
          <dt>Last event</dt>
          <dd>{status?.message ?? "—"}</dd>
        </dl>
        <a href="/api/diagnostics" target="_blank" rel="noreferrer">
          Hardware report
        </a>
      </details>
      <footer>
        Clean feed: Goggles → Camera → Advanced Camera Settings → Camera View
        Recording off.
      </footer>
    </main>
  );
}
function Metric({
  title,
  primary,
  secondary,
}: {
  title: string;
  primary: string;
  secondary: string;
}) {
  return (
    <div className="panel metric">
      <h2>{title}</h2>
      <strong>{primary}</strong>
      <span>{secondary}</span>
    </div>
  );
}
