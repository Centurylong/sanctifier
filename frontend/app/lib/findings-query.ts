import type { Finding, Severity } from "../types";

/**
 * Pure helpers for the findings results view.
 *
 * The URL search params are the single source of truth for the active view:
 * severity facets, full-text search, and column sorting all serialize to the
 * query string so a pasted link reproduces the exact same table. Everything in
 * this module is framework-agnostic and side-effect free, which keeps the
 * filtering/sorting logic easy to reason about and unit-test independently of
 * React or the router.
 */

/** Canonical display + serialization order for severities. */
export const SEVERITY_ORDER: readonly Severity[] = [
  "critical",
  "high",
  "medium",
  "low",
];

/** Higher number = more severe. Used as the sort weight for the severity column. */
export const SEVERITY_RANK: Record<Severity, number> = {
  critical: 3,
  high: 2,
  medium: 1,
  low: 0,
};

export type SortKey = "severity" | "code" | "location" | "message";
export type SortDir = "asc" | "desc";

export interface FindingsQueryState {
  /** Selected severity facets. Empty array means "all severities". */
  severities: Severity[];
  /** Debounced full-text search term. */
  q: string;
  /** Active sort column, or null for the natural (input) order. */
  sort: SortKey | null;
  /** Sort direction; only meaningful when {@link sort} is set. */
  dir: SortDir;
}

export const EMPTY_QUERY: FindingsQueryState = {
  severities: [],
  q: "",
  sort: null,
  dir: "desc",
};

const VALID_SORT_KEYS: readonly SortKey[] = [
  "severity",
  "code",
  "location",
  "message",
];

/** Minimal read interface satisfied by both URLSearchParams and Next's ReadonlyURLSearchParams. */
interface ReadableParams {
  get(name: string): string | null;
}

function isSeverity(value: string): value is Severity {
  return (SEVERITY_ORDER as readonly string[]).includes(value);
}

function isSortKey(value: string): value is SortKey {
  return (VALID_SORT_KEYS as readonly string[]).includes(value);
}

/** Parse the findings view state from URL search params, ignoring anything malformed. */
export function parseFindingsQuery(params: ReadableParams): FindingsQueryState {
  const rawSeverities = (params.get("severity") ?? "")
    .split(",")
    .map((s) => s.trim().toLowerCase())
    .filter(isSeverity);
  // De-duplicate and force canonical order so the serialized form is stable.
  const severities = SEVERITY_ORDER.filter((s) => rawSeverities.includes(s));

  const q = (params.get("q") ?? "").trim();

  const rawSort = params.get("sort") ?? "";
  const sort = isSortKey(rawSort) ? rawSort : null;

  const dir: SortDir = params.get("dir") === "asc" ? "asc" : "desc";

  return { severities, q, sort, dir };
}

/** Serialize view state back into URL search params, omitting defaults to keep links tidy. */
export function serializeFindingsQuery(
  state: FindingsQueryState
): URLSearchParams {
  const params = new URLSearchParams();

  if (state.severities.length > 0) {
    params.set(
      "severity",
      SEVERITY_ORDER.filter((s) => state.severities.includes(s)).join(",")
    );
  }

  const q = state.q.trim();
  if (q) params.set("q", q);

  if (state.sort) {
    params.set("sort", state.sort);
    params.set("dir", state.dir);
  }

  return params;
}

function haystack(f: Finding): string {
  return [f.title, f.category, f.location, f.snippet, f.suggestion]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();
}

/** Apply the severity facets + search term. Sorting is handled by the table itself. */
export function filterFindings(
  findings: Finding[],
  state: FindingsQueryState
): Finding[] {
  const q = state.q.trim().toLowerCase();
  const selected = new Set(state.severities);

  return findings.filter((f) => {
    if (selected.size > 0 && !selected.has(f.severity)) return false;
    if (q && !haystack(f).includes(q)) return false;
    return true;
  });
}

/**
 * Count findings per severity for the facet chips. Counts respect the active
 * search term but ignore the severity selection itself, so each chip shows how
 * many results selecting it would surface.
 */
export function severityCounts(
  findings: Finding[],
  state: FindingsQueryState
): Record<Severity, number> {
  const q = state.q.trim().toLowerCase();
  const counts: Record<Severity, number> = {
    critical: 0,
    high: 0,
    medium: 0,
    low: 0,
  };

  for (const f of findings) {
    if (q && !haystack(f).includes(q)) continue;
    counts[f.severity] += 1;
  }

  return counts;
}
