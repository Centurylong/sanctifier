// Utility functions to load gallery data
import adoptorsData from '../../data/adopters.json';
import findingsData from '../../data/findings-showcase.json';

export type Adopter = {
  id: string;
  name: string;
  repository: string;
  description: string;
  category: string;
  findings_count: number;
  vulnerabilities_found: string[];
  date_added: string;
  logo_url: string;
  verified: boolean;
  notes?: string;
};

export type Finding = {
  id: string;
  vulnerability_id: string;
  title: string;
  severity: 'critical' | 'high' | 'medium' | 'low';
  cvss: number;
  project: string;
  project_name: string;
  detected_by: string;
  detection_date: string;
  disclosure_date: string;
  status: string;
  description: string;
  impact: string;
  finding_code: string;
  detection_category: string;
  patch_summary: string;
  timeline: {
    detected: string;
    reported_to_team: string;
    team_acknowledged: string;
    patch_deployed: string;
    public_disclosure: string;
  };
  references: Array<{
    title: string;
    url: string;
  }>;
};

export function getAllAdopters(): Adopter[] {
  return adoptorsData.adopters;
}

export function getAdopterById(id: string): Adopter | undefined {
  return adoptorsData.adopters.find(a => a.id === id);
}

export function getAdoptersByCategory(category: string): Adopter[] {
  return adoptorsData.adopters.filter(a => a.category === category);
}

export function getVerifiedAdopters(): Adopter[] {
  return adoptorsData.adopters.filter(a => a.verified);
}

export function getAllFindings(): Finding[] {
  return findingsData.featured_findings as Finding[];
}

export function getFindingById(id: string): Finding | undefined {
  return findingsData.featured_findings.find(f => f.id === id) as Finding | undefined;
}

export function getFindingsBySeverity(severity: string): Finding[] {
  return findingsData.featured_findings.filter(f => f.severity === severity) as Finding[];
}

export function getFindingsByProject(projectId: string): Finding[] {
  return findingsData.featured_findings.filter(f => f.project === projectId) as Finding[];
}

export function getGalleryStatistics() {
  return {
    adopters: adoptorsData.statistics,
    findings: findingsData.statistics,
  };
}

export function getCategoryStats(): Record<string, number> {
  const stats: Record<string, number> = {};
  adoptorsData.adopters.forEach(adopter => {
    stats[adopter.category] = (stats[adopter.category] || 0) + 1;
  });
  return stats;
}

export function getSeverityStats(): Record<string, number> {
  const stats: Record<'critical' | 'high' | 'medium' | 'low', number> = {
    critical: 0,
    high: 0,
    medium: 0,
    low: 0,
  };
  (findingsData.featured_findings as Finding[]).forEach(finding => {
    const severity = finding.severity as 'critical' | 'high' | 'medium' | 'low';
    stats[severity]++;
  });
  return stats;
}
