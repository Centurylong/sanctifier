/**
 * @sanctifier/wasm — Soroban contract analysis in the browser and at the edge.
 *
 * This is the JS layer over the wasm-bindgen exports. It exists for two
 * reasons:
 *
 * 1. The Rust export `analyze()` returns the whole analysis report. The
 *    documented API for this package is `analyze(source) -> Finding[]`, because
 *    that is what a caller almost always wants; the full report stays available
 *    as `analyzeReport()` for anyone who needs the per-detector breakdown.
 * 2. wasm-bindgen's `web` target requires an explicit async init before any
 *    export can be called, and its `nodejs` target does not. Callers should not
 *    have to care which one they got, so `init()` is always available, always
 *    safe to call, and always resolves once.
 */

let wasm = null;
let initPromise = null;

/** Resolve the wasm module for whichever environment this is running in. */
async function load() {
  if (typeof process !== "undefined" && process.versions && process.versions.node) {
    // The nodejs target is synchronous — requiring it is enough.
    return await import("../dist/node/sanctifier_wasm.js");
  }
  const mod = await import("../dist/web/sanctifier_wasm.js");
  await mod.default();
  return mod;
}

/**
 * Initialize the wasm module. Safe to call repeatedly and concurrently: the
 * first call starts the load, every later call awaits the same promise.
 *
 * Calling this explicitly before the first `analyze()` moves the one-time
 * compile cost somewhere you control — a page load rather than the keystroke
 * the user is waiting on.
 *
 * @returns {Promise<void>}
 */
export function init() {
  if (!initPromise) {
    initPromise = load().then((mod) => {
      wasm = mod;
    });
  }
  return initPromise;
}

function requireWasm() {
  if (!wasm) {
    throw new Error(
      "@sanctifier/wasm is not initialized — await init() before calling this function",
    );
  }
  return wasm;
}

/**
 * Analyze Soroban contract source and return the findings.
 *
 * @param {string} source - Rust source of a Soroban contract.
 * @returns {import("./index.d.ts").Finding[]}
 */
export function analyze(source) {
  return analyzeReport(source).findings;
}

/**
 * Analyze and return the full report: findings, severity summary, and the
 * per-detector raw output.
 *
 * @param {string} source
 * @param {import("./index.d.ts").SanctifyConfig} [config]
 * @returns {import("./index.d.ts").AnalysisReport}
 */
export function analyzeReport(source, config) {
  const w = requireWasm();
  if (typeof source !== "string") {
    throw new TypeError("source must be a string");
  }
  const report = config
    ? w.analyze_with_config(JSON.stringify(config), source)
    : w.analyze(source);

  // serde_wasm_bindgen returns JsValue::NULL if serialization fails rather
  // than throwing, so a null here is a real failure and not an empty result.
  if (report == null) {
    throw new Error("analysis failed: the wasm module returned no report");
  }
  return report;
}

/**
 * The catalog of finding codes this build can emit, for building filters or
 * documentation without having to run an analysis first.
 *
 * @returns {import("./index.d.ts").FindingCode[]}
 */
export function findingCodes() {
  return requireWasm().finding_code_catalog();
}

/**
 * Version of the underlying sanctifier-wasm crate.
 * @returns {string}
 */
export function version() {
  return requireWasm().version();
}
