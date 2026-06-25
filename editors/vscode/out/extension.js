"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode_1 = require("vscode");
const sdk_1 = require("@sanctifier/sdk");
const DIAGNOSTIC_SOURCE = "Sanctifier";
const FINDING_DOCS_BASE = "https://github.com/Centurylong/sanctifier/blob/main/docs/error-codes.md";
let diagnostics;
function getFindingDocLink(code) {
    return `${FINDING_DOCS_BASE}#${code.toLowerCase()}`;
}
function severityToDiagnosticSeverity(severity) {
    switch (severity) {
        case "critical":
        case "high":
            return vscode_1.DiagnosticSeverity.Error;
        case "medium":
        case "low":
            return vscode_1.DiagnosticSeverity.Warning;
        case "info":
        default:
            return vscode_1.DiagnosticSeverity.Information;
    }
}
function shouldAnalyzeDocument(document) {
    if (document.isUntitled) {
        return false;
    }
    const language = document.languageId.toLowerCase();
    const path = document.fileName.toLowerCase();
    return language === "rust" || path.endsWith(".rs");
}
function buildDiagnostic(finding, document) {
    const line = Math.max(0, (finding.line ?? 1) - 1);
    const cappedLine = Math.min(line, document.lineCount - 1);
    const textLine = document.lineAt(cappedLine).text;
    const range = new vscode_1.Range(cappedLine, 0, cappedLine, textLine.length);
    const diagnostic = new vscode_1.Diagnostic(range, `${finding.message} (${finding.code})`, severityToDiagnosticSeverity(finding.severity));
    diagnostic.source = DIAGNOSTIC_SOURCE;
    diagnostic.code = {
        value: finding.code,
        target: vscode_1.Uri.parse(getFindingDocLink(finding.code)),
    };
    return diagnostic;
}
function extractDiagnosticsFromError(error, document) {
    if (error instanceof sdk_1.SanctifierError && error.report?.findings) {
        return error.report.findings.map((finding) => buildDiagnostic(finding, document));
    }
    return [];
}
async function runAnalysis(document) {
    if (!shouldAnalyzeDocument(document)) {
        diagnostics.delete(document.uri);
        return;
    }
    const config = vscode_1.workspace.getConfiguration("sanctifier-vscode");
    if (!config.get("enabled", true)) {
        diagnostics.delete(document.uri);
        return;
    }
    if (!config.get("analyzeOnSave", true)) {
        diagnostics.delete(document.uri);
        return;
    }
    const options = {};
    const failOn = config.get("failOn", "none");
    if (failOn && failOn !== "none") {
        options.failOn = failOn;
    }
    const enabledRules = config.get("enabledRules", []);
    if (enabledRules.length > 0) {
        options.enabledRules = enabledRules;
    }
    try {
        const report = await (0, sdk_1.analyze)(document.getText(), options);
        const documentDiagnostics = report.findings.map((finding) => buildDiagnostic(finding, document));
        diagnostics.set(document.uri, documentDiagnostics);
    }
    catch (error) {
        const documentDiagnostics = extractDiagnosticsFromError(error, document);
        if (documentDiagnostics.length > 0) {
            diagnostics.set(document.uri, documentDiagnostics);
        }
        else {
            diagnostics.delete(document.uri);
        }
        const message = error instanceof Error ? error.message : String(error);
        vscode_1.window.showErrorMessage(`Sanctifier analysis failed: ${message}`);
    }
}
async function activate(context) {
    diagnostics = vscode_1.languages.createDiagnosticCollection("sanctifier");
    context.subscriptions.push(diagnostics);
    context.subscriptions.push(vscode_1.workspace.onDidSaveTextDocument(async (document) => {
        if (shouldAnalyzeDocument(document)) {
            await runAnalysis(document);
        }
    }));
    context.subscriptions.push(vscode_1.workspace.onDidOpenTextDocument(async (document) => {
        if (shouldAnalyzeDocument(document)) {
            await runAnalysis(document);
        }
    }));
    context.subscriptions.push(vscode_1.workspace.onDidCloseTextDocument((document) => {
        diagnostics.delete(document.uri);
    }));
    context.subscriptions.push(vscode_1.workspace.onDidChangeTextDocument(async (event) => {
        if (event.document === vscode_1.window.activeTextEditor?.document && shouldAnalyzeDocument(event.document)) {
            await runAnalysis(event.document);
        }
    }));
    context.subscriptions.push(vscode_1.workspace.onDidChangeConfiguration((event) => {
        if (event.affectsConfiguration("sanctifier-vscode") && vscode_1.window.activeTextEditor) {
            void runAnalysis(vscode_1.window.activeTextEditor.document);
        }
    }));
    context.subscriptions.push(vscode_1.workspace.onDidChangeWorkspaceFolders(() => {
        diagnostics.clear();
    }));
    context.subscriptions.push(vscode_1.commands.registerCommand("sanctifier-vscode.analyzeActiveDocument", async () => {
        const activeDocument = vscode_1.window.activeTextEditor?.document;
        if (!activeDocument || !shouldAnalyzeDocument(activeDocument)) {
            vscode_1.window.showInformationMessage("No Rust document is active for Sanctifier analysis.");
            return;
        }
        await runAnalysis(activeDocument);
        vscode_1.window.showInformationMessage("Sanctifier analysis complete.");
    }));
}
function deactivate() {
    diagnostics?.dispose();
}
//# sourceMappingURL=extension.js.map