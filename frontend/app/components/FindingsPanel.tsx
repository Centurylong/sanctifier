"use client";

import { useMemo } from "react";
import type { Finding } from "../types";
import { filterFindings, severityCounts } from "../lib/findings-query";
import { useFindingsQuery } from "./useFindingsQuery";
import { SeverityFacets } from "./SeverityFacets";
import { FindingsSearch } from "./FindingsSearch";
import { FindingsTable } from "./FindingsTable";

interface FindingsPanelProps {
  findings: Finding[];
}

/**
 * The results experience: severity facets + debounced search + a virtualized,
 * sortable table, all driven by URL-backed state so the view is shareable.
 * Must be rendered inside a <Suspense> boundary because it reads useSearchParams.
 */
export function FindingsPanel({ findings }: FindingsPanelProps) {
  const {
    state,
    toggleSeverity,
    clearSeverities,
    setQuery,
    setSort,
    clearAll,
    hasActiveFilters,
  } = useFindingsQuery();

  const counts = useMemo(
    () => severityCounts(findings, state),
    [findings, state]
  );
  const filtered = useMemo(
    () => filterFindings(findings, state),
    [findings, state]
  );

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <SeverityFacets
          selected={state.severities}
          counts={counts}
          onToggle={toggleSeverity}
          onClear={clearSeverities}
        />
        <FindingsSearch value={state.q} onChange={setQuery} />
      </div>

      <p
        className="text-sm text-zinc-500 dark:text-zinc-400 theme-high-contrast:text-white"
        aria-live="polite"
      >
        {filtered.length === findings.length
          ? `${findings.length} findings`
          : `${filtered.length} of ${findings.length} findings`}
      </p>

      {filtered.length === 0 ? (
        <div className="rounded-lg border border-dashed border-zinc-300 dark:border-zinc-700 theme-high-contrast:border-white py-12 text-center">
          <p className="text-zinc-500 dark:text-zinc-400 theme-high-contrast:text-white">
            No findings match the current filters.
          </p>
          {hasActiveFilters && (
            <button
              type="button"
              onClick={clearAll}
              className="mt-3 rounded-lg border border-zinc-300 dark:border-zinc-600 theme-high-contrast:border-white px-3 py-1.5 text-sm font-medium hover:bg-zinc-100 dark:hover:bg-zinc-800"
            >
              Clear filters
            </button>
          )}
        </div>
      ) : (
        <FindingsTable
          rows={filtered}
          sort={state.sort}
          dir={state.dir}
          onSortChange={setSort}
        />
      )}
    </div>
  );
}
