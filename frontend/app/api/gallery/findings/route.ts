import findingsData from '@/data/findings-showcase.json';
import { NextResponse } from 'next/server';

export async function GET() {
  try {
    return NextResponse.json(findingsData, {
      headers: {
        'Cache-Control': 'public, s-maxage=3600, stale-while-revalidate=86400',
      },
    });
  } catch (error) {
    return NextResponse.json(
      { error: 'Failed to fetch findings data' },
      { status: 500 }
    );
  }
}
