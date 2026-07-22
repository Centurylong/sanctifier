'use client';

import { Adopter } from '../lib/gallery-data';
import Link from 'next/link';
import { ExternalLink, CheckCircle } from 'lucide-react';

interface AdopterCardProps {
  adopter: Adopter;
}

export function AdopterCard({ adopter }: AdopterCardProps) {
  const categoryColors: Record<string, string> = {
    core: 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200',
    defi: 'bg-purple-100 text-purple-800 dark:bg-purple-900 dark:text-purple-200',
    infrastructure: 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200',
    governance: 'bg-orange-100 text-orange-800 dark:bg-orange-900 dark:text-orange-200',
    nft: 'bg-pink-100 text-pink-800 dark:bg-pink-900 dark:text-pink-200',
    other: 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-200',
  };

  return (
    <div className="border rounded-lg p-6 hover:shadow-lg transition-shadow bg-card">
      <div className="flex items-start justify-between mb-4">
        <div className="flex-1">
          <div className="flex items-center gap-2 mb-2">
            <h3 className="text-lg font-semibold">{adopter.name}</h3>
            {adopter.verified && (
              <CheckCircle className="w-5 h-5 text-green-600 dark:text-green-400" />
            )}
          </div>
          <p className="text-sm text-muted-foreground mb-3">{adopter.description}</p>
        </div>
      </div>

      <div className="flex flex-wrap gap-2 mb-4">
        <span className={`inline-flex items-center px-3 py-1 rounded-full text-xs font-medium ${categoryColors[adopter.category] || categoryColors.other}`}>
          {adopter.category}
        </span>
      </div>

      <div className="grid grid-cols-2 gap-4 mb-4 text-sm">
        <div>
          <div className="text-muted-foreground">Vulnerabilities Found</div>
          <div className="text-2xl font-bold">{adopter.findings_count}</div>
        </div>
        <div>
          <div className="text-muted-foreground">Total Issues</div>
          <div className="text-2xl font-bold">{adopter.vulnerabilities_found.length}</div>
        </div>
      </div>

      {adopter.notes && (
        <div className="bg-muted p-3 rounded mb-4 text-sm">
          <p className="text-muted-foreground">{adopter.notes}</p>
        </div>
      )}

      <div className="flex items-center justify-between pt-4 border-t">
        <span className="text-xs text-muted-foreground">
          Added {new Date(adopter.date_added).toLocaleDateString()}
        </span>
        <Link
          href={adopter.repository}
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex items-center gap-2 text-sm font-medium text-primary hover:underline"
        >
          View Repository
          <ExternalLink className="w-4 h-4" />
        </Link>
      </div>
    </div>
  );
}
