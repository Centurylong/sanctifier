import { NextRequest, NextResponse } from "next/server";
import { getReport } from "../../../lib/report-storage";

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  try {
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
