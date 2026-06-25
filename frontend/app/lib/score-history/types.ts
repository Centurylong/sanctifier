// Public types for the Sanctity Score time-series chart on /dashboard.
//
// The chart reads from a typed adapter (see ./adapter.ts) so the data path
// can swap from the in-memory fixture used today to the persisted source
// shipped with the shareable permalink work without touching component code.

import type { Severity } from "../../types";

export type Range = "7d" | "30d" | "all";

export const RANGE_DAYS: Record<Exclude<Range, "all">, number> = {
  "7d": 7,
  "30d": 30,
};

// One observation of the score for a contract or repo at a point in time.
// breakdown is the per-severity count at that moment; the chart uses the
// delta against the previous point to label what drove a change in tooltips.
export interface HistoricalScorePoint {
  // ISO 8601 timestamp. Stored as a string so it survives JSON serialization
  // through whatever persistence backend the permalink work lands on.
  timestamp: string;
  score: number;
  grade: "A" | "B" | "C" | "D" | "F";
  breakdown: Record<Severity, number>;
}

export interface ScoreHistory {
  contractId: string;
  points: HistoricalScorePoint[];
}

export interface ScoreHistoryAdapter {
  get(contractId: string): Promise<ScoreHistory | null>;
}

// Sanctity score severity bands. Mirrors the thresholds used by the existing
// SanctityScore gauge so the chart's overlay reads the same way as the
// gauge: green is healthy, amber is moderate, orange is high risk, red is
// critical. The overlay renders as recharts ReferenceArea bands.
export interface ScoreBand {
  id: "critical" | "high" | "moderate" | "healthy";
  from: number;
  to: number;
  cssVar: string;
  label: string;
}

export const SCORE_BANDS: ScoreBand[] = [
  { id: "critical", from: 0, to: 40, cssVar: "--severity-critical", label: "Critical" },
  { id: "high", from: 40, to: 60, cssVar: "--severity-high", label: "High risk" },
  { id: "moderate", from: 60, to: 75, cssVar: "--warning", label: "Moderate" },
  { id: "healthy", from: 75, to: 100, cssVar: "--success", label: "Healthy" },
];
