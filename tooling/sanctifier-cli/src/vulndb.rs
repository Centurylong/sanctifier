use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VulnEntry {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    pub name: String,
    pub description: String,
    pub cvss: Option<f64>,
    pub severity: String,
    pub category: String,
    pub affected_versions: Option<String>,
    pub pattern: String,
    pub poc_exploit: Option<String>,
    pub patch: Option<String>,
    pub recommendation: String,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub related_cves: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VulnDatabase {
    pub version: String,
    pub last_updated: String,
    pub description: String,
    pub vulnerabilities: Vec<VulnEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VulnMatch {
    pub vuln_id: String,
    pub name: String,
    pub severity: String,
    pub category: String,
    pub description: String,
    pub recommendation: String,
    pub file: String,
    pub line: usize,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RssItem {
    pub title: String,
    pub link: String,
    pub description: String,
    pub pub_date: String,
    pub guid: String,
}

impl VulnDatabase {
    /// Load the vulnerability database from a JSON file.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let db: VulnDatabase = serde_json::from_str(&content)?;
        Ok(db)
    }

    /// Load the embedded default vulnerability database.
    pub fn load_default() -> Self {
        let content = include_str!("../../../data/vulnerability-db.json");
        serde_json::from_str(content).expect("embedded vulnerability-db.json is valid")
    }

    /// Scan source code against all vulnerability patterns.
    pub fn scan(&self, source: &str, file_name: &str) -> Vec<VulnMatch> {
        let mut matches = Vec::new();

        for vuln in &self.vulnerabilities {
            let re = match Regex::new(&vuln.pattern) {
                Ok(r) => r,
                Err(_) => continue,
            };

            for mat in re.find_iter(source) {
                let line = source[..mat.start()].matches('\n').count() + 1;
                let line_start = source[..mat.start()]
                    .rfind('\n')
                    .map(|p| p + 1)
                    .unwrap_or(0);
                let line_end = source[mat.end()..]
                    .find('\n')
                    .map(|p| mat.end() + p)
                    .unwrap_or(source.len());
                let snippet = source[line_start..line_end].trim().to_string();

                matches.push(VulnMatch {
                    vuln_id: vuln.id.clone(),
                    name: vuln.name.clone(),
                    severity: vuln.severity.clone(),
                    category: vuln.category.clone(),
                    description: vuln.description.clone(),
                    recommendation: vuln.recommendation.clone(),
                    file: file_name.to_string(),
                    line,
                    snippet,
                });
            }
        }

        matches
    }

    /// Search vulnerabilities by keyword (searches id, name, description, tags).
    pub fn search(&self, keyword: &str) -> Vec<&VulnEntry> {
        let kw = keyword.to_lowercase();
        self.vulnerabilities
            .iter()
            .filter(|v| {
                v.id.to_lowercase().contains(&kw)
                    || v.name.to_lowercase().contains(&kw)
                    || v.description.to_lowercase().contains(&kw)
                    || v.tags.iter().any(|t| t.to_lowercase().contains(&kw))
                    || v.category.to_lowercase().contains(&kw)
            })
            .collect()
    }

    /// Filter vulnerabilities by category.
    pub fn by_category<'a>(&'a self, category: &str) -> Vec<&'a VulnEntry> {
        let cat = category.to_lowercase();
        self.vulnerabilities
            .iter()
            .filter(|v| v.category.to_lowercase() == cat)
            .collect()
    }

    /// Filter vulnerabilities by severity.
    pub fn by_severity<'a>(&'a self, severity: &str) -> Vec<&'a VulnEntry> {
        let sev = severity.to_lowercase();
        self.vulnerabilities
            .iter()
            .filter(|v| v.severity.to_lowercase() == sev)
            .collect()
    }

    /// Get a single vulnerability by ID.
    pub fn get_by_id(&self, id: &str) -> Option<&VulnEntry> {
        let id_upper = id.to_uppercase();
        self.vulnerabilities.iter().find(|v| v.id.to_uppercase() == id_upper)
    }

    /// Serialize the full database to JSON.
    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Generate an RSS 2.0 feed of all vulnerabilities.
    pub fn to_rss(&self, base_url: &str) -> String {
        let base = base_url.trim_end_matches('/');
        let items: String = self
            .vulnerabilities
            .iter()
            .map(|v| {
                let title = format!("[{}] {}", v.severity.to_uppercase(), v.name);
                let link = format!("{}/vulndb/{}", base, v.id);
                let desc = xml_escape(&v.description);
                let cvss_str = v
                    .cvss
                    .map(|c| format!(" | CVSS: {:.1}", c))
                    .unwrap_or_default();
                format!(
                    "    <item>\n\
                           <title>{title}</title>\n\
                           <link>{link}</link>\n\
                           <description>{desc}{cvss_str}</description>\n\
                           <guid>{link}</guid>\n\
                           <pubDate>{}</pubDate>\n\
                         </item>",
                    self.last_updated
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <rss version=\"2.0\">\n\
               <channel>\n\
                 <title>Sanctifier Soroban Vulnerability Database</title>\n\
                 <link>{base}/vulndb</link>\n\
                 <description>{}</description>\n\
                 <lastBuildDate>{}</lastBuildDate>\n\
                 <language>en-us</language>\n\
             {items}\n\
               </channel>\n\
             </rss>",
            xml_escape(&self.description),
            self.last_updated,
        )
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
