# Frontend

The web interface for interacting with the Sanctifier suite.

## Tech Stack
- **Framework**: Next.js 14 (App Router)
- **Styling**: Tailwind CSS
- **Wallet Connection**: Freighter (via Stellar Wallets Kit)

## Getting Started

1. Install dependencies:
   ```bash
   npm install
   ```
2. Run development server:
   ```bash
   npm run dev
   ```
3. Open [http://localhost:3000](http://localhost:3000)

## Features
- Upload WASM files for analysis.
- View real-time security reports.
- Dashboard for tracked contracts.

## Run Sanctifier in the Browser (WASM)

The dashboard can run the Rust analysis engine directly in your browser using WebAssembly.
To build the WASM bundle locally:

```bash
# From the repository root
wasm-pack build tooling/sanctifier-wasm --release --target web --out-dir frontend/public/wasm
```

Then start the app:

```bash
cd frontend
npm run dev
```

Open the Dashboard and use the "Analyze Rust Source (Runs in Your Browser)" section.
