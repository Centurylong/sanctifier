import type { AnalysisReport } from "../types";

type WasmModule = {
  default?: (input?: unknown) => Promise<unknown> | unknown;
  analyze: (src: string) => AnalysisReport;
  analyze_with_config: (cfg: string, src: string) => AnalysisReport;
};

export async function analyzeSourceInBrowser(source: string): Promise<AnalysisReport> {
  const mod = (await import(
    /* webpackIgnore: true */ "/wasm/sanctifier_wasm.js"
  )) as unknown as WasmModule;
  if (typeof mod?.default === "function") {
    await mod.default();
  }
  const result = mod.analyze(source);
  return result;
}

export async function analyzeSourceWithConfigInBrowser(
  configJson: string,
  source: string
): Promise<AnalysisReport> {
  const mod = (await import(
    /* webpackIgnore: true */ "/wasm/sanctifier_wasm.js"
  )) as unknown as WasmModule;
  if (typeof mod?.default === "function") {
    await mod.default();
  }
  return mod.analyze_with_config(configJson, source);
}
