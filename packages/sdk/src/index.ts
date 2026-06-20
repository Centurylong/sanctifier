// Shared public type surface. Both the node and browser entrypoints
// re-export from here so the published .d.ts is identical regardless
// of which runtime the consumer resolves.
export * from "./types.js";
export { SanctifierError } from "./errors.js";

import type {
  AnalysisReport,
  AnalyzeOptions,
  FindingCodeEntry,
} from "./types.js";

// These signatures match both the node and browser entries so callers
// can write environment-agnostic code that imports from "@sanctifier/sdk".
export declare function analyze(
  source: string,
  options?: AnalyzeOptions,
): Promise<AnalysisReport>;

export declare function findingCodes(): FindingCodeEntry[];

export declare function coreVersion(): string;

export declare function init(input?: unknown): Promise<void>;
