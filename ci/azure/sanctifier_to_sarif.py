#!/usr/bin/env python3
"""Convert Sanctifier's existing analyze --format json output into SARIF 2.1.0.

Uses only Python's standard library. Unknown source locations are omitted, never
fabricated; the input scan's exit status remains the CI step's responsibility.
"""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
from urllib.parse import quote

SARIF_SCHEMA = "https://json.schemastore.org/sarif-2.1.0.json"
LOCATION_RE = re.compile(r"^(?P<file>.+?\.rs):(?P<line>[1-9]\d*)(?::(?P<column>[1-9]\d*))?(?::.*)?$")
LEVELS = {
    "critical": "error",
    "high": "error",
    "error": "error",
    "medium": "warning",
    "moderate": "warning",
    "warning": "warning",
    "low": "note",
    "info": "note",
    "informational": "note",
}


def _description(finding: dict, category: str) -> str:
    for name in (
        "message", "description", "issue_type", "function_name", "function",
        "rule_name", "pattern_type", "struct_name", "event_name", "snippet",
        "suggestion", "call_expression", "vulnerability_id", "vuln_id",
    ):
        value = finding.get(name)
        if isinstance(value, str) and value.strip():
            return value.strip()
    return f"Sanctifier finding: {category.replace('_', ' ')}"


def _physical_location(finding: dict) -> dict | None:
    """Do not assign guessed file/line numbers to field-only findings."""
    raw = finding.get("location")
    file_name, line, column = None, None, None
    if isinstance(raw, dict):
        file_name = raw.get("file") or raw.get("path")
        line = raw.get("line")
        column = raw.get("column")
    elif isinstance(raw, str):
        matched = LOCATION_RE.match(raw.strip())
        if matched:
            file_name = matched.group("file")
            line = int(matched.group("line"))
            column = int(matched.group("column")) if matched.group("column") else None

    if not isinstance(file_name, str):
        return None
    file_name = file_name.replace("\\", "/")
    while file_name.startswith("./"):
        file_name = file_name[2:]
    parts = file_name.split("/")
    if not file_name or file_name.startswith("/") or ":" in parts[0] or any(
        part in ("", ".", "..") for part in parts
    ):
        return None
    if isinstance(line, bool) or not isinstance(line, int) or line < 1:
        return None

    physical = {
        "artifactLocation": {"uri": quote(file_name, safe="/-._~")},
        "region": {"startLine": line},
    }
    if isinstance(column, int) and not isinstance(column, bool) and column > 0:
        physical["region"]["startColumn"] = column
    return {"physicalLocation": physical}


def convert(report: dict) -> dict:
    if not isinstance(report, dict) or report.get("success") is False:
        raise ValueError("input is not a successful Sanctifier JSON findings report")
    categories = report.get("findings")
    if not isinstance(categories, dict):
        raise ValueError("missing findings object; run sanctifier analyze --format json")

    results = []
    rules: dict[str, dict] = {}
    groups = dict(categories)
    # VulnDB findings currently live alongside, not inside, the findings map.
    if "vulnerability_db_matches" not in groups:
        groups["vulnerability_db_matches"] = report.get("vulnerability_db_matches", [])

    for category, entries in sorted(groups.items()):
        if not isinstance(entries, list):
            raise ValueError(f"findings.{category} must be an array")
        for item in entries:
            if not isinstance(item, dict):
                raise ValueError(f"findings.{category} must contain objects")
            fallback = "SANCTIFIER_" + re.sub(r"[^A-Z0-9]+", "_", category.upper()).strip("_")
            code = str(item.get("code") or fallback)
            severity = str(item.get("severity") or item.get("level") or "").lower()
            sarif_level = LEVELS.get(severity, "warning")
            message = _description(item, category)
            rules.setdefault(code, {
                "id": code,
                "shortDescription": {"text": category.replace("_", " ").capitalize()},
            })
            result = {
                "ruleId": code,
                "level": sarif_level,
                "message": {"text": message},
                "properties": {"category": category},
            }
            location = _physical_location(item)
            if location:
                result["locations"] = [location]
            results.append(result)

    return {
        "$schema": SARIF_SCHEMA,
        "version": "2.1.0",
        "runs": [{
            "tool": {"driver": {
                "name": "Sanctifier",
                "informationUri": "https://github.com/Centurylong/sanctifier",
                "rules": [rules[key] for key in sorted(rules)],
            }},
            "results": results,
        }],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path, help="Sanctifier analyze --format json file")
    parser.add_argument("output", type=Path, help="SARIF 2.1.0 destination")
    args = parser.parse_args()
    try:
        report = json.loads(args.input.read_text(encoding="utf-8"))
        sarif = convert(report)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        parser.error(str(exc))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(sarif, indent=2) + "\n", encoding="utf-8")
    print(f"SARIF: {len(sarif['runs'][0]['results'])} findings -> {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
