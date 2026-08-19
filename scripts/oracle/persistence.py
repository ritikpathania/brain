"""
Artifact Persistence & File-Level Assertions
"""

import os
import json
from typing import Optional
from .terminal import CanonicalFrame


def save_frame_artifacts(
    base_dir: str,
    target: str,
    stage_idx: int,
    stage_name: str,
    frame: CanonicalFrame,
    diff_text: Optional[str] = None
):
    """
    Saves:
      1. .raw bytes for bit-level stream replay
      2. .txt representation of screen lines
      3. .json structured representation with full cell attribute matrices
      4. .diff if target diverged from oracle
    """
    target_dir = os.path.join(base_dir, target)
    os.makedirs(target_dir, exist_ok=True)
    
    # 1. Raw PTY stream (.raw)
    raw_path = os.path.join(target_dir, f"stage_{stage_idx:02d}_{stage_name}.raw")
    with open(raw_path, "wb") as f:
        f.write(frame.raw_pty_bytes)
        
    # 2. Text display (.txt)
    txt_path = os.path.join(target_dir, f"stage_{stage_idx:02d}_{stage_name}.txt")
    with open(txt_path, "w", encoding="utf-8") as f:
        for line in frame.screen_lines:
            f.write(line + "\n")
            
    # 3. Machine-readable JSON (.json) with exact cell grid
    json_path = os.path.join(target_dir, f"stage_{stage_idx:02d}_{stage_name}.json")
    with open(json_path, "w", encoding="utf-8") as f:
        json.dump(frame.to_dict(), f, indent=2)
        
    # 4. Unified Diff (.diff) if divergence occurred
    if diff_text:
        diff_dir = os.path.join(base_dir, "diff")
        os.makedirs(diff_dir, exist_ok=True)
        diff_path = os.path.join(diff_dir, f"stage_{stage_idx:02d}_{stage_name}.diff")
        with open(diff_path, "w", encoding="utf-8") as f:
            f.write(diff_text)
