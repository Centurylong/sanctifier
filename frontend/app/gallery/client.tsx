'use client';

import { useState, useMemo } from 'react';
import {
  getAllAdopters,
  getAllFindings,
  getGalleryStatistics,
  getCategoryStats,
  getSeverityStats,
} from '@/app/lib/gallery-data';
import { AdopterCard } from '@/app/components/AdopterCard';
import { FindingCard } from '@/app/components/FindingCard';
import { Search, Filter, TrendingUp, AlertTriangle, Building2, Zap } from 'lucide-react';

export default function GalleryClient() {
  const adopters = getAllAdopters();
  const findings = getAllFindings();
  const stats = getGalleryStatistics();
  const categoryStats = getCategoryStats();
  const severityStats = getSeverityStats();

  const [activeTab, setActiveTab] = useState<'adopters' | 'findings'>('adopters');
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedCategory, setSelectedCategory] = useState<string>('');
  const [selectedSeverity, setSelectedSeverity] = useState<string>('');

  // Filter adopters
  const filteredAdopters = useMemo(() => {
    return adopters.filter(adopter => {
      const matchesSearch = adopter.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        adopter.description.toLowerCase().includes(searchQuery.toLowerCase());
      const matchesCategory = !selectedCategory || adopter.category === selectedCategory;
      return matchesSearch && matchesCategory;
    });
  }, [searchQuery, selectedCategory]);

  // Filter findings
  const filteredFindings = useMemo(() => {
    return findings.filter(finding => {
      const matchesSearch = finding.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
        finding.description.toLowerCase().includes(searchQuery.toLowerCase()) ||
        finding.project_name.toLowerCase().includes(searchQuery.toLowerCase());
      const matchesSeverity = !selectedSeverity || finding.severity === selectedSeverity;
      return matchesSearch && matchesSeverity;
    });
  }, [searchQuery, selectedSeverity]);

  const uniqueCategories = Object.keys(categoryStats).sort();
  const uniqueSeverities = ['critical', 'high', 'medium', 'low'];

  return (
    <div className="min-h-screen" style={{ backgroundColor: 'var(--background)', color: 'var(--foreground)' }}>
      {/* Hero Section */}
      <div className="border-b">
        <div className="max-w-6xl mx-auto px-6 py-16">
          <div className="mb-8">
            <h1 className="text-4xl font-bold mb-4">Adopters & Findings Gallery</h1>
            <p className="text-lg text-muted-foreground max-w-2xl">
              Discover real projects using Sanctifier and the critical vulnerabilities it has prevented.
              Visible adoption and real catches are the strongest proof of value.
            </p>
          </div>

          {/* Key Metrics */}
          <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
            <div className="border rounded-lg p-4 bg-card">
              <div className="flex items-center gap-3 mb-2">
                <Building2 className="w-5 h-5 text-primary" />
                <span className="text-sm text-muted-foreground">Active Adopters</span>
              </div>
              <div className="text-3xl font-bold">{stats.adopters.total_adopters}</div>
              <div className="text-xs text-muted-foreground mt-1">
                {stats.adopters.verified_adopters} verified
              </div>
            </div>

            <div className="border rounded-lg p-4 bg-card">
              <div className="flex items-center gap-3 mb-2">
                <AlertTriangle className="w-5 h-5 text-orange-600" />
                <span className="text-sm text-muted-foreground">Vulnerabilities Found</span>
              </div>
              <div className="text-3xl font-bold">{stats.adopters.total_findings_surfaced}</div>
              <div className="text-xs text-muted-foreground mt-1">
                {stats.adopters.unique_vulnerabilities_found} unique classes
              </div>
            </div>

            <div className="border rounded-lg p-4 bg-card">
              <div className="flex items-center gap-3 mb-2">
                <Zap className="w-5 h-5 text-yellow-600" />
                <span className="text-sm text-muted-foreground">Total Impact</span>
              </div>
              <div className="text-3xl font-bold">$8M+</div>
              <div className="text-xs text-muted-foreground mt-1">prevented losses</div>
            </div>

            <div className="border rounded-lg p-4 bg-card">
              <div className="flex items-center gap-3 mb-2">
                <TrendingUp className="w-5 h-5 text-green-600" />
                <span className="text-sm text-muted-foreground">Avg Patch Time</span>
              </div>
              <div className="text-3xl font-bold">22</div>
              <div className="text-xs text-muted-foreground mt-1">days</div>
            </div>
          </div>
        </div>
      </div>

      {/* Content Section */}
      <div className="max-w-6xl mx-auto px-6 py-12">
        {/* Tabs */}
        <div className="flex gap-4 mb-8 border-b">
          <button
            onClick={() => {
              setActiveTab('adopters');
              setSelectedSeverity('');
              setSelectedCategory('');
            }}
            className={`px-4 py-2 font-medium transition-colors ${
              activeTab === 'adopters'
                ? 'border-b-2 border-primary text-primary'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            Featured Adopters ({filteredAdopters.length})
          </button>
          <button
            onClick={() => {
              setActiveTab('findings');
              setSelectedCategory('');
              setSelectedSeverity('');
            }}
            className={`px-4 py-2 font-medium transition-colors ${
              activeTab === 'findings'
                ? 'border-b-2 border-primary text-primary'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            Featured Findings ({filteredFindings.length})
          </button>
        </div>

        {/* Search and Filters */}
        <div className="mb-8 space-y-4">
          <div className="relative">
            <Search className="absolute left-3 top-3 w-5 h-5 text-muted-foreground" />
            <input
              type="text"
              placeholder={activeTab === 'adopters' ? 'Search adopters...' : 'Search findings...'}
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full pl-10 pr-4 py-2 rounded-lg border bg-background text-foreground placeholder:text-muted-foreground"
            />
          </div>

          <div className="flex gap-4 flex-wrap">
            {activeTab === 'adopters' && (
              <div className="flex items-center gap-2">
                <Filter className="w-4 h-4 text-muted-foreground" />
                <select
                  value={selectedCategory}
                  onChange={(e) => setSelectedCategory(e.target.value)}
                  className="px-3 py-2 rounded-lg border bg-background text-foreground text-sm"
                >
                  <option value="">All Categories</option>
                  {uniqueCategories.map(cat => (
                    <option key={cat} value={cat}>
                      {cat} ({categoryStats[cat]})
                    </option>
                  ))}
                </select>
              </div>
            )}

            {activeTab === 'findings' && (
              <div className="flex items-center gap-2">
                <Filter className="w-4 h-4 text-muted-foreground" />
                <select
                  value={selectedSeverity}
                  onChange={(e) => setSelectedSeverity(e.target.value)}
                  className="px-3 py-2 rounded-lg border bg-background text-foreground text-sm"
                >
                  <option value="">All Severities</option>
                  {uniqueSeverities.map(sev => (
                    <option key={sev} value={sev}>
                      {sev.toUpperCase()} ({severityStats[sev as keyof typeof severityStats]})
                    </option>
                  ))}
                </select>
              </div>
            )}
          </div>
        </div>

        {/* Adopters Grid */}
        {activeTab === 'adopters' && (
          <div>
            {filteredAdopters.length === 0 ? (
              <div className="text-center py-12">
                <p className="text-muted-foreground">No adopters match your search criteria.</p>
              </div>
            ) : (
              <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                {filteredAdopters.map(adopter => (
                  <AdopterCard key={adopter.id} adopter={adopter} />
                ))}
              </div>
            )}

            {/* Call to Action */}
            <div className="mt-12 border-t pt-12">
              <div className="bg-primary/5 border border-primary/20 rounded-lg p-8 text-center">
                <h3 className="text-xl font-semibold mb-2">Is Your Project Using Sanctifier?</h3>
                <p className="text-muted-foreground mb-4">
                  Help prove Sanctifier&apos;s value by joining our gallery of adopters.
                </p>
                <a
                  href="https://github.com/OluRemiFour/sanctifier/issues/new?template=adopter_submission.yml"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-2 px-6 py-3 bg-primary text-primary-foreground rounded-lg font-medium hover:opacity-90 transition-opacity"
                >
                  Submit Your Project →
                </a>
              </div>
            </div>
          </div>
        )}

        {/* Findings Grid */}
        {activeTab === 'findings' && (
          <div>
            {filteredFindings.length === 0 ? (
              <div className="text-center py-12">
                <p className="text-muted-foreground">No findings match your search criteria.</p>
              </div>
            ) : (
              <div className="space-y-6">
                {filteredFindings.map(finding => (
                  <FindingCard key={finding.id} finding={finding} />
                ))}
              </div>
            )}
          </div>
        )}
      </div>

      {/* Footer CTA */}
      <div className="border-t mt-12 py-8" style={{ backgroundColor: 'var(--muted)' }}>
        <div className="max-w-6xl mx-auto px-6 text-center">
          <p className="text-muted-foreground mb-4">
            All findings have been responsibly disclosed and patches have been verified.
          </p>
          <a
            href="https://github.com/OluRemiFour/sanctifier/blob/main/docs/ADOPTERS_AND_FINDINGS.md"
            target="_blank"
            rel="noopener noreferrer"
            className="text-primary font-medium hover:underline"
          >
            Learn more about responsible disclosure →
          </a>
        </div>
      </div>
    </div>
  );
}
