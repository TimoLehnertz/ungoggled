export const repository = "https://github.com/TimoLehnertz/ungoggled";
export type Release = {
  version: string;
  name: string;
  notes: string;
  url: string;
};
export function versionParts(value: string): number[] | null {
  const match = /^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.exec(value);
  if (!match) return null;
  const parts = match.slice(1).map(Number);
  return parts.every(Number.isSafeInteger) ? parts : null;
}
export function compareVersions(a: string, b: string): number {
  const x = versionParts(a),
    y = versionParts(b);
  if (!x || !y) throw new Error("Invalid stable version");
  for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return x[i] > y[i] ? 1 : -1;
  return 0;
}
export function newestRelease(data: unknown, current: string): Release | null {
  if (!Array.isArray(data) || !versionParts(current)) return null;
  let latest: Release | null = null;
  for (const r of data) {
    if (
      !r ||
      r.draft ||
      r.prerelease ||
      typeof r.tag_name !== "string" ||
      !versionParts(r.tag_name)
    )
      continue;
    if (
      compareVersions(r.tag_name, current) <= 0 ||
      (latest && compareVersions(r.tag_name, latest.version) <= 0)
    )
      continue;
    latest = {
      version: r.tag_name.replace(/^v/, ""),
      name: typeof r.name === "string" ? r.name : r.tag_name,
      notes:
        typeof r.body === "string"
          ? r.body.slice(0, 128 * 1024)
          : "No release notes provided.",
      url: `${repository}/releases/tag/${encodeURIComponent(r.tag_name)}`,
    };
  }
  return latest;
}
