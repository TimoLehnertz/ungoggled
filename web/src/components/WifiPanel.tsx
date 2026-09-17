import { useEffect, useState } from "react";
import type { Wifi } from "../types";
import { api, headers, usePolling } from "../api";
export function WifiPanel({ notify }: { notify: (s: string) => void }) {
  const { value: wifi } = usePolling<Wifi>("wifi", 5000);
  const [ssid, setSsid] = useState("");
  const [password, setPassword] = useState("");
  const [dirty, setDirty] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    if (wifi?.ssid && !dirty) setSsid(wifi.ssid);
  }, [wifi?.ssid, dirty]);
  return (
    <section className="panel wifi">
      <h2>Wi-Fi access point</h2>
      {wifi?.available ? (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            setBusy(true);
            setError("");
            void api<{ message: string }>("wifi", {
              method: "POST",
              headers,
              body: JSON.stringify({ ssid, password }),
            })
              .then((r) => {
                notify(r.message);
                setPassword("");
                setDirty(false);
              })
              .catch((e) => setError(String(e)))
              .finally(() => setBusy(false));
          }}
        >
          <div className="wifi-fields">
            <label>
              SSID
              <input
                value={ssid}
                required
                maxLength={32}
                autoComplete="off"
                onChange={(e) => {
                  setSsid(e.target.value);
                  setDirty(true);
                }}
              />
            </label>
            <label>
              New password
              <input
                type="password"
                value={password}
                required
                minLength={8}
                maxLength={63}
                autoComplete="new-password"
                onChange={(e) => setPassword(e.target.value)}
              />
            </label>
            <button disabled={busy || wifi.applying}>Apply Wi-Fi</button>
          </div>
          <p className="help">
            Applying disconnects Wi-Fi. Reconnect using the new SSID and
            password. HDMI keeps running.
          </p>
          {(error || wifi.error) && (
            <p role="alert" className="error">
              {error || wifi.error}
            </p>
          )}
        </form>
      ) : (
        <p className="help">No managed access point configured.</p>
      )}
    </section>
  );
}
