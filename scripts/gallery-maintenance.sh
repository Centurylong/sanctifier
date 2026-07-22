#!/bin/bash
# Gallery maintenance script for Sanctifier
# This script helps validate and maintain the adopters and findings gallery data

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DATA_DIR="$PROJECT_ROOT/data"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo "🛡️  Sanctifier Gallery Maintenance Script"
echo "========================================"
echo ""

# Function to validate JSON
validate_json() {
    local file=$1
    if ! jq empty "$file" 2>/dev/null; then
        echo -e "${RED}❌ Invalid JSON in $file${NC}"
        return 1
    fi
    return 0
}

# Function to check adopters data
check_adopters() {
    echo "📋 Checking adopters data..."
    
    if ! validate_json "$DATA_DIR/adopters.json"; then
        return 1
    fi
    
    local total_adopters=$(jq '.adopters | length' "$DATA_DIR/adopters.json")
    local verified=$(jq '[.adopters[] | select(.verified == true)] | length' "$DATA_DIR/adopters.json")
    local findings_total=$(jq '[.adopters[].findings_count] | add' "$DATA_DIR/adopters.json")
    
    echo -e "${GREEN}✅ Adopters data valid${NC}"
    echo "   Total adopters: $total_adopters"
    echo "   Verified adopters: $verified"
    echo "   Total findings surfaced: $findings_total"
    echo ""
}

# Function to check findings data
check_findings() {
    echo "📊 Checking findings data..."
    
    if ! validate_json "$DATA_DIR/findings-showcase.json"; then
        return 1
    fi
    
    local total_findings=$(jq '.featured_findings | length' "$DATA_DIR/findings-showcase.json")
    local critical=$(jq '[.featured_findings[] | select(.severity == "critical")] | length' "$DATA_DIR/findings-showcase.json")
    local high=$(jq '[.featured_findings[] | select(.severity == "high")] | length' "$DATA_DIR/findings-showcase.json")
    local medium=$(jq '[.featured_findings[] | select(.severity == "medium")] | length' "$DATA_DIR/findings-showcase.json")
    
    echo -e "${GREEN}✅ Findings data valid${NC}"
    echo "   Total findings: $total_findings"
    echo "   Critical: $critical | High: $high | Medium: $medium"
    echo ""
}

# Function to validate repository URLs
validate_repos() {
    echo "🔗 Validating repository URLs..."
    
    local invalid_count=0
    local repos=$(jq -r '.adopters[].repository' "$DATA_DIR/adopters.json")
    
    while IFS= read -r repo; do
        if [[ ! "$repo" =~ ^https?:// ]]; then
            echo -e "${RED}❌ Invalid URL: $repo${NC}"
            ((invalid_count++))
        fi
    done <<< "$repos"
    
    if [ $invalid_count -eq 0 ]; then
        echo -e "${GREEN}✅ All repository URLs are valid${NC}"
    else
        echo -e "${RED}❌ Found $invalid_count invalid URLs${NC}"
        return 1
    fi
    echo ""
}

# Function to update statistics
update_statistics() {
    echo "📈 Updating statistics..."
    
    local adopters_file="$DATA_DIR/adopters.json"
    local findings_file="$DATA_DIR/findings-showcase.json"
    
    # Update adopters stats
    jq '.statistics.last_updated = now | strftime("%Y-%m-%d")' "$adopters_file" > "$adopters_file.tmp"
    mv "$adopters_file.tmp" "$adopters_file"
    
    # Update findings stats  
    jq '.statistics.last_updated = now | strftime("%Y-%m-%d")' "$findings_file" > "$findings_file.tmp"
    mv "$findings_file.tmp" "$findings_file"
    
    echo -e "${GREEN}✅ Statistics updated${NC}"
    echo ""
}

# Function to list recent additions
list_recent() {
    echo "📅 Recent additions (last 30 days):"
    
    local recent=$(jq -r '.adopters[] | select(.date_added >= (now | strftime("%Y-%m-%d") | fromdateiso8601 - (30*24*3600) | strftime("%Y-%m-%d"))) | "\(.name) - Added \(.date_added)"' "$DATA_DIR/adopters.json")
    
    if [ -z "$recent" ]; then
        echo "   No recent additions"
    else
        echo "$recent" | while read -r line; do
            echo "   • $line"
        done
    fi
    echo ""
}

# Function to generate report
generate_report() {
    echo "📄 Generating gallery report..."
    
    local report_file="$DATA_DIR/reports/gallery-$(date +%Y-%m-%d).json"
    mkdir -p "$DATA_DIR/reports"
    
    jq -n \
        --slurpfile adopters "$DATA_DIR/adopters.json" \
        --slurpfile findings "$DATA_DIR/findings-showcase.json" \
        '{
            generated_at: now | strftime("%Y-%m-%dT%H:%M:%SZ"),
            adopters_summary: $adopters[0].statistics,
            findings_summary: $findings[0].statistics,
            adopters: $adopters[0].adopters,
            findings: $findings[0].featured_findings
        }' > "$report_file"
    
    echo -e "${GREEN}✅ Report generated${NC}"
    echo "   Location: $report_file"
    echo ""
}

# Main execution
main() {
    if [ ! -d "$DATA_DIR" ]; then
        echo -e "${RED}❌ Data directory not found: $DATA_DIR${NC}"
        exit 1
    fi
    
    check_adopters || exit 1
    check_findings || exit 1
    validate_repos || exit 1
    list_recent
    
    # Optional: update statistics and generate report
    if [ "$1" = "--update" ]; then
        update_statistics
        generate_report
    fi
    
    echo -e "${GREEN}✅ Gallery data validation complete!${NC}"
}

# Run main
main "$@"
