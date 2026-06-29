import initWasm, * as wasmModule from "../wasm/web/sanctifier.js";
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

const wasm = wasmModule as unknown as SanctifierWasm;

let initialized = false;
let initPromise: Promise<void> | null = null;

// Browsers need WASM bytes from a URL or BufferSource before any Rust
// function can run. Default is the bundled sanctifier_bg.wasm next to
// the JS glue, which works with most bundlers out of the box.
export type InitInput =
  | RequestInfo
  | URL
  | Response
  | BufferSource
  | WebAssembly.Module;

export async function init(input?: InitInput): Promise<void> {
  if (initialized) return;
  if (initPromise) {
    await initPromise;
    return;
  }
  initPromise = (async () => {
    await initWasm(input as Parameters<typeof initWasm>[0]);
    initialized = true;
  })();
  await initPromise;
}

function assertInitialized() {
  if (!initialized) {
    throw new Error(
      "Sanctifier WASM is not initialized. Call `await init()` first or pass the wasm URL/bytes to init().",
    );
  }
}

export async function analyze(
  source: string,
  options: AnalyzeOptions = {},
): Promise<AnalysisReport> {
  assertInitialized();
  return runAnalyze(wasm, source, options);
}

export function findingCodes(): FindingCodeEntry[] {
  assertInitialized();
  return listFindingCodes(wasm);
}

export function coreVersion(): string {
  assertInitialized();
  return getCoreVersion(wasm);
}
