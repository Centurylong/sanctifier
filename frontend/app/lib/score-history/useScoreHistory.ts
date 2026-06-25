"use client";

// useScoreHistory reads a contract's score series from the adapter and
// applies the active range. The hook owns three pieces of UX subtlety the
// chart should not have to know about:
//
//   1. The effective range collapses to "all" when the dataset is shorter
//      than the requested window, so a brand new repo does not render an
//      empty 7d chart with a blank canvas.
//   2. The latest point and the prior point are surfaced together so the
//      tooltip can show the delta and the category that drove the change
//      without recomputing it on every hover.
//   3. The hook is async-aware: empty during load, then populated. Stories
//      and the dashboard branch on `isLoading` and `isEmpty` without
//      caring about Promise plumbing.

import { useEffect, useMemo, useState } from "react";
import { createInMemoryScoreHistoryAdapter } from "./adapter";
import {
  RANGE_DAYS,
  type HistoricalScorePoint,
  type Range,
  type ScoreHistoryAdapter,
} from "./types";

export interface ScoreHistoryView {
  // Points after the range filter has been applied, sorted ascending by
  // timestamp. Empty when the contract has no history at all.
  points: HistoricalScorePoint[];
  // The range that is actually being displayed. Differs from the requested
  // range when the dataset is shorter than the window so the UI can call
  // that out (a small "showing all 3 days" hint, say).
  effectiveRange: Range;
  isLoading: boolean;
  isEmpty: boolean;
  // Convenience handles for the tooltip. latestDelta is positive when the
  // score improved against the previous point, negative for a regression.
  latest: HistoricalScorePoint | null;
  previous: HistoricalScorePoint | null;
  latestDelta: number | null;
}

interface UseScoreHistoryOptions {
  // Defaults to an in-memory adapter so a consumer can call the hook with
  // just a contract id. Pass a real adapter once the permalink work lands.
  adapter?: ScoreHistoryAdapter;
}

function filterByRange(
  points: HistoricalScorePoint[],
  range: Range,
): HistoricalScorePoint[] {
  if (range === "all" || points.length === 0) return points;
  const cutoffMs =
    Date.now() - RANGE_DAYS[range] * 24 * 60 * 60 * 1000;
  return points.filter((p) => new Date(p.timestamp).getTime() >= cutoffMs);
}

// Decide which range to actually render. The principle is: never show an
// emptier chart than the data supports. If the requested range filters
// everything out, fall back to "all" so the chart shows the full series
// instead of a blank canvas.
function resolveEffectiveRange(
  points: HistoricalScorePoint[],
  requested: Range,
): Range {
  if (requested === "all") return "all";
  const filtered = filterByRange(points, requested);
  return filtered.length > 0 ? requested : "all";
}

export function useScoreHistory(
  contractId: string,
  range: Range,
  options: UseScoreHistoryOptions = {},
): ScoreHistoryView {
  const adapter = useMemo(
    () => options.adapter ?? createInMemoryScoreHistoryAdapter(),
    [options.adapter],
  );

  // Derived loading state: we are loading whenever the most recently
  // resolved contract id does not match the requested one. That way the
  // effect produces exactly one setState call (when the fetch resolves),
  // which keeps the React Compiler's set-state-in-effect rule happy.
  const [resolved, setResolved] = useState<{
    key: string;
    points: HistoricalScorePoint[];
  } | null>(null);

  useEffect(() => {
    let cancelled = false;
    adapter
      .get(contractId)
      .then((history) => {
        if (cancelled) return;
        setResolved({ key: contractId, points: history?.points ?? [] });
      })
      .catch(() => {
        if (cancelled) return;
        // Adapter failures degrade to "no data" rather than crashing the
        // dashboard. The shareable permalink work can introduce a richer
        // error surface once it lands.
        setResolved({ key: contractId, points: [] });
      });
    return () => {
      cancelled = true;
    };
  }, [adapter, contractId]);

  return useMemo(() => {
    const isLoading = resolved?.key !== contractId;
    const points = isLoading ? [] : (resolved?.points ?? []);
    const effectiveRange = resolveEffectiveRange(points, range);
    const visible = filterByRange(points, effectiveRange);
    const latest = visible.length > 0 ? visible[visible.length - 1] : null;
    const previous =
      visible.length > 1 ? visible[visible.length - 2] : null;
    const latestDelta =
      latest && previous ? latest.score - previous.score : null;

    return {
      points: visible,
      effectiveRange,
      isLoading,
      isEmpty: !isLoading && visible.length === 0,
      latest,
      previous,
      latestDelta,
    };
  }, [resolved, contractId, range]);
}

// Pure helpers exported for testing and stories.
export const _internal = {
  filterByRange,
  resolveEffectiveRange,
};
