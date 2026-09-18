import { useEffect, useState } from "react";
export const headers = {
  "X-DJI-Control": "1",
  "Content-Type": "application/json",
};
export async function api<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`/api/${path}`, init);
  if (!response.ok) {
    const body = await response.json().catch(() => ({}));
    throw new Error(body.error || `Request failed (${response.status})`);
  }
  return response.json() as Promise<T>;
}
export function usePolling<T>(path: string, ms: number) {
  const [value, setValue] = useState<T | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    const abort = new AbortController();
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const data = await api<T>(path, {
          signal: AbortSignal.any([abort.signal, AbortSignal.timeout(5000)]),
        });
        if (!abort.signal.aborted) {
          setValue(data);
          setError("");
        }
      } catch (e) {
        if (!abort.signal.aborted) setError(String(e));
      }
      if (!abort.signal.aborted) timer = setTimeout(poll, ms);
    };
    void poll();
    return () => {
      abort.abort();
      clearTimeout(timer);
    };
  }, [path, ms]);
  return { value, error };
}
