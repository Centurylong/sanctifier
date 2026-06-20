import type { Finding, DiffedFinding } from "../types";

/**
 * Compare two sets of findings by their stable fingerprint.
 * Returns findings bucketed as added (in `next` only), removed (in `base` only),
 * or unchanged (fingerprint present in both).
 */
export function diffFindings(
  base: Finding[],
  next: Finding[]
): DiffedFinding[] {
  const baseSet = new Set(base.map((f) => f.fingerprint));
  const nextSet = new Set(next.map((f) => f.fingerprint));

  const result: DiffedFinding[] = [];

  for (const f of base) {
    result.push({
      finding: f,
      bucket: nextSet.has(f.fingerprint) ? "unchanged" : "removed",
    });
  }

  for (const f of next) {
    if (!baseSet.has(f.fingerprint)) {
      result.push({ finding: f, bucket: "added" });
    }
  }

  return result;
}
