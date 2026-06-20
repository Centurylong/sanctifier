import type { SavedReport, Finding, AnalysisReport } from "../types";

const STORAGE_KEY = "sanctifier_saved_reports";
const MAX_REPORTS = 20;

export function listSavedReports(): SavedReport[] {
  if (typeof window === "undefined") return [];
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? (JSON.parse(raw) as SavedReport[]) : [];
  } catch {
    return [];
  }
}

export function saveReport(
  findings: Finding[],
  report: AnalysisReport,
  label?: string
): SavedReport {
  const saved = listSavedReports();
  const id = `report-${Date.now()}`;
  const savedAt = new Date().toISOString();
  const entry: SavedReport = {
    id,
    label: label ?? `Report ${savedAt.slice(0, 16).replace("T", " ")}`,
    savedAt,
    findings,
    report,
  };

  // Prepend newest first, cap at max
  const updated = [entry, ...saved].slice(0, MAX_REPORTS);
  localStorage.setItem(STORAGE_KEY, JSON.stringify(updated));
  return entry;
}

export function getReport(id: string): SavedReport | undefined {
  return listSavedReports().find((r) => r.id === id);
}

export function deleteReport(id: string): void {
  const updated = listSavedReports().filter((r) => r.id !== id);
  localStorage.setItem(STORAGE_KEY, JSON.stringify(updated));
}
