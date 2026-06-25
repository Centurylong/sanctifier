import fs from "fs/promises";
import path from "path";
import { randomBytes } from "crypto";
import type { AnalysisReport, Finding } from "../types";

const STORAGE_DIR = path.join(process.cwd(), "data", "reports");
const MAX_REPORTS = 10000;
const REPORT_EXPIRY_DAYS = 30;

export interface StoredReport {
  id: string;
  report: AnalysisReport;
  findings: Finding[];
  timestamp: number;
  expiresAt: number;
  sourceSnippet?: string;
}

/**
 * Generate a unique report ID
 */
export function generateReportId(): string {
  return randomBytes(16).toString("hex");
}

/**
 * Ensure the storage directory exists
 */
async function ensureStorageDir() {
  try {
    await fs.mkdir(STORAGE_DIR, { recursive: true });
  } catch (err) {
    console.error("Failed to create storage directory:", err);
  }
}

/**
 * Save a report to storage
 */
export async function saveReport(
  report: AnalysisReport,
  findings: Finding[],
  sourceSnippet?: string
): Promise<string> {
  await ensureStorageDir();

  const id = generateReportId();
  const timestamp = Date.now();
  const expiresAt = timestamp + REPORT_EXPIRY_DAYS * 24 * 60 * 60 * 1000;

  const stored: StoredReport = {
    id,
    report,
    findings,
    timestamp,
    expiresAt,
    sourceSnippet,
  };

  const filePath = path.join(STORAGE_DIR, `${id}.json`);
  await fs.writeFile(filePath, JSON.stringify(stored, null, 2), "utf-8");

  // Cleanup old reports to prevent unbounded growth
  await cleanupExpiredReports();

  return id;
}

/**
 * Retrieve a report by ID
 */
export async function getReport(id: string): Promise<StoredReport | null> {
  // Validate ID format to prevent directory traversal
  if (!/^[a-f0-9]{32}$/.test(id)) {
    return null;
  }

  const filePath = path.join(STORAGE_DIR, `${id}.json`);

  try {
    const data = await fs.readFile(filePath, "utf-8");
    const stored: StoredReport = JSON.parse(data);

    // Check if expired
    if (Date.now() > stored.expiresAt) {
      await fs.unlink(filePath).catch(() => {}); // Delete expired report
      return null;
    }

    return stored;
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") {
      return null;
    }
    throw err;
  }
}

/**
 * Clean up expired reports
 */
async function cleanupExpiredReports() {
  try {
    await ensureStorageDir();
    const files = await fs.readdir(STORAGE_DIR);
    const now = Date.now();
    let cleanedCount = 0;

    for (const file of files) {
      if (!file.endsWith(".json")) continue;

      const filePath = path.join(STORAGE_DIR, file);
      try {
        const data = await fs.readFile(filePath, "utf-8");
        const stored: StoredReport = JSON.parse(data);

        if (now > stored.expiresAt) {
          await fs.unlink(filePath);
          cleanedCount++;
        }
      } catch {
        // Skip corrupted files
      }
    }

    // If we have too many reports, delete the oldest ones
    if (files.length - cleanedCount > MAX_REPORTS) {
      const reports: Array<{ path: string; timestamp: number }> = [];

      for (const file of files) {
        if (!file.endsWith(".json")) continue;

        const filePath = path.join(STORAGE_DIR, file);
        try {
          const data = await fs.readFile(filePath, "utf-8");
          const stored: StoredReport = JSON.parse(data);
          if (Date.now() <= stored.expiresAt) {
            reports.push({ path: filePath, timestamp: stored.timestamp });
          }
        } catch {
          // Skip
        }
      }

      // Sort by timestamp and delete oldest
      reports.sort((a, b) => a.timestamp - b.timestamp);
      const toDelete = reports.slice(0, reports.length - MAX_REPORTS);

      for (const { path: filePath } of toDelete) {
        await fs.unlink(filePath).catch(() => {});
      }
    }
  } catch (err) {
    console.error("Cleanup failed:", err);
  }
}
