import { NextRequest, NextResponse } from "next/server";
import { spawn } from "child_process";
import type { AnalysisReport, UnsafePattern, ArithmeticIssue, PanicIssue } from "../../types";
import { saveReport } from "../../lib/report-storage";
import { transformReport } from "../../lib/transform";

// ---------------------------------------------------------------------------
// GET – streaming terminal endpoint (existing behaviour)
// ---------------------------------------------------------------------------
export async function GET(request: NextRequest) {
  const searchParams = request.nextUrl.searchParams;
  const projectPath = searchParams.get("path") || ".";

  const encoder = new TextEncoder();
  const stream = new ReadableStream({
    start(controller) {
      const cliProcess = spawn(
        "cargo",
        ["run", "--bin", "sanctifier", "--", "analyze", "--path", projectPath],
        {
          cwd: "/home/rampop/Sanctifier",
          env: { ...process.env, FORCE_COLOR: "0" },
        }
      );

      const sendLog = (data: string) => {
        const lines = data.split("\n");
        for (const line of lines) {
          if (line.trim()) {
            controller.enqueue(
              encoder.encode(`data: ${JSON.stringify(line)}\n\n`)
            );
          }
        }
      };

      cliProcess.stdout.on("data", (data) => sendLog(data.toString()));
      cliProcess.stderr.on("data", (data) =>
        sendLog(`[DEBUG] ${data.toString()}`)
      );
      cliProcess.on("close", (code) => {
        controller.enqueue(
          encoder.encode(
            `data: ${JSON.stringify(`--- Analysis complete with exit code ${code} ---`)}\n\n`
          )
        );
        controller.close();
      });
      cliProcess.on("error", (err) => {
        controller.enqueue(
          encoder.encode(
            `data: ${JSON.stringify(`Error spawning process: ${err.message}`)}\n\n`
          )
        );
        controller.close();
      });
    },
  });

  return new Response(stream, {
    headers: {
      "Content-Type": "text/event-stream",
      "Cache-Control": "no-cache",
      Connection: "keep-alive",
    },
  });
}

// ---------------------------------------------------------------------------
// POST – scan submitted source code and return an AnalysisReport
// ---------------------------------------------------------------------------

const MAX_SOURCE_BYTES = 512 * 1024; // 512 KB
const ALLOWED_EXTENSIONS = [".rs", ".zip"];

export async function POST(request: NextRequest) {
  try {
    let source = "";
    let filename = "contract.rs";

    const ct = request.headers.get("content-type") ?? "";

    if (ct.includes("multipart/form-data")) {
      const form = await request.formData();

      const fileField = form.get("file");
      if (fileField instanceof File) {
        const ext = "." + fileField.name.split(".").pop()?.toLowerCase();
        if (!ALLOWED_EXTENSIONS.includes(ext)) {
          return NextResponse.json(
            { error: `Only ${ALLOWED_EXTENSIONS.join(", ")} files are allowed.` },
            { status: 400 }
          );
        }
        if (fileField.size > MAX_SOURCE_BYTES) {
          return NextResponse.json(
            { error: "File too large — maximum size is 512 KB." },
            { status: 400 }
          );
        }
        filename = fileField.name;
        if (ext === ".zip") {
          return NextResponse.json(
            {
              error:
                "Zip scanning via the web UI is not yet supported. " +
                "Extract the archive and paste the .rs source, or run: sanctifier analyze --path ./your-contract",
            },
            { status: 422 }
          );
        }
        source = await fileField.text();
      } else {
        const sourceField = form.get("source");
        if (typeof sourceField !== "string" || !sourceField.trim()) {
          return NextResponse.json(
            { error: "No source code provided." },
            { status: 400 }
          );
        }
        source = sourceField;
      }
    } else {
      // application/json
      const body = (await request.json()) as { source?: string };
      if (!body.source?.trim()) {
        return NextResponse.json(
          { error: "No source code provided." },
          { status: 400 }
        );
      }
      source = body.source;
    }

    if (new TextEncoder().encode(source).length > MAX_SOURCE_BYTES) {
      return NextResponse.json(
        { error: "Source too large — maximum size is 512 KB." },
        { status: 400 }
      );
    }

    const report = analyzeRustSource(source);
    const findings = transformReport(report);
    
    // Save report and get shareable ID
    const sourceSnippet = source.slice(0, 500); // First 500 chars for preview
    const reportId = await saveReport(report, findings, sourceSnippet);
    
    return NextResponse.json({
      ...report,
      reportId,
    });
  } catch (err) {
    const message = err instanceof Error ? err.message : "Unknown error";
    return NextResponse.json(
      { error: `Analysis failed: ${message}` },
      { status: 500 }
    );
  }
}

