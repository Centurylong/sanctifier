// In-memory ScoreHistoryAdapter used until the shareable permalink
// persistence (issue #364, declared as a dependency on #378) lands.
//
// The adapter shape is intentionally async so the swap to a real backed
// implementation (KV store, indexedDB, the permalink JSON, etc.) is a
// drop-in replacement: no consumer code has to change. Stories and the
// dashboard reach the data through createInMemoryScoreHistoryAdapter and a
// fixtures map keyed by contractId.

import type {
  HistoricalScorePoint,
  ScoreHistory,
  ScoreHistoryAdapter,
} from "./types";

// Deterministically build a point on a given offset from a reference date.
// Centralising the construction keeps the fixtures readable and stops
// stories from drifting on grade boundaries by accident.
function point(
  daysAgo: number,
  score: number,
  breakdown: HistoricalScorePoint["breakdown"],
  base: Date = new Date("2026-06-19T12:00:00Z"),
): HistoricalScorePoint {
  const ts = new Date(base.getTime() - daysAgo * 24 * 60 * 60 * 1000);
  const grade: HistoricalScorePoint["grade"] =
    score >= 90 ? "A" : score >= 80 ? "B" : score >= 65 ? "C" : score >= 50 ? "D" : "F";
  return { timestamp: ts.toISOString(), score, grade, breakdown };
}

// Built-in fixtures. These cover the four canonical states called out in the
// application note: empty, single point, healthy trend, regression spike.
// New stories should add a key here and reference it by contractId.
export const DEFAULT_FIXTURES: Record<string, HistoricalScorePoint[]> = {
  "demo-empty": [],
  "demo-single": [
    point(0, 92, { critical: 0, high: 0, medium: 1, low: 2 }),
  ],
  "demo-healthy": [
    point(60, 62, { critical: 0, high: 2, medium: 4, low: 5 }),
    point(45, 68, { critical: 0, high: 1, medium: 5, low: 4 }),
    point(30, 74, { critical: 0, high: 1, medium: 3, low: 4 }),
    point(20, 79, { critical: 0, high: 1, medium: 2, low: 3 }),
    point(14, 82, { critical: 0, high: 0, medium: 3, low: 2 }),
    point(7, 86, { critical: 0, high: 0, medium: 2, low: 2 }),
    point(3, 89, { critical: 0, high: 0, medium: 1, low: 2 }),
    point(0, 92, { critical: 0, high: 0, medium: 1, low: 1 }),
  ],
  "demo-regression": [
    point(60, 88, { critical: 0, high: 0, medium: 2, low: 2 }),
    point(45, 90, { critical: 0, high: 0, medium: 1, low: 3 }),
    point(30, 87, { critical: 0, high: 0, medium: 2, low: 3 }),
    point(20, 83, { critical: 0, high: 1, medium: 1, low: 4 }),
    point(14, 71, { critical: 0, high: 2, medium: 3, low: 4 }),
    point(7, 58, { critical: 1, high: 2, medium: 4, low: 5 }),
    point(3, 44, { critical: 1, high: 3, medium: 5, low: 6 }),
    point(0, 39, { critical: 2, high: 3, medium: 4, low: 5 }),
  ],
};

export function createInMemoryScoreHistoryAdapter(
  fixtures: Record<string, HistoricalScorePoint[]> = DEFAULT_FIXTURES,
): ScoreHistoryAdapter {
  return {
    async get(contractId: string): Promise<ScoreHistory | null> {
      // Pretend latency on the seeded data is not free: the dashboard needs a
      // real Promise to drive its loading state. A microtask resolve is enough.
      const raw = fixtures[contractId];
      if (raw === undefined) return null;
      // Always emit points sorted by time so the chart never has to.
      const sorted = [...raw].sort(
        (a, b) =>
          new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime(),
      );
      return Promise.resolve({ contractId, points: sorted });
    },
  };
}
