import { NextRequest, NextResponse } from "next/server";
import { getReport } from "../../../lib/report-storage";
import { rateLimit } from "../../../lib/rate-limit";

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  try {
    const ip = request.headers.get("x-forwarded-for") || "127.0.0.1";
    if (!rateLimit(ip, 30, 60000)) { // 30 reqs per minute
      return NextResponse.json({ error: "Too many requests" }, { status: 429 });
    }
    const { id } = await params;

    const report = await getReport(id);

    if (!report) {
      return NextResponse.json(
        { error: "Report not found or has expired" },
        { status: 404 }
      );
    }

    return NextResponse.json(report);
  } catch (err) {
    console.error("Error fetching report:", err);
    return NextResponse.json(
      { error: "Failed to retrieve report" },
      { status: 500 }
    );
  }
}
