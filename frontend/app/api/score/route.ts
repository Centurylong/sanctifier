import { NextRequest, NextResponse } from "next/server";
import { DEFAULT_FIXTURES } from "../../lib/score-history/adapter";
import { rateLimit } from "../../lib/rate-limit";

export async function GET(request: NextRequest) {
  try {
    const ip = request.headers.get("x-forwarded-for") || "127.0.0.1";
    if (!rateLimit(ip, 30, 60000)) { // 30 reqs per minute
      return NextResponse.json({ error: "Too many requests" }, { status: 429 });
    }

    const searchParams = request.nextUrl.searchParams;
    const contractId = searchParams.get("contractId");
    
    if (!contractId) {
      return NextResponse.json({ error: "contractId query parameter is required" }, { status: 400 });
    }
    
    const raw = DEFAULT_FIXTURES[contractId];
    if (!raw) {
      return NextResponse.json({ error: "Score history not found" }, { status: 404 });
    }
    
    const sorted = [...raw].sort(
      (a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime()
    );
    
    return NextResponse.json({ contractId, points: sorted });
  } catch (err) {
    console.error("Error fetching score:", err);
    return NextResponse.json({ error: "Failed to retrieve score" }, { status: 500 });
  }
}
