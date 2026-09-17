export const number = (v: number | null | undefined, digits = 1) =>
  v == null ? "—" : v.toFixed(digits);
export const size = (w?: number, h?: number) => (w && h ? `${w} × ${h}` : "—");
