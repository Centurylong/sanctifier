import type { Metadata } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import "./globals.css";
import { ThemeProvider } from "./providers/theme-provider";

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  title: "Sanctifier | Soroban Security Suite",
  description: "Comprehensive security analysis and formal verification for Stellar Soroban smart contracts",
  keywords: ["Stellar", "Soroban", "Security", "Smart Contracts", "Blockchain", "Formal Verification"],
  authors: [{ name: "Sanctifier Team" }],
  openGraph: {
    title: "Sanctifier | Soroban Security Suite",
    description: "Comprehensive security analysis and formal verification for Stellar Soroban smart contracts",
    type: "website",
    locale: "en_US",
    siteName: "Sanctifier",
  },
  twitter: {
    card: "summary_large_image",
    title: "Sanctifier | Soroban Security Suite",
    description: "Comprehensive security analysis and formal verification for Stellar Soroban smart contracts",
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body
        className={`${geistSans.variable} ${geistMono.variable} antialiased`}
      >
        <a href="#main-content" className="skip-link">
          Skip to main content
        </a>
        <ThemeProvider>{children}</ThemeProvider>
      </body>
    </html>
  );
}
