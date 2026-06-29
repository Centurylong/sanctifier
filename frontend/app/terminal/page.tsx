"use client";

import { useState, useCallback } from "react";
import { AnalysisTerminal } from "../components/AnalysisTerminal";
import Link from "next/link";
import { ThemeToggle } from "../components/ThemeToggle";
import { ErrorBoundary } from "../components/ErrorBoundary";

export default function TerminalPage() {
    const [logs, setLogs] = useState<string[]>([]);
    const [isAnalyzing, setIsAnalyzing] = useState(false);

    const startAnalysis = useCallback(() => {
        setLogs([]);
        setIsAnalyzing(true);

        const eventSource = new EventSource("/api/analyze?path=.");

        eventSource.onmessage = (event) => {
            const data = JSON.parse(event.data);
            setLogs((prev) => [...prev, data]);

            if (data.includes("Analysis complete")) {
                eventSource.close();
                setIsAnalyzing(false);
            }
        };

        eventSource.onerror = (err) => {
            console.error("EventSource failed:", err);
            setLogs((prev) => [...prev, "❌ Error: Connection lost or server error."]);
            eventSource.close();
            setIsAnalyzing(false);
        };

        return () => {
            eventSource.close();
        };
    }, []);

    return (
        <div className="min-h-screen" style={{ backgroundColor: "var(--background)", color: "var(--foreground)" }}>
            <header 
                className="border-b px-6 py-4 flex items-center justify-between"
                style={{ borderColor: "var(--border)", backgroundColor: "var(--card)" }}
                role="banner"
            >
                <div className="flex items-center gap-6">
                    <Link href="/" className="font-bold text-lg focus:outline-none focus:ring-2">
                        Sanctifier
                    </Link>
                    <span style={{ color: "var(--muted-foreground)" }}>Real-time Analysis</span>
                </div>
                <nav className="flex items-center gap-4" aria-label="Main navigation">
                    <Link
                        href="/dashboard"
                        className="text-sm font-medium transition-colors focus:outline-none focus:ring-2"
                        style={{ color: "var(--muted-foreground)" }}
                    >
                        Dashboard
                    </Link>
                    <ThemeToggle />
                </nav>
            </header>

            <main id="main-content" className="max-w-5xl mx-auto px-6 py-12 space-y-8">
                <div className="flex flex-col md:flex-row md:items-end justify-between gap-6">
                    <div className="space-y-2">
                        <h1 className="text-3xl font-bold tracking-tight">Analysis Terminal</h1>
                        <p className="max-w-2xl" style={{ color: "var(--muted-foreground)" }}>
                            Monitor your contract&apos;s security analysis in real-time. This interactive terminal
                            streams live logs directly from the Sanctifier core engine.
                        </p>
                    </div>

                    <button
                        onClick={startAnalysis}
                        disabled={isAnalyzing}
                        className="px-8 py-3 rounded-xl font-bold transition-all shadow-lg hover:shadow-xl active:scale-95 flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed focus:outline-none focus:ring-2"
                        style={{
                            backgroundColor: isAnalyzing ? "var(--muted)" : "var(--primary)",
                            color: isAnalyzing ? "var(--muted-foreground)" : "var(--primary-foreground)",
                        }}
                        aria-live="polite"
                        aria-busy={isAnalyzing}
                    >
                        {isAnalyzing ? (
                            <>
                                <div 
                                    className="w-4 h-4 border-2 border-t-transparent rounded-full animate-spin" 
                                    style={{ borderColor: "var(--muted-foreground)" }}
                                    aria-hidden="true"
                                />
                                Analyzing...
                            </>
                        ) : (
                            "Start New Analysis"
                        )}
                    </button>
                </div>

                <ErrorBoundary>
                    <section className="relative">
                        <div className="absolute -inset-1 rounded-2xl blur opacity-25" style={{ background: "linear-gradient(to right, var(--success), var(--info))" }}></div>
                        <AnalysisTerminal logs={logs} isAnalyzing={isAnalyzing} />
                    </section>
                </ErrorBoundary>

                <section className="grid grid-cols-1 md:grid-cols-3 gap-6">
                    <div 
                        className="p-6 rounded-2xl border shadow-sm"
                        style={{ borderColor: "var(--border)", backgroundColor: "var(--card)" }}
                    >
                        <div 
                            className="w-10 h-10 rounded-full flex items-center justify-center mb-4"
                            style={{ backgroundColor: "var(--success)", opacity: 0.1 }}
                        >
                            <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--success)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" /></svg>
                        </div>
                        <h3 className="font-bold mb-2">Live Scanning</h3>
                        <p className="text-sm" style={{ color: "var(--muted-foreground)" }}>
                            Watch as our engine traverses your Soroban contract code in real-time.
                        </p>
                    </div>
                    <div 
                        className="p-6 rounded-2xl border shadow-sm"
                        style={{ borderColor: "var(--border)", backgroundColor: "var(--card)" }}
                    >
                        <div 
                            className="w-10 h-10 rounded-full flex items-center justify-center mb-4"
                            style={{ backgroundColor: "var(--info)", opacity: 0.1 }}
                        >
                            <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--info)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><polyline points="16 18 22 12 16 6" /><polyline points="8 6 2 12 8 18" /></svg>
                        </div>
                        <h3 className="font-bold mb-2">Instant Feedback</h3>
                        <p className="text-sm" style={{ color: "var(--muted-foreground)" }}>
                            Get immediate diagnostic information without waiting for long build processes.
                        </p>
                    </div>
                    <div 
                        className="p-6 rounded-2xl border shadow-sm"
                        style={{ borderColor: "var(--border)", backgroundColor: "var(--card)" }}
                    >
                        <div 
                            className="w-10 h-10 rounded-full flex items-center justify-center mb-4"
                            style={{ backgroundColor: "var(--warning)", opacity: 0.1 }}
                        >
                            <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--warning)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline points="7 10 12 15 17 10" /><line x1="12" y1="15" x2="12" y2="3" /></svg>
                        </div>
                        <h3 className="font-bold mb-2">Export Logs</h3>
                        <p className="text-sm" style={{ color: "var(--muted-foreground)" }}>
                            Keep a record of your analysis sessions for compliance and auditing purposes.
                        </p>
                    </div>
                </section>
            </main>
        </div>
    );
}
