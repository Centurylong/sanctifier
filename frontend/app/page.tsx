import Link from "next/link";
import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Home | Sanctifier",
  description: "Welcome to Sanctifier - Your comprehensive Stellar Soroban security analysis and formal verification suite",
  openGraph: {
    title: "Home | Sanctifier",
    description: "Welcome to Sanctifier - Your comprehensive Stellar Soroban security analysis and formal verification suite",
  },
};

export default function Home() {
  return (
    <div className="flex min-h-screen flex-col items-center justify-center font-sans" style={{ backgroundColor: "var(--background)", color: "var(--foreground)" }}>
      <main id="main-content" className="flex flex-col items-center gap-8 px-6">
        <h1 className="text-4xl font-bold">
          Sanctifier
        </h1>
        <p className="text-lg text-center max-w-md" style={{ color: "var(--muted-foreground)" }}>
          Stellar Soroban Security & Formal Verification Suite
        </p>
        <Link
          href="/dashboard"
          className="rounded-lg px-6 py-3 font-medium transition-colors focus:outline-none focus:ring-2 focus:ring-offset-2"
          style={{
            backgroundColor: "var(--primary)",
            color: "var(--primary-foreground)",
          }}
        >
          Open Security Dashboard
        </Link>
      </main>
    </div>
  );
}
