export type Severity = "critical" | "high" | "medium" | "low";

export interface SizeWarning {
  struct_name: string;
  estimated_size: number;
  limit: number;
  level?: "ExceedsLimit" | "ApproachingLimit";
}

export interface PanicIssue {
  function_name: string;
  issue_type: string;
  location: string;
}

export interface UnsafePattern {
  pattern_type: "Panic" | "Unwrap" | "Expect";
  line: number;
  snippet: string;
}

export interface ArithmeticIssue {
  function_name: string;
  operation: string;
  suggestion: string;
  location: string;
}

export interface CustomRuleMatch {
  rule_name: string;
  line: number;
  snippet: string;
}

export interface KaniVerificationMetrics {
  total_assertions: number;
  proven: number;
  failed: number;
  unreachable: number;
}

export interface PathNode {
  node_type: string;
  description: string;
  line: number;
}

export interface ExecutionPath {
  nodes: PathNode[];
  is_panic: boolean;
}

export interface SymbolicGraph {
  function_name: string;
  paths: ExecutionPath[];
}

export interface ScoreDeduction {
  category: string;
  amount: number;
  message: string;
}

export interface SanctityScore {
  total_score: number;
  security_score: number;
  verification_score: number;
  coverage_score: number;
  deductions: ScoreDeduction[];
}

export interface AnalysisReport {
  sanctity_score?: SanctityScore;
  size_warnings?: SizeWarning[];
  unsafe_patterns?: UnsafePattern[];
  auth_gaps?: string[];
  panic_issues?: PanicIssue[];
  arithmetic_issues?: ArithmeticIssue[];
  custom_rule_matches?: CustomRuleMatch[];
  kani_metrics?: KaniVerificationMetrics;
  symbolic_paths?: SymbolicGraph[];
}


export interface Finding {
  id: string;
  severity: Severity;
  category: string;
  title: string;
  location: string;
  snippet?: string;
  line?: number;
  suggestion?: string;
  raw: unknown;
}
