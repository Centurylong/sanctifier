"use client";

import type { Severity } from "../types";
import { SEVERITY_ORDER } from "../lib/findings-query";

interface SeverityFacetsProps {
  /** Currently selected severities. Empty means "all". */
  selected: Severity[];
  /** Count of findings per severity (respecting the active search term). */
  counts: Record<Severity, number>;
  onToggle: (severity: Severity) => void;
  onClear: () => void;
}

const labels: Record<Severity, string> = {
  critical: "Critical",
  high: "High",
  medium: "Medium",
  low: "Low",
};

const activeColors: Record<Severity, string> = {
  critical: "bg-red-500 text-white border-red-500",
  high: "bg-orange-500 text-white border-orange-500",
  medium: "bg-amber-500 text-white border-amber-500",
  low: "bg-zinc-500 text-white border-zinc-500",
};

const baseChip =
  "rounded-lg border px-3 py-1.5 text-sm font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-40 theme-high-contrast:border-white";
const idleChip =
  "border-zinc-300 dark:border-zinc-700 bg-zinc-200 dark:bg-zinc-800 text-zinc-700 dark:text-zinc-300 hover:bg-zinc-300 dark:hover:bg-zinc-700 theme-high-contrast:bg-black theme-high-contrast:text-white";

/** Multi-select severity facets. Toggling a chip adds/removes it; "All" clears the selection. */
export function SeverityFacets({
  selected,
  counts,
  onToggle,
  onClear,
}: SeverityFacetsProps) {
  const allActive = selected.length === 0;

  return (
    <div
      className="flex flex-wrap items-center gap-2"
      role="group"
      aria-label="Filter findings by severity"
    >
      <button
        type="button"
        onClick={onClear}
        aria-pressed={allActive}
        className={`${baseChip} ${
          allActive
            ? "bg-zinc-800 dark:bg-zinc-700 text-white border-zinc-800 dark:border-zinc-700"
            : idleChip
        }`}
      >
        All
      </button>

      {SEVERITY_ORDER.map((s) => {
        const isSelected = selected.includes(s);
        const count = counts[s] ?? 0;
        return (
          <button
            key={s}
            type="button"
            onClick={() => onToggle(s)}
            aria-pressed={isSelected}
            disabled={count === 0 && !isSelected}
            className={`${baseChip} ${isSelected ? activeColors[s] : idleChip}`}
          >
            {labels[s]}
            <span className="ml-1.5 tabular-nums opacity-75">{count}</span>
          </button>
        );
      })}
    </div>
  );
}
