import { useState } from "react";
import type { History, Sample } from "../types";
import { number } from "../format";
export function Timeline({ history }: { history: History | null }) {
  const [metric, setMetric] = useState("bitrate");
  const [hover, setHover] = useState<number | null>(null);
  const samples = history?.samples ?? [];
  const now = history?.now ?? 0;
  const begin = Math.max(0, now - 1800);
  const span = Math.max(60, now - begin);
  const keys: (
    "bitrate_mbps" | "input_fps" | "output_fps" | "temperature_c"
  )[] =
    metric === "fps"
      ? ["input_fps", "output_fps"]
      : metric === "temperature"
        ? ["temperature_c"]
        : ["bitrate_mbps"];
  const max =
    Math.max(
      metric === "temperature" ? 80 : metric === "fps" ? 60 : 10,
      ...samples.flatMap((s) => keys.map((k) => s[k] ?? 0)),
    ) * 1.05;
  const x = (t: number) => 44 + ((t - begin) / span) * 920;
  const y = (v: number) => 172 - (v / max) * 145;
  const chosen =
    hover == null
      ? null
      : samples.reduce<Sample | null>(
          (best, s) =>
            !best || Math.abs(s.t - hover) < Math.abs(best.t - hover)
              ? s
              : best,
          null,
        );
  return (
    <section className="panel timeline">
      <div className="section-title">
        <h2>Last 30 minutes</h2>
        <select
          aria-label="Timeline metric"
          value={metric}
          onChange={(e) => setMetric(e.target.value)}
        >
          <option value="bitrate">Bitrate · Mbps</option>
          <option value="fps">Frames per second</option>
          <option value="temperature">Temperature · °C</option>
        </select>
      </div>
      <svg
        viewBox="0 0 1000 210"
        role="img"
        aria-label={`${metric} history, retained in RAM`}
        onPointerMove={(e) => {
          const rect = e.currentTarget.getBoundingClientRect();
          setHover(
            begin +
              Math.max(
                0,
                Math.min(
                  1,
                  (((e.clientX - rect.left) / rect.width) * 1000 - 44) / 920,
                ),
              ) *
                span,
          );
        }}
        onPointerLeave={() => setHover(null)}
      >
        {[0, 0.5, 1].map((f) => (
          <g key={f}>
            <line
              x1="44"
              x2="964"
              y1={y(f * max)}
              y2={y(f * max)}
              className="gridline"
            />
            <text x="36" y={y(f * max) + 4} textAnchor="end">
              {number(f * max, metric === "bitrate" ? 1 : 0)}
            </text>
          </g>
        ))}
        {samples
          .filter((s) => s.hdmi === "fallback" || s.phase !== "streaming")
          .map((s) => (
            <rect
              key={s.t}
              x={x(s.t)}
              y="20"
              width={Math.max(1, 920 / span)}
              height="152"
              className="outage"
            />
          ))}
        {keys.map((key, i) => {
          let pen = false;
          const d = samples
            .map((s) => {
              const v = s[key];
              if (v == null) {
                pen = false;
                return "";
              }
              const part = `${pen ? "L" : "M"}${x(s.t).toFixed(1)},${y(v).toFixed(1)}`;
              pen = true;
              return part;
            })
            .join(" ");
          return <path key={key} d={d} className={`line line-${i}`} />;
        })}
        <text x="44" y="197">
          {Math.round(span / 60)} min ago
        </text>
        <text x="964" y="197" textAnchor="end">
          Now
        </text>
        {chosen && (
          <line
            className="cursor"
            x1={x(chosen.t)}
            x2={x(chosen.t)}
            y1="20"
            y2="174"
          />
        )}
      </svg>
      <div className="chart-caption">
        <span>
          {chosen
            ? `${Math.round(now - chosen.t)}s ago · ${keys.map((k) => `${k === "input_fps" ? "Input " : k === "output_fps" ? "Output " : ""}${number(chosen[k])}`).join(" / ")}`
            : metric === "fps"
              ? "Blue: input · Green: rendered"
              : "Hover to inspect"}
        </span>
        <span>Shaded: no live signal · RAM only</span>
      </div>
    </section>
  );
}
