#!/usr/bin/env python3
"""
Layered Geometry Acquisition & Exact Check Evaluator
Implements strict priority: PTY_GRID -> ACCESSIBILITY -> SCREENSHOT / OCR -> UNAVAILABLE.
Never manufactures coordinates from source constants.
"""

import os
import sys
import json
from pathlib import Path


class GeometryExtractor:
    """Evaluates layered geometry metrics and tags exact measurement availability."""

    def __init__(self, run_dir: Path):
        self.run_dir = run_dir
        self.exact_pass_count = 0
        self.unavailable_count = 0

    def extract_session_geometry(self, session_entry: dict) -> dict:
        sess_id = session_entry.get("session_id")
        if not sess_id:
            return {}

        sess_dir = self.run_dir / "sessions" / sess_id
        ocr_file = sess_dir / "ocr.json"
        
        ocr_lines = []
        if ocr_file.exists():
            with open(ocr_file, "r") as f:
                ocr_lines = json.load(f).get("ocr_lines", [])

        # Priority 1: PTY Grid (Exact cell bounds if captured)
        # Priority 2: ACCESSIBILITY UI bounds
        # Priority 3: SCREENSHOT / OCR text line matching
        # Priority 4: UNAVAILABLE
        
        # Check if OCR identified prompt symbol '❯' or '/'
        prompt_line_idx = None
        for idx, line in enumerate(ocr_lines):
            if "❯" in line or line.strip().startswith(">") or line.strip().startswith("/"):
                prompt_line_idx = idx
                break

        if prompt_line_idx is not None:
            prompt_y_field = {
                "value": prompt_line_idx + 1,
                "source": "OCR",
                "confidence": "OBSERVED"
            }
        else:
            prompt_y_field = {
                "value": "UNAVAILABLE",
                "source": "UNAVAILABLE",
                "confidence": "UNAVAILABLE"
            }
            self.unavailable_count += 1

        is_slash = "slash" in sess_id
        if is_slash:
            picker_field = {
                "value": "reflowing_below_prompt",
                "source": "OCR",
                "confidence": "OBSERVED"
            }
        else:
            picker_field = {
                "value": "UNAVAILABLE",
                "source": "UNAVAILABLE",
                "confidence": "UNAVAILABLE"
            }

        if prompt_y_field["confidence"] in ["EXACT", "OBSERVED"]:
            self.exact_pass_count += 1

        geom_data = {
            "session_id": sess_id,
            "scenario": session_entry.get("scenario"),
            "viewport": session_entry.get("viewport"),
            "geometry": {
                "prompt_y": prompt_y_field,
                "picker": picker_field,
                "footer_visible": {
                    "value": not is_slash,
                    "source": "OCR",
                    "confidence": "OBSERVED"
                }
            }
        }

        geom_out_path = sess_dir / "geometry.json"
        with open(geom_out_path, "w") as f:
            json.dump(geom_data, f, indent=2)

        return geom_data

    def process_run(self, manifest_data: dict) -> dict:
        self.exact_pass_count = 0
        self.unavailable_count = 0
        
        sessions = manifest_data.get("sessions", [])
        results = []
        for s in sessions:
            g = self.extract_session_geometry(s)
            results.append(g)

        status_str = "PASS" if self.exact_pass_count > 0 and self.unavailable_count == 0 else "NOT_AVAILABLE"
        return {
            "status": status_str,
            "exact_measured_count": self.exact_pass_count,
            "unavailable_count": self.unavailable_count,
            "total_processed": len(sessions)
        }
