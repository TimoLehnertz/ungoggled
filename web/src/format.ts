export const number = (v: number | null | undefined, digits = 1) =>
  v == null ? "—" : v.toFixed(digits);
export const size = (w?: number, h?: number) => (w && h ? `${w} × ${h}` : "—");
export function skewLabel(deltaMs: number): string {
  const behind = deltaMs > 0;
  const sec = Math.max(1, Math.round(Math.abs(deltaMs) / 1000));
  let amount: string;
  if (sec < 60) amount = `${sec} second${sec === 1 ? "" : "s"}`;
  else if (sec < 3600) {
    const m = Math.round(sec / 60);
    amount = `${m} minute${m === 1 ? "" : "s"}`;
  } else if (sec < 86400) {
    const h = Math.round(sec / 3600);
    amount = `${h} hour${h === 1 ? "" : "s"}`;
  } else {
    const d = Math.round(sec / 86400);
    amount = `${d} day${d === 1 ? "" : "s"}`;
  }
  return behind ? `${amount} behind` : `${amount} ahead`;
}
