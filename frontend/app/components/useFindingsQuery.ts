"use client";

import { useCallback, useMemo } from "react";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import type { Severity } from "../types";
import {
  parseFindingsQuery,
  serializeFindingsQuery,
  type FindingsQueryState,
  type SortDir,
  type SortKey,
} from "../lib/findings-query";

export interface UseFindingsQuery {
  state: FindingsQueryState;
  /** Add/remove a single severity facet. */
  toggleSeverity: (severity: Severity) => void;
  /** Clear all severity facets (back to "all"). */
  clearSeverities: () => void;
  /** Set the search term (callers debounce). */
  setQuery: (q: string) => void;
  /** Set or clear the active sort column. */
  setSort: (sort: SortKey | null, dir: SortDir) => void;
  /** Reset every filter back to the default view. */
  clearAll: () => void;
  /** Whether any filter/sort is currently active. */
  hasActiveFilters: boolean;
}

/**
 * Reads the findings view state from the URL and exposes setters that write it
 * back via a shallow `router.replace` (no scroll jump, no history spam). The URL
 * stays the single source of truth, so the view is always shareable.
 */
export function useFindingsQuery(): UseFindingsQuery {
  const router = useRouter();
  const pathname = usePathname();
  const searchParams = useSearchParams();

  const state = useMemo(
    () => parseFindingsQuery(searchParams),
    [searchParams]
  );

  const commit = useCallback(
    (next: FindingsQueryState) => {
      const qs = serializeFindingsQuery(next).toString();
      router.replace(qs ? `${pathname}?${qs}` : pathname, { scroll: false });
    },
    [router, pathname]
  );

  const toggleSeverity = useCallback(
    (severity: Severity) => {
      const has = state.severities.includes(severity);
      const severities = has
        ? state.severities.filter((s) => s !== severity)
        : [...state.severities, severity];
      commit({ ...state, severities });
    },
    [state, commit]
  );

  const clearSeverities = useCallback(
    () => commit({ ...state, severities: [] }),
    [state, commit]
  );

  const setQuery = useCallback(
    (q: string) => commit({ ...state, q }),
    [state, commit]
  );

  const setSort = useCallback(
    (sort: SortKey | null, dir: SortDir) => commit({ ...state, sort, dir }),
    [state, commit]
  );

  const clearAll = useCallback(
    () => commit({ severities: [], q: "", sort: null, dir: "desc" }),
    [commit]
  );

  const hasActiveFilters =
    state.severities.length > 0 || state.q.trim().length > 0 || state.sort !== null;

  return {
    state,
    toggleSeverity,
    clearSeverities,
    setQuery,
    setSort,
    clearAll,
    hasActiveFilters,
  };
}
