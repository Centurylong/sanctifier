"use client";

import type { Severity } from "../types";

interface SeverityFilterProps {
  selected: Severity | "all";
  onChange: (s: Severity | "all") => void;
}

const labels: Record<Severity | "all", string> = {
  all: "All",
  critical: "Critical",
  high: "High",
  medium: "Medium",
  low: "Low",
};

export function SeverityFilter({ selected, onChange }: SeverityFilterProps) {
  const options: (Severity | "all")[] = ["all", "critical", "high", "medium", "low"];

  return (
    <fieldset className="flex flex-wrap gap-2">
      <legend className="sr-only">Filter findings by severity</legend>
      {options.map((s) => (
        <button
          key={s}
          onClick={() => onChange(s)}
          className="rounded-lg px-3 py-1.5 text-sm font-medium transition-colors focus:outline-none focus:ring-2"
          style={{
            backgroundColor:
              selected === s
                ? s === "all"
                  ? "var(--primary)"
                  : `var(--severity-${s})`
                : "var(--secondary)",
            color:
              selected === s
                ? s === "all"
                  ? "var(--primary-foreground)"
                  : "white"
                : "var(--secondary-foreground)",
          }}
          aria-pressed={selected === s}
          role="radio"
          aria-checked={selected === s}
        >
          {labels[s]}
        </button>
      ))}
    </fieldset>
  );
}
