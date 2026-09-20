import { useEffect, useRef, useState } from "react";
import { api, headers, usePolling } from "../api";
import { repository } from "../release-version";
import type { useReleases } from "../releases";
type UpdateState = {
  id: string | null;
  phase: string;
  version: string | null;
  message: string;
  warnings: string[];
  release_notes: string | null;
};
type Info = {
  available: boolean;
  current_version: string;
  current_build?: string;
  state?: UpdateState;
  error?: string;
};
const pendingKey = "ungoggled-pending-update";
function remembered(): string | null {
  try {
    return sessionStorage.getItem(pendingKey);
  } catch {
    return null;
  }
}
function remember(id: string | null) {
  try {
    if (id) sessionStorage.setItem(pendingKey, id);
    else sessionStorage.removeItem(pendingKey);
  } catch {
    /* Optional. */
  }
}
const active = (phase?: string) =>
  ["queued", "installing", "restarting"].includes(phase ?? "");
export function SoftwareUpdate({
  releases,
  showRelease,
  setShowRelease,
  onBusy,
}: {
  releases: ReturnType<typeof useReleases>;
  showRelease: boolean;
  setShowRelease: (v: boolean) => void;
  onBusy: (v: boolean) => void;
}) {
  const { value: polled, error: pollingError } = usePolling<Info>(
    "update",
    1000,
  );
  const [uploaded, setUploaded] = useState<UpdateState | null>(null);
  const [progress, setProgress] = useState<number | null>(null);
  const [error, setError] = useState("");
  const [dragOver, setDragOver] = useState(false);
  const dragDepth = useRef(0);
  const [pending, setPending] = useState<string | null>(remembered);
  const [sending, setSending] = useState(false);
  const request = useRef<XMLHttpRequest | null>(null);
  const previousBuild = useRef<string | undefined>(undefined);
  // The immediate upload result bridges the next status poll only.
  const state = uploaded ?? polled?.state;
  const busy = active(state?.phase) || sending || !!pending;
  useEffect(() => {
    if (polled?.state?.id === uploaded?.id) setUploaded(null);
  }, [polled, uploaded?.id]);
  useEffect(() => {
    onBusy(busy);
  }, [busy, onBusy]);
  useEffect(() => () => request.current?.abort(), []);
  useEffect(() => {
    if (!polled?.state) return;
    const s = polled.state;
    if (pending && s.id !== pending && !sending) {
      // Another browser may have replaced a prepared update while we were away.
      remember(null);
      setPending(null);
    }
    if (
      s.id === pending &&
      ["succeeded", "rolled_back", "failed"].includes(s.phase)
    ) {
      remember(null);
      setPending(null);
      setSending(false);
      if (s.phase === "succeeded") window.location.reload();
    }
    if (
      previousBuild.current &&
      polled.current_build &&
      previousBuild.current !== polled.current_build &&
      s.phase === "succeeded"
    )
      window.location.reload();
    previousBuild.current = polled.current_build;
  }, [polled, pending, sending]);
  const uploadDisabled = !polled?.available || busy || progress !== null;
  function upload(file: File) {
    if (file.size > 64 * 1024 * 1024) {
      setError("Update files must be smaller than 64 MiB.");
      return;
    }
    setError("");
    setProgress(0);
    setUploaded(null);
    const xhr = new XMLHttpRequest();
    request.current = xhr;
    xhr.open("POST", "/api/update/upload");
    xhr.setRequestHeader("X-DJI-Control", "1");
    xhr.timeout = 180000;
    xhr.upload.onprogress = (e) => {
      if (e.lengthComputable)
        setProgress(Math.round((e.loaded / e.total) * 100));
    };
    xhr.onload = () => {
      try {
        const result = JSON.parse(xhr.responseText);
        if (xhr.status >= 200 && xhr.status < 300) setUploaded(result);
        else setError(result.error || "Update validation failed.");
      } catch {
        setError("Invalid response from receiver.");
      }
      setProgress(null);
      request.current = null;
    };
    xhr.onerror = xhr.ontimeout = () => {
      setError("Upload interrupted. Reconnect and try again.");
      setProgress(null);
      request.current = null;
    };
    xhr.onabort = () => {
      setProgress(null);
      request.current = null;
    };
    const form = new FormData();
    form.append("update", file);
    xhr.send(form);
  }
  async function install() {
    if (!state?.id) return;
    setError("");
    setSending(true);
    remember(state.id);
    setPending(state.id);
    try {
      await api("update/install", {
        method: "POST",
        headers,
        body: JSON.stringify({ id: state.id }),
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : "Could not start update.");
      remember(null);
      setPending(null);
    } finally {
      setSending(false);
      setUploaded(null);
    }
  }
  return (
    <section className="panel software-update" id="software-update">
      <div className="section-title">
        <h2>Software update</h2>
        <span>v{polled?.current_version ?? "—"}</span>
      </div>
      <div className="actions">
        <button
          disabled={releases.checking}
          onClick={() => void releases.check(true)}
        >
          {releases.checking ? "Checking…" : "Check releases"}
        </button>
        <a href={`${repository}/releases`} target="_blank" rel="noreferrer">
          GitHub releases ↗
        </a>
      </div>
      {releases.latest && (
        <div className="release-summary">
          <button onClick={() => setShowRelease(!showRelease)}>
            v{releases.latest.version} available ·{" "}
            {showRelease ? "Hide" : "Release notes"}
          </button>
          {showRelease && (
            <div className="release-details">
              <h3>{releases.latest.name}</h3>
              <div className="release-notes">{releases.latest.notes}</div>
              <a href={releases.latest.url} target="_blank" rel="noreferrer">
                Open release on GitHub ↗
              </a>
            </div>
          )}
        </div>
      )}
      {releases.error ? (
        <p className="help">{releases.error} File uploads work offline.</p>
      ) : (
        releases.checked &&
        !releases.latest && (
          <p className="help">No newer stable release found.</p>
        )
      )}
      {polled && !polled.available && (
        <p className="help">
          Web installation is available on a Pi installed with updater support.
        </p>
      )}
      <div
        className={`update-upload-zone ${dragOver ? "drag-over" : ""} ${uploadDisabled ? "disabled" : ""}`}
        onDragEnter={(e) => {
          e.preventDefault();
          if (uploadDisabled) return;
          dragDepth.current += 1;
          setDragOver(true);
        }}
        onDragOver={(e) => {
          e.preventDefault();
        }}
        onDragLeave={(e) => {
          e.preventDefault();
          dragDepth.current = Math.max(0, dragDepth.current - 1);
          if (dragDepth.current === 0) setDragOver(false);
        }}
        onDrop={(e) => {
          e.preventDefault();
          dragDepth.current = 0;
          setDragOver(false);
          const file = e.dataTransfer.files?.[0];
          if (file && !uploadDisabled) upload(file);
        }}
      >
        <div className="actions update-upload">
          <label className={`button ${uploadDisabled ? "disabled" : ""}`}>
            Upload update
            <input
              className="file-input"
              type="file"
              accept=".gz,.tgz"
              disabled={uploadDisabled}
              onChange={(e) => {
                const file = e.target.files?.[0];
                e.target.value = "";
                if (file) upload(file);
              }}
            />
          </label>
          {progress !== null && (
            <>
              <progress max={100} value={progress} aria-label="Update upload" />
              <span>
                {progress === 100 ? "Checking file…" : `${progress}%`}
              </span>
            </>
          )}
        </div>
        <p className="help">
          Choose the release’s .update.tar.gz file, or drag and drop it here.
          Video pauses during installation; compatible settings are retained.
        </p>
      </div>
      {state?.phase === "ready" && (
        <div className="update-ready">
          <strong>v{state.version} ready to install</strong>
          {state.release_notes && (
            <details>
              <summary>Package release notes</summary>
              <div className="release-notes">{state.release_notes}</div>
            </details>
          )}
          <button
            className="primary"
            disabled={busy || progress !== null}
            onClick={() => void install()}
          >
            Install v{state.version}
          </button>
        </div>
      )}
      {busy && (
        <p role="status">
          {pollingError
            ? "Receiver restarting. Waiting for it to reconnect…"
            : state?.message || "Starting update…"}
        </p>
      )}
      {state &&
        ["succeeded", "rolled_back", "failed"].includes(state.phase) && (
          <p
            role="status"
            className={state.phase === "succeeded" ? "" : "error"}
          >
            {state.message}
          </p>
        )}
      {!!state?.warnings.length && (
        <ul>
          {state.warnings.map((w) => (
            <li key={w}>{w}</li>
          ))}
        </ul>
      )}
      {(error || polled?.error || (!busy && pollingError)) && (
        <p role="alert" className="error">
          {error || polled?.error || "Cannot reach the receiver."}
        </p>
      )}
    </section>
  );
}
