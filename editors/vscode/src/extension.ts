import {
  commands,
  Diagnostic,
  DiagnosticCollection,
  DiagnosticSeverity,
  ExtensionContext,
  languages,
  Range,
  TextDocument,
  Uri,
  window,
  workspace,
} from "vscode";
import {
  analyze,
  type AnalyzeOptions,
  type Finding,
  SanctifierError,
} from "@sanctifier/sdk";

const DIAGNOSTIC_SOURCE = "Sanctifier";
const FINDING_DOCS_BASE = "https://github.com/Centurylong/sanctifier/blob/main/docs/error-codes.md";
let diagnostics: DiagnosticCollection;

function getFindingDocLink(code: string): string {
  return `${FINDING_DOCS_BASE}#${code.toLowerCase()}`;
}

function severityToDiagnosticSeverity(severity: string): DiagnosticSeverity {
  switch (severity) {
    case "critical":
    case "high":
      return DiagnosticSeverity.Error;
    case "medium":
    case "low":
      return DiagnosticSeverity.Warning;
    case "info":
    default:
      return DiagnosticSeverity.Information;
  }
}

function shouldAnalyzeDocument(document: TextDocument): boolean {
  if (document.isUntitled) {
    return false;
  }
  const language = document.languageId.toLowerCase();
  const path = document.fileName.toLowerCase();
  return language === "rust" || path.endsWith(".rs");
}

function buildDiagnostic(finding: Finding, document: TextDocument): Diagnostic {
  const line = Math.max(0, (finding.line ?? 1) - 1);
  const cappedLine = Math.min(line, document.lineCount - 1);
  const textLine = document.lineAt(cappedLine).text;
  const range = new Range(cappedLine, 0, cappedLine, textLine.length);

  const diagnostic = new Diagnostic(
    range,
    `${finding.message} (${finding.code})`,
    severityToDiagnosticSeverity(finding.severity),
  );

  diagnostic.source = DIAGNOSTIC_SOURCE;
  diagnostic.code = {
    value: finding.code,
    target: Uri.parse(getFindingDocLink(finding.code)),
  };
  return diagnostic;
}

function extractDiagnosticsFromError(error: unknown, document: TextDocument): Diagnostic[] {
  if (error instanceof SanctifierError && error.report?.findings) {
    return error.report.findings.map((finding: Finding) => buildDiagnostic(finding, document));
  }
  return [];
}

async function runAnalysis(document: TextDocument): Promise<void> {
  if (!shouldAnalyzeDocument(document)) {
    diagnostics.delete(document.uri);
    return;
  }

  const config = workspace.getConfiguration("sanctifier-vscode");
  if (!config.get<boolean>("enabled", true)) {
    diagnostics.delete(document.uri);
    return;
  }

  if (!config.get<boolean>("analyzeOnSave", true)) {
    diagnostics.delete(document.uri);
    return;
  }

  const options: AnalyzeOptions = {};
  const failOn = config.get<string>("failOn", "none");
  if (failOn && failOn !== "none") {
    options.failOn = failOn as any;
  }

  const enabledRules = config.get<string[]>("enabledRules", []);
  if (enabledRules.length > 0) {
    options.enabledRules = enabledRules;
  }

  try {
    const report = await analyze(document.getText(), options);
    const documentDiagnostics = report.findings.map((finding) => buildDiagnostic(finding, document));
    diagnostics.set(document.uri, documentDiagnostics);
  } catch (error: unknown) {
    const documentDiagnostics = extractDiagnosticsFromError(error, document);
    if (documentDiagnostics.length > 0) {
      diagnostics.set(document.uri, documentDiagnostics);
    } else {
      diagnostics.delete(document.uri);
    }

    const message = error instanceof Error ? error.message : String(error);
    window.showErrorMessage(`Sanctifier analysis failed: ${message}`);
  }
}

export async function activate(context: ExtensionContext): Promise<void> {
  diagnostics = languages.createDiagnosticCollection("sanctifier");
  context.subscriptions.push(diagnostics);

  context.subscriptions.push(
    workspace.onDidSaveTextDocument(async (document) => {
      if (shouldAnalyzeDocument(document)) {
        await runAnalysis(document);
      }
    }),
  );

  context.subscriptions.push(
    workspace.onDidOpenTextDocument(async (document) => {
      if (shouldAnalyzeDocument(document)) {
        await runAnalysis(document);
      }
    }),
  );

  context.subscriptions.push(
    workspace.onDidCloseTextDocument((document) => {
      diagnostics.delete(document.uri);
    }),
  );

  context.subscriptions.push(
    workspace.onDidChangeTextDocument(async (event) => {
      if (event.document === window.activeTextEditor?.document && shouldAnalyzeDocument(event.document)) {
        await runAnalysis(event.document);
      }
    }),
  );

  context.subscriptions.push(
    workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("sanctifier-vscode") && window.activeTextEditor) {
        void runAnalysis(window.activeTextEditor.document);
      }
    }),
  );

  context.subscriptions.push(
    workspace.onDidChangeWorkspaceFolders(() => {
      diagnostics.clear();
    }),
  );

  context.subscriptions.push(
    commands.registerCommand("sanctifier-vscode.analyzeActiveDocument", async () => {
      const activeDocument = window.activeTextEditor?.document;
      if (!activeDocument || !shouldAnalyzeDocument(activeDocument)) {
        window.showInformationMessage("No Rust document is active for Sanctifier analysis.");
        return;
      }
      await runAnalysis(activeDocument);
      window.showInformationMessage("Sanctifier analysis complete.");
    }),
  );
}

export function deactivate(): void {
  diagnostics?.dispose();
}
