import { useCallback, useEffect, useState } from "react";
import { newestRelease, type Release } from "./release-version";
const endpoint =
  "https://api.github.com/repos/TimoLehnertz/ungoggled/releases?per_page=100";
const cacheKey = "ungoggled-github-releases-v1";
const hour = 60 * 60 * 1000;
let inFlight: Promise<unknown> | null = null;
async function releases(force: boolean): Promise<unknown> {
  if (!force) {
    try {
      const cached = JSON.parse(localStorage.getItem(cacheKey) || "null");
      if (
        cached &&
        Date.now() - cached.time >= 0 &&
        Date.now() - cached.time < hour
      )
        return cached.data;
    } catch {
      /* Storage may be disabled. */
    }
  }
  if (!inFlight)
    inFlight = (async () => {
      const response = await fetch(endpoint, {
        headers: { Accept: "application/vnd.github+json" },
        signal: AbortSignal.timeout(10000),
      });
      if (!response.ok)
        throw new Error(
          response.status === 403 || response.status === 429
            ? "GitHub rate limit reached. Try later."
            : "Could not check GitHub releases.",
        );
      const data: unknown = await response.json();
      if (!Array.isArray(data)) throw new Error("Invalid GitHub response");
      try {
        localStorage.setItem(
          cacheKey,
          JSON.stringify({ time: Date.now(), data }),
        );
      } catch {
        /* Optional cache. */
      }
      return data;
    })().finally(() => {
      inFlight = null;
    });
  return inFlight;
}
export function useReleases(current?: string) {
  const [latest, setLatest] = useState<Release | null>(null);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState("");
  const [checked, setChecked] = useState(false);
  const check = useCallback(
    async (force = false) => {
      if (!current) return;
      setChecking(true);
      setError("");
      try {
        setLatest(newestRelease(await releases(force), current));
        setChecked(true);
      } catch (e) {
        setError(
          e instanceof Error ? e.message : "Could not check GitHub releases.",
        );
      } finally {
        setChecking(false);
      }
    },
    [current],
  );
  useEffect(() => {
    void check();
    const timer = setInterval(() => void check(), hour);
    return () => clearInterval(timer);
  }, [check]);
  return { latest, checking, error, checked, check };
}
