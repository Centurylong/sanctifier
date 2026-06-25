"use client";

import type { Finding, Severity } from "../types";
import { CodeSnippet } from "./CodeSnippet";

interface FindingsListProps {
  findings: Finding[];
  severityFilter: Severity | "all";
}

export function FindingsList({ findings, severityFilter }: FindingsListProps) {
  const filtered =
    severityFilter === "all"
      ? findings
      : findings.filter((f) => f.severity === severityFilter);

  return (
    <div className="space-y-4">
      {filtered.length === 0 ? (
        <p className="py-8 text-center" style={{ color: "var(--muted-foreground)" }}>
          No findings match the selected filter.
        </p>
      ) : (
        filtered.map((f) => (
          <div
            key={f.id}
            className="rounded-lg border p-4"
            style={{
              borderColor: `var(--severity-${f.severity})`,
              backgroundColor: "var(--card)",
            }}
          >
            <div className="flex items-start justify-between gap-4">
              <div className="min-w-0 flex-1">
                <span 
                  className="text-xs font-semibold uppercase tracking-wide"
                  style={{ color: `var(--severity-${f.severity})` }}
                >
                  {f.category}
                </span>
                <h3 className="mt-1 font-medium" style={{ color: "var(--card-foreground)" }}>
                  {f.title}
                </h3>
                <p className="mt-1 text-sm" style={{ color: "var(--muted-foreground)" }}>
                  {f.location}
                </p>
                {f.suggestion && (
                  <p className="mt-2 text-sm italic" style={{ color: "var(--muted-foreground)" }}>
                    💡 {f.suggestion}
                  </p>
                )}
              </div>
              <span
                className="shrink-0 rounded px-2 py-1 text-xs font-medium border"
                style={{
                  borderColor: `var(--severity-${f.severity})`,
                  color: `var(--severity-${f.severity})`,
                }}
              >
                {f.severity}
              </span>
            </div>
            {f.snippet && (
              <div className="mt-3">
                <CodeSnippet code={f.snippet} highlightLine={f.line} />
              </div>
            )}
          </div>
        ))
      )}
    </div>
  );
}
