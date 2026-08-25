/** Severity as reported by the analysis engine, highest first. Lowercase —
 *  these are the exact strings `sanctifier-core` serializes. */
export type Severity = "critical" | "high" | "medium" | "low" | "info";

export interface Finding {
  /** Stable finding code, e.g. "SANCT-AUTH-001". Safe to key rules off. */
  code: string;
  /** Detector family the finding came from. */
  category: string;
  severity: Severity;
  /** Human-readable description of what was found. */
  message: string;
  /** Where in the source, as rendered by the detector. */
  location: string;
  /** Enclosing function, when the detector could attribute one. */
  function_name?: string;
  /** 1-based line number, when the detector could attribute one. */
  line?: number;
}

export interface Summary {
  total: number;
  critical: number;
  high: number;
  medium: number;
  low: number;
  info: number;
}

/**
 * Per-detector output. Shapes mirror the `sanctifier-core` structs and are
 * typed loosely on purpose — they are a passthrough, and pinning them here
 * would mean this file has to change every time a detector gains a field.
 */
export interface RawReport {
  size_warnings: unknown[];
  unsafe_patterns: unknown[];
  auth_gaps: string[];
  panic_issues: unknown[];
  arithmetic_issues: unknown[];
  storage_collisions: unknown[];
  unhandled_results: unknown[];
  event_issues: unknown[];
  upgrade_report: unknown;
  custom_rule_matches: unknown[];
}

export interface AnalysisReport {
  findings: Finding[];
  summary: Summary;
  raw: RawReport;
}

export interface FindingCode {
  code: string;
  category: string;
  description: string;
}

/** Subset of `.sanctify.toml` that the wasm build honours. */
export interface SanctifyConfig {
  custom_rules?: unknown[];
  [key: string]: unknown;
}

/**
 * Load the wasm module. Safe to call repeatedly and concurrently.
 *
 * Call it explicitly on page load to move the one-time compile cost off the
 * keystroke the user is waiting on.
 */
export function init(): Promise<void>;

/** Analyze Soroban contract source and return the findings. */
export function analyze(source: string): Finding[];

/** Analyze and return findings, severity summary, and raw per-detector output. */
export function analyzeReport(source: string, config?: SanctifyConfig): AnalysisReport;

/** Every finding code this build can emit. */
export function findingCodes(): FindingCode[];

/** Version of the underlying sanctifier-wasm crate. */
export function version(): string;
