#!/usr/bin/env python3
"""
HTML & Markdown Inspection Report Generator
Generates report.html and CONTACT_SHEETS.md allowing visual evidence audit per session.
"""

import os
import json
from pathlib import Path


class ContactSheetGenerator:
    """Generates visual contact sheets and HTML inspection reports."""

    def __init__(self, run_dir: Path):
        self.run_dir = run_dir

    def generate_html_report(self, manifest_data: dict) -> Path:
        sessions = manifest_data.get("sessions", [])
        
        html_lines = [
            "<!DOCTYPE html>",
            "<html>",
            "<head>",
            "<title>Claude UX Empirical Audit Report</title>",
            "<style>",
            "body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 20px; background: #1e1e1e; color: #d4d4d4; }",
            "h1, h2 { color: #d77757; }",
            ".summary-card { background: #252526; border: 1px solid #3c3c3c; border-radius: 6px; padding: 15px; margin-bottom: 20px; }",
            ".session-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(380px, 1fr)); gap: 15px; }",
            ".session-card { background: #2d2d2d; border: 1px solid #454545; border-radius: 6px; padding: 12px; }",
            ".badge-pass { background: #2e6b34; color: #fff; padding: 2px 8px; border-radius: 4px; font-weight: bold; }",
            ".badge-invalid { background: #8b0000; color: #fff; padding: 2px 8px; border-radius: 4px; font-weight: bold; }",
            "img { max-width: 100%; border: 1px solid #555; border-radius: 4px; margin-top: 8px; }",
            "pre { background: #1e1e1e; padding: 8px; border-radius: 4px; font-size: 11px; overflow-x: auto; }",
            "</style>",
            "</head>",
            "<body>",
            "<h1>Claude UX Empirical Audit Report</h1>",
            f"<div class='summary-card'>",
            f"<h3>Execution Timestamp: {manifest_data.get('timestamp')}</h3>",
            f"<p>Total Sessions: <strong>{manifest_data.get('total_executed')}</strong> | PASS: <span class='badge-pass'>{manifest_data.get('summary', {}).get('pass')}</span> | INVALID: <span class='badge-invalid'>{manifest_data.get('summary', {}).get('invalid')}</span></p>",
            "</div>",
            "<div class='session-grid'>"
        ]

        for s in sessions:
            sess_id = s.get("session_id", "unknown")
            status = s.get("status", "UNKNOWN")
            vp = s.get("viewport", "")
            sc = s.get("scenario", "")
            
            shot_rel = f"sessions/{sess_id}/screenshot.png"
            shot_file = self.run_dir / shot_rel
            img_tag = f"<img src='{shot_rel}' alt='{sess_id}'>" if shot_file.exists() else "<p><em>No screenshot</em></p>"

            badge_class = "badge-pass" if status == "PASS" else "badge-invalid"

            html_lines.append(f"""
            <div class='session-card'>
                <h4>Scenario: {sc} ({vp}) <span class='{badge_class}'>{status}</span></h4>
                <p><strong>Session ID:</strong> <code>{sess_id}</code></p>
                {img_tag}
                <details>
                    <summary>OCR Validation & Trace</summary>
                    <pre>{json.dumps(s.get('ocr_validation', {}), indent=2)}</pre>
                </details>
            </div>
            """)

        html_lines.extend(["</div>", "</body>", "</html>"])
        
        report_path = self.run_dir / "report.html"
        with open(report_path, "w") as f:
            f.write("\n".join(html_lines))

        return report_path

    def generate_markdown(self, manifest_data: dict) -> Path:
        md_lines = ["# Claude UX Contact Sheets & Visual Comparison\n"]
        md_lines.append(f"> Timestamp: {manifest_data.get('timestamp')} · Total Sessions: {manifest_data.get('total_executed')}\n")

        for s in manifest_data.get("sessions", []):
            sess_id = s.get("session_id")
            sc = s.get("scenario")
            vp = s.get("viewport")
            status = s.get("status")
            img_rel = f"sessions/{sess_id}/screenshot.png"
            
            md_lines.append(f"### Session `{sess_id}` ({sc} @ {vp}) — **{status}**")
            md_lines.append(f"![{sess_id}]({img_rel})\n")

        out_path = self.run_dir / "CONTACT_SHEETS.md"
        with open(out_path, "w") as f:
            f.write("\n".join(md_lines))

        return out_path
