import type { AnalysisReport, Severity } from "./types.js";

// Thrown when analyze() finds at least one issue at or above the
// failOn severity threshold. The full report is attached so callers
// can render details after catching.
export class SanctifierError extends Error {
  readonly report: AnalysisReport;
  readonly threshold: Severity;

  constructor(message: string, report: AnalysisReport, threshold: Severity) {
    super(message);
    this.name = "SanctifierError";
    this.report = report;
    this.threshold = threshold;
  }
}
