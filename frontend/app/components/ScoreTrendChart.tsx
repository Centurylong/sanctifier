"use client";

// ScoreTrendChart renders the Sanctity Score over time for a single contract
// or repo on the dashboard. Backs the time-series story called out in
// issue #378.
//
// Design notes worth keeping in the code rather than the PR description:
//
//  - Severity bands are ReferenceArea overlays, not separate series, so the
//    line stays the visual focus and the bands reflow with the chart on
//    resize. Colour reads from the same CSS variables the gauge uses, so
//    light and dark mode follow the existing theme tokens for free.
//  - The range selector is rendered here. It is keyboard reachable and
//    visually shows the active range. When the requested range collapses
//    to "all" because the dataset is shorter than the window, the helper
//    text below the chart calls it out instead of silently widening.
//  - The single-point state renders the dot with an annotation rather than
//    a degenerate line. recharts will draw a line of length zero if you
//    let it; we explicitly switch to the "dot" presentation in that case.
//  - The tooltip surfaces three pieces of information the gauge does not
//    expose elsewhere: the score, the delta against the previous datapoint,
//    and the severity category whose count changed most. The category line
//    is what makes the chart a story rather than a number.

import { useMemo, useState } from "react";
import {
  CartesianGrid,
  Line,
  LineChart,
  ReferenceArea,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { Severity } from "../types";
import { SCORE_BANDS, type Range, type HistoricalScorePoint } from "../lib/score-history/types";
import { useScoreHistory } from "../lib/score-history/useScoreHistory";
import type { ScoreHistoryAdapter } from "../lib/score-history/types";

export interface ScoreTrendChartProps {
  contractId: string;
  // Optional adapter override. The default in-memory adapter ships fixtures
  // for stories and the dashboard demo; production code passes the
  // persisted-history adapter once #364 lands.
  adapter?: ScoreHistoryAdapter;
  className?: string;
}

const RANGES: { value: Range; label: string }[] = [
  { value: "7d", label: "7d" },
  { value: "30d", label: "30d" },
  { value: "all", label: "All" },
];

function formatDate(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

// Return the severity whose count moved the most between two snapshots.
// Ties break in favour of the worse severity (critical > high > medium > low)
// because that is the one a reviewer cares about most.
function biggestMover(
  a: HistoricalScorePoint["breakdown"],
  b: HistoricalScorePoint["breakdown"],
): { severity: Severity; delta: number } | null {
  const order: Severity[] = ["critical", "high", "medium", "low"];
  let best: { severity: Severity; delta: number } | null = null;
  for (const sev of order) {
    const delta = (b[sev] ?? 0) - (a[sev] ?? 0);
    if (delta === 0) continue;
    if (!best || Math.abs(delta) > Math.abs(best.delta)) {
      best = { severity: sev, delta };
    }
  }
  return best;
}

type TrendTooltipPoint = HistoricalScorePoint & {
  previous: HistoricalScorePoint | null;
};

interface TrendTooltipProps {
  active?: boolean;
  payload?: ReadonlyArray<{ payload: TrendTooltipPoint }>;
}

function TrendTooltip({ active, payload }: TrendTooltipProps) {
  if (!active || !payload || payload.length === 0) return null;
  const point = payload[0].payload;
  const prev = point.previous;
  const delta = prev ? point.score - prev.score : null;
  const mover = prev ? biggestMover(prev.breakdown, point.breakdown) : null;

  return (
    <div
      className="rounded-md border px-3 py-2 text-xs shadow-md"
      style={{
        backgroundColor: "var(--card)",
        color: "var(--card-foreground)",
        borderColor: "var(--border)",
      }}
      role="tooltip"
    >
      <div className="font-semibold mb-1">{formatDate(point.timestamp)}</div>
      <div>
        Score <span className="font-mono">{point.score}</span>{" "}
        <span style={{ color: "var(--muted-foreground)" }}>(grade {point.grade})</span>
      </div>
      {delta !== null && (
        <div className="mt-0.5">
          Delta{" "}
          <span
            className="font-mono"
            style={{
              color:
                delta > 0
                  ? "var(--success)"
                  : delta < 0
                    ? "var(--severity-critical)"
                    : "var(--muted-foreground)",
            }}
          >
            {delta > 0 ? "+" : ""}
            {delta}
          </span>
        </div>
      )}
      {mover && (
        <div className="mt-0.5" style={{ color: "var(--muted-foreground)" }}>
          Driven by {mover.delta > 0 ? "+" : ""}
          {mover.delta} {mover.severity} finding{Math.abs(mover.delta) === 1 ? "" : "s"}
        </div>
      )}
    </div>
  );
}

export function ScoreTrendChart({
  contractId,
  adapter,
  className,
}: ScoreTrendChartProps) {
  const [range, setRange] = useState<Range>("30d");
  const adapterOption = useMemo(
    () => (adapter ? { adapter } : {}),
    [adapter],
  );
  const view = useScoreHistory(contractId, range, adapterOption);

  const chartData = useMemo(() => {
    return view.points.map((p, idx, all) => ({
      ...p,
      // ms key so recharts treats the axis as a true time axis even
      // though the timestamps round-trip as strings through JSON.
      t: new Date(p.timestamp).getTime(),
      previous: idx > 0 ? all[idx - 1] : null,
    }));
  }, [view.points]);

  const isSinglePoint = view.points.length === 1;
  const collapsedToAll =
    range !== "all" && view.effectiveRange === "all" && view.points.length > 0;

  return (
    <section
      className={`rounded-lg border p-6 ${className ?? ""}`}
      style={{
        borderColor: "var(--border)",
        backgroundColor: "var(--card)",
        color: "var(--card-foreground)",
      }}
      aria-labelledby="score-trend-heading"
    >
      <header className="flex items-start justify-between gap-4 mb-4">
        <div>
          <h3 id="score-trend-heading" className="text-sm font-semibold">
            Sanctity Score Trend
          </h3>
          <p className="text-xs mt-0.5" style={{ color: "var(--muted-foreground)" }}>
            Historical score for{" "}
            <span className="font-mono" style={{ color: "var(--foreground)" }}>
              {contractId}
            </span>
          </p>
        </div>

        <div
          role="group"
          aria-label="Select time range"
          className="inline-flex rounded-md border overflow-hidden"
          style={{ borderColor: "var(--border)" }}
        >
          {RANGES.map((r) => {
            const isActive = r.value === range;
            return (
              <button
                key={r.value}
                type="button"
                onClick={() => setRange(r.value)}
                aria-pressed={isActive}
                className="px-2.5 py-1 text-xs font-medium transition-colors focus:outline-none focus:ring-2"
                style={{
                  backgroundColor: isActive
                    ? "var(--primary)"
                    : "transparent",
                  color: isActive
                    ? "var(--primary-foreground)"
                    : "var(--muted-foreground)",
                }}
              >
                {r.label}
              </button>
            );
          })}
        </div>
      </header>

      {view.isLoading && (
        <div
          className="h-56 flex items-center justify-center text-sm"
          style={{ color: "var(--muted-foreground)" }}
          aria-busy="true"
          aria-live="polite"
        >
          Loading score history…
        </div>
      )}

      {!view.isLoading && view.isEmpty && (
        <div
          className="h-56 flex flex-col items-center justify-center text-sm gap-1"
          style={{ color: "var(--muted-foreground)" }}
        >
          <span>No history yet for this contract.</span>
          <span className="text-xs">
            Run an analysis to record the first datapoint.
          </span>
        </div>
      )}

      {!view.isLoading && !view.isEmpty && (
        <>
          <div className="h-56 w-full">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={chartData} margin={{ top: 8, right: 12, left: 0, bottom: 4 }}>
                <CartesianGrid
                  strokeDasharray="3 3"
                  stroke="var(--border)"
                  vertical={false}
                />
                {/* Severity overlays. Drawn before the line so the line wins
                    visual focus. Each band uses the matching CSS variable
                    with a low alpha so the colour is just a hint. */}
                {SCORE_BANDS.map((band) => (
                  <ReferenceArea
                    key={band.id}
                    y1={band.from}
                    y2={band.to}
                    strokeOpacity={0}
                    fill={`var(${band.cssVar})`}
                    fillOpacity={0.08}
                    ifOverflow="visible"
                  />
                ))}
                <XAxis
                  dataKey="t"
                  type="number"
                  domain={["dataMin", "dataMax"]}
                  scale="time"
                  tickFormatter={(t: number) => formatDate(new Date(t).toISOString())}
                  stroke="var(--muted-foreground)"
                  fontSize={11}
                  tickLine={false}
                  axisLine={{ stroke: "var(--border)" }}
                />
                <YAxis
                  domain={[0, 100]}
                  ticks={[0, 25, 50, 75, 100]}
                  stroke="var(--muted-foreground)"
                  fontSize={11}
                  tickLine={false}
                  axisLine={{ stroke: "var(--border)" }}
                  width={32}
                />
                <Tooltip
                  content={<TrendTooltip />}
                  cursor={{ stroke: "var(--muted-foreground)", strokeDasharray: "3 3" }}
                />
                <Line
                  type="monotone"
                  dataKey="score"
                  stroke="var(--primary)"
                  strokeWidth={2}
                  // A single datapoint becomes an annotated dot rather than a
                  // degenerate line. Recharts honours the dot prop in both modes
                  // so we can be explicit about size and stroke here.
                  dot={{ r: isSinglePoint ? 6 : 3, fill: "var(--primary)" }}
                  activeDot={{ r: 6, fill: "var(--primary)" }}
                  isAnimationActive
                  animationDuration={400}
                />
              </LineChart>
            </ResponsiveContainer>
          </div>

          <footer
            className="mt-3 flex flex-wrap items-center justify-between gap-2 text-xs"
            style={{ color: "var(--muted-foreground)" }}
          >
            <div className="flex items-center gap-3 flex-wrap" aria-label="Severity bands">
              {SCORE_BANDS.map((band) => (
                <span key={band.id} className="inline-flex items-center gap-1.5">
                  <span
                    className="w-2.5 h-2.5 rounded-sm"
                    style={{ backgroundColor: `var(${band.cssVar})`, opacity: 0.5 }}
                    aria-hidden="true"
                  />
                  {band.label}
                </span>
              ))}
            </div>

            <div className="text-right">
              {collapsedToAll && (
                <span>
                  Showing all {view.points.length} datapoint
                  {view.points.length === 1 ? "" : "s"}; selected range had no data.
                </span>
              )}
              {!collapsedToAll && view.latest && view.latestDelta !== null && (
                <span>
                  Latest{" "}
                  <span
                    className="font-mono"
                    style={{ color: "var(--foreground)" }}
                  >
                    {view.latest.score}
                  </span>{" "}
                  ({view.latestDelta > 0 ? "+" : ""}
                  {view.latestDelta} vs previous)
                </span>
              )}
              {isSinglePoint && (
                <span>First recorded scan; no trend to compare against yet.</span>
              )}
            </div>
          </footer>
        </>
      )}
    </section>
  );
}