// ---------------------------------------------------------------------------
// Lightweight static analysis (mirrors sanctifier-core heuristics in JS)
// ---------------------------------------------------------------------------

function analyzeRustSource(source: string): AnalysisReport {
  const lines = source.split("\n");

  const unsafePatterns: UnsafePattern[] = [];
  const panicIssues: PanicIssue[] = [];
  const arithmeticIssues: ArithmeticIssue[] = [];
  const authGaps: string[] = [];

  // ── Pass 1: line-by-line patterns ─────────────────────────────────────────
  for (let i = 0; i < lines.length; i++) {
    const lineNo = i + 1;
    const line = lines[i];
    const trimmed = line.trim();

    // panic! / unwrap / expect
    if (/\bpanic!\s*\(/.test(line)) {
      panicIssues.push({
        function_name: nearestFunctionName(lines, i),
        issue_type: "panic!",
        location: `line ${lineNo}`,
      });
    }
    if (/\.unwrap\s*\(\)/.test(line)) {
      unsafePatterns.push({
        pattern_type: "Unwrap",
        line: lineNo,
        snippet: trimmed.slice(0, 120),
      });
    }
    if (/\.expect\s*\(/.test(line)) {
      unsafePatterns.push({
        pattern_type: "Expect",
        line: lineNo,
        snippet: trimmed.slice(0, 120),
      });
    }

    // Unchecked arithmetic: integer literal arithmetic without checked_* variants
    if (
      /\bi(8|16|32|64|128|size)\b/.test(line) &&
      /[+\-*]/.test(line) &&
      !/\.checked_(add|sub|mul|div|rem)/.test(line) &&
      !/\/\//.test(trimmed.slice(0, trimmed.search(/[+\-*]/)))
    ) {
      const op = (line.match(/[+\-*]/) ?? [])[0] ?? "+";
      const opName = op === "+" ? "addition" : op === "-" ? "subtraction" : "multiplication";
      arithmeticIssues.push({
        function_name: nearestFunctionName(lines, i),
        operation: opName,
        suggestion: `Use checked_${opName.replace("tion", "").replace("rac", "sub")}() or the soroban_sdk overflow-safe helpers to avoid integer overflow.`,
        location: `line ${lineNo}`,
      });
    }
  }

  // ── Pass 2: function-level auth gap detection ─────────────────────────────
  // Collect all `pub fn` blocks and check if they mutate storage without require_auth
  const pubFnRe = /\bpub\s+fn\s+(\w+)\s*\(/g;
  let m: RegExpExecArray | null;
  while ((m = pubFnRe.exec(source)) !== null) {
    const fnName = m[1];
    const bodyStart = source.indexOf("{", m.index);
    if (bodyStart === -1) continue;

    // Collect the function body by brace-matching
    let depth = 0;
    let bodyEnd = bodyStart;
    for (let k = bodyStart; k < source.length; k++) {
      if (source[k] === "{") depth++;
      else if (source[k] === "}") {
        depth--;
        if (depth === 0) {
          bodyEnd = k;
          break;
        }
      }
    }
    const body = source.slice(bodyStart, bodyEnd + 1);

    const mutatesStorage =
      /env\.storage\(\)\s*\.\s*(instance|persistent|temporary)\s*\(\)\s*\.\s*set\b/.test(body) ||
      /storage\(\)\s*\.\s*set\b/.test(body);

    const hasAuth =
      /require_auth|verify_auth|authorize|env\.require_auth/.test(body);

    if (mutatesStorage && !hasAuth) {
      // Find the line number of the fn keyword
      const lineNo =
        source.slice(0, m.index).split("\n").length;
      authGaps.push(`line ${lineNo}: ${fnName}`);
    }
  }

  return {
    auth_gaps: authGaps,
    panic_issues: panicIssues,
    arithmetic_issues: deduplicateArithmetic(arithmeticIssues),
    size_warnings: [],
    unsafe_patterns: unsafePatterns,
  };
}

function nearestFunctionName(lines: string[], fromIndex: number): string {
  for (let i = fromIndex; i >= 0; i--) {
    const m = lines[i].match(/\bfn\s+(\w+)\s*\(/);
    if (m) return m[1];
  }
  return "<unknown>";
}

function deduplicateArithmetic(issues: ArithmeticIssue[]): ArithmeticIssue[] {
  const seen = new Set<string>();
  return issues.filter((issue) => {
    const key = `${issue.function_name}:${issue.operation}:${issue.location}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}
