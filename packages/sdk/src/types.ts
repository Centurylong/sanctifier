// Finding severity. `info` is the lightest, `critical` the heaviest.
export type Severity = "info" | "low" | "medium" | "high" | "critical";

// Stable finding codes such as S001, S002 used by both CLI and SDK.
export type FindingCode =
  | "S001"
  | "S002"
  | "S003"
  | "S004"
  | "S005"
  | "S006"
  | "S007"
  | "S008"
  | "S009"
  | "S010"
  | "S011"
  | "S012"
  | "S013"
  | "S014"
  | "S015"
  | "S016"
  | string;

// A single issue surfaced by the analyzer.
export interface Finding {
  code: FindingCode;
  category: string;
  severity: Severity;
  message: string;
  location: string;
  function_name: string | null;
  line: number | null;
}

export interface SeveritySummary {
  total: number;
  critical: number;
  high: number;
  medium: number;
  low: number;
  info: number;
}

export interface SizeWarning {
  struct_name: string;
  estimated_size: number;
  limit: number;
  level: "ExceedsLimit" | "ApproachingLimit";
}

export type UnsafePatternType = "Panic" | "Unwrap" | "Expect";

export interface UnsafePattern {
  pattern_type: UnsafePatternType;
  line: number;
  snippet: string;
}

export interface PanicIssue {
  function_name: string;
  issue_type: string;
  location: string;
}

export interface ArithmeticIssue {
  function_name: string;
  operation: string;
  suggestion: string;
  location: string;
}

export interface StorageCollisionIssue {
  key_value: string;
  key_type: string;
  location: string;
  message: string;
}

export interface UnhandledResultIssue {
  function_name: string;
  call_expression: string;
  message: string;
  location: string;
}

export interface EventIssue {
  function_name: string;
  event_name: string;
  issue_type: "InconsistentSchema" | "OptimizableTopic" | string;
  message: string;
  location: string;
}

export type UpgradeCategory =
  | "admin_control"
  | "timelock"
  | "init_pattern"
  | "storage_layout"
  | "governance";

export interface UpgradeFinding {
  category: UpgradeCategory;
  function_name: string | null;
  location: string;
  message: string;
  suggestion: string;
}

export interface UpgradeReport {
  findings: UpgradeFinding[];
  upgrade_mechanisms: string[];
  init_functions: string[];
  storage_types: string[];
  suggestions: string[];
}

export type RuleSeverity = "info" | "warning" | "error";

export interface CustomRule {
  name: string;
  pattern: string;
  severity?: RuleSeverity;
}

export interface CustomRuleMatch {
  rule_name: string;
  line: number;
  snippet: string;
  severity: RuleSeverity;
}

export interface RawReport {
  size_warnings: SizeWarning[];
  unsafe_patterns: UnsafePattern[];
  auth_gaps: string[];
  panic_issues: PanicIssue[];
  arithmetic_issues: ArithmeticIssue[];
  storage_collisions: StorageCollisionIssue[];
  unhandled_results: UnhandledResultIssue[];
  event_issues: EventIssue[];
  upgrade_report: UpgradeReport;
  custom_rule_matches: CustomRuleMatch[];
}

export interface AnalysisReport {
  findings: Finding[];
  summary: SeveritySummary;
  raw: RawReport;
}

// Public options accepted by analyze(). failOn raises SanctifierError
// when any finding meets or exceeds the given severity threshold.
export interface AnalyzeOptions {
  failOn?: Severity;
  ledgerLimit?: number;
  approachingThreshold?: number;
  enabledRules?: string[];
  ignorePaths?: string[];
  customRules?: CustomRule[];
}

export interface FindingCodeEntry {
  code: string;
  category: string;
  description: string;
}

// Shape of the WASM module surface that both node and browser entrypoints
// expose to the shared core wrapper. Keeps the core agnostic to environment.
export interface SanctifierWasm {
  analyze(source: string): unknown;
  analyze_with_config(configJson: string, source: string): unknown;
  finding_code_catalog(): unknown;
  version(): string;
}
