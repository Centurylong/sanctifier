'use client';

import { Finding } from '../lib/gallery-data';
import Link from 'next/link';
import { AlertCircle, ExternalLink, Calendar } from 'lucide-react';

interface FindingCardProps {
  finding: Finding;
}

export function FindingCard({ finding }: FindingCardProps) {
  const severityColors: Record<string, string> = {
    critical: 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-200',
    high: 'bg-orange-100 text-orange-800 dark:bg-orange-900 dark:text-orange-200',
    medium: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-200',
    low: 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200',
  };

  const severityBorder: Record<string, string> = {
    critical: 'border-l-4 border-l-red-600',
    high: 'border-l-4 border-l-orange-600',
    medium: 'border-l-4 border-l-yellow-600',
    low: 'border-l-4 border-l-blue-600',
  };

  const daysToDisclosure = Math.floor(
    (new Date(finding.timeline.public_disclosure).getTime() - 
     new Date(finding.timeline.detected).getTime()) / (1000 * 60 * 60 * 24)
  );

  return (
    <div className={`border rounded-lg p-6 hover:shadow-lg transition-shadow bg-card ${severityBorder[finding.severity]}`}>
      <div className="flex items-start justify-between mb-4">
        <div className="flex-1">
          <div className="flex items-center gap-3 mb-2">
            <AlertCircle className="w-5 h-5 text-muted-foreground flex-shrink-0" />
            <h3 className="text-lg font-semibold">{finding.title}</h3>
          </div>
          <p className="text-sm text-muted-foreground">{finding.vulnerability_id}</p>
        </div>
        <span className={`inline-flex items-center px-3 py-1 rounded-full text-xs font-medium whitespace-nowrap ${severityColors[finding.severity]}`}>
          {finding.severity.toUpperCase()} - CVSS {finding.cvss}
        </span>
      </div>

      <p className="text-sm mb-4 leading-relaxed">{finding.description}</p>

      <div className="bg-red-50 dark:bg-red-950 border border-red-200 dark:border-red-800 rounded p-3 mb-4">
        <div className="text-sm font-semibold text-red-900 dark:text-red-100 mb-1">Impact</div>
        <p className="text-sm text-red-800 dark:text-red-200">{finding.impact}</p>
      </div>

      <div className="grid grid-cols-2 gap-4 mb-4 text-sm">
        <div>
          <div className="text-muted-foreground font-medium">Project</div>
          <div>{finding.project_name}</div>
        </div>
        <div>
          <div className="text-muted-foreground font-medium">Finding Code</div>
          <div className="font-mono text-xs bg-muted px-2 py-1 rounded w-fit">{finding.finding_code}</div>
        </div>
      </div>

      <div className="bg-muted p-3 rounded mb-4 text-sm">
        <div className="font-semibold mb-2">Timeline to Disclosure</div>
        <div className="space-y-1 text-xs">
          <div className="flex items-center gap-2">
            <Calendar className="w-4 h-4 text-muted-foreground" />
            <span>Detected: {new Date(finding.timeline.detected).toLocaleDateString()}</span>
          </div>
          <div className="flex items-center gap-2">
            <Calendar className="w-4 h-4 text-muted-foreground" />
            <span>Patched: {new Date(finding.timeline.patch_deployed).toLocaleDateString()}</span>
          </div>
          <div className="flex items-center gap-2">
            <Calendar className="w-4 h-4 text-muted-foreground" />
            <span>Disclosed: {new Date(finding.timeline.public_disclosure).toLocaleDateString()}</span>
          </div>
          <div className="text-muted-foreground mt-2">
            Total time to disclosure: <strong>{daysToDisclosure} days</strong>
          </div>
        </div>
      </div>

      <div className="border-t pt-4">
        <div className="text-sm font-semibold mb-2">Detection Details</div>
        <div className="space-y-2 text-sm mb-4">
          <div>
            <span className="text-muted-foreground">Category:</span>
            <span className="ml-2 font-mono text-xs bg-muted px-2 py-1 rounded">{finding.detection_category}</span>
          </div>
          <div>
            <span className="text-muted-foreground">Status:</span>
            <span className="ml-2 font-medium">{finding.status}</span>
          </div>
          <div>
            <span className="text-muted-foreground">Patch Summary:</span>
            <p className="mt-1">{finding.patch_summary}</p>
          </div>
        </div>

        {finding.references.length > 0 && (
          <div className="space-y-2">
            <div className="text-sm font-semibold">References</div>
            {finding.references.map((ref, idx) => (
              <Link
                key={idx}
                href={ref.url}
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex items-center gap-2 text-sm text-primary hover:underline"
              >
                {ref.title}
                <ExternalLink className="w-3 h-3" />
              </Link>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
