import adoptorsData from '@/data/adopters.json';
import { NextResponse } from 'next/server';

export async function GET() {
  try {
    return NextResponse.json(adoptorsData, {
      headers: {
        'Cache-Control': 'public, s-maxage=3600, stale-while-revalidate=86400',
      },
    });
  } catch (error) {
    return NextResponse.json(
      { error: 'Failed to fetch adopters data' },
      { status: 500 }
    );
  }
}
