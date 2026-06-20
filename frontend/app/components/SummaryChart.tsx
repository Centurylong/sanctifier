"use client";

import { useMemo } from "react";
import type { Finding, Severity } from "../types";

interface SummaryChartProps {
  findings: Finding[];
}

export function SummaryChart({ findings }: SummaryChartProps) {
  const counts = useMemo(() => {
    const s: Record<Severity, number> = {
      critical: 0,
      high: 0,
      medium: 0,
      low: 0,
    };
    findings.forEach((f) => {
      s[f.severity]++;
    });
    return s;
  }, [findings]);

  const total = findings.length;
  const max = Math.max(...Object.values(counts), 1);

  const bars: { label: Severity; count: number }[] = [
    { label: "critical", count: counts.critical },
    { label: "high", count: counts.high },
    { label: "medium", count: counts.medium },
    { label: "low", count: counts.low },
  ];

  return (
    <div
      className="rounded-lg border p-4"
      style={{
        borderColor: "var(--border)",
        backgroundColor: "var(--card)",
        color: "var(--card-foreground)",
      }}
    >
      <h3 className="text-sm font-semibold mb-4" style={{ color: "var(--muted-foreground)" }}>
        Findings by Severity
      </h3>
      <div className="space-y-3" role="list" aria-label="Severity counts">
        {bars.map(({ label, count }) => (
          <div key={label} className="flex items-center gap-2 sm:gap-3" role="listitem">
            <span className="w-16 sm:w-20 text-[10px] sm:text-xs font-medium capitalize truncate">
              {label}
            </span>
            <div
              className="flex-1 h-6 rounded overflow-hidden"
              style={{ backgroundColor: "var(--muted)" }}
              role="progressbar"
              aria-valuenow={count}
              aria-valuemin={0}
              aria-valuemax={max}
              aria-label={`${label} severity: ${count} findings`}
            >
              <div
                className="h-full transition-all"
                style={{
                  width: `${(count / max) * 100}%`,
                  backgroundColor: `var(--severity-${label})`,
                }}
              />
            </div>
            <span className="w-6 sm:w-8 text-right text-xs sm:text-sm">{count}</span>
          </div>
        ))}
      </div>
      <p className="mt-3 text-xs" style={{ color: "var(--muted-foreground)" }}>
        Total: {total} findings
      </p>
    </div>
  );
}
