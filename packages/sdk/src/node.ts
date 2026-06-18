import { createRequire } from "node:module";
import { getCoreVersion, listFindingCodes, runAnalyze } from "./core.js";
import { SanctifierError } from "./errors.js";
import type {
  AnalysisReport,
  AnalyzeOptions,
  FindingCodeEntry,
  SanctifierWasm,
} from "./types.js";

export type {
  AnalysisReport,
  AnalyzeOptions,
  ArithmeticIssue,
  CustomRule,
  CustomRuleMatch,
  EventIssue,
  Finding,
  FindingCode,
  FindingCodeEntry,
  PanicIssue,
  RawReport,
  RuleSeverity,
  SanctifierWasm,
  Severity,
  SeveritySummary,
  SizeWarning,
  StorageCollisionIssue,
  UnhandledResultIssue,
  UnsafePattern,
  UnsafePatternType,
  UpgradeCategory,
  UpgradeFinding,
  UpgradeReport,
} from "./types.js";

export { SanctifierError };

// The Node target of wasm-pack ships CommonJS, so it has to be required
// rather than ESM-imported. createRequire keeps this working in both
// ESM and CJS distributions emitted by tsup.
const require = createRequire(import.meta.url);
const wasm: SanctifierWasm = require("../../wasm/node/sanctifier.js");

// Analyze a Soroban contract source string and return the structured report.
// Rejects with SanctifierError when options.failOn is set and any finding
// meets or exceeds the given severity. async so a synchronous throw inside
// runAnalyze surfaces as a rejected promise instead of bubbling out.
export async function analyze(
  source: string,
  options: AnalyzeOptions = {},
): Promise<AnalysisReport> {
  return runAnalyze(wasm, source, options);
}

// Synchronous variant. Useful for CLIs and scripts that do not want
// to await a Promise. The async form is the documented entry.
export function analyzeSync(
  source: string,
  options: AnalyzeOptions = {},
): AnalysisReport {
  return runAnalyze(wasm, source, options);
}

export function findingCodes(): FindingCodeEntry[] {
  return listFindingCodes(wasm);
}

export function coreVersion(): string {
  return getCoreVersion(wasm);
}

// Node target exposes init as a no-op so user code can call init()
// in environment-agnostic library code without branching.
export function init(): Promise<void> {
  return Promise.resolve();
}
