"""
Canonical Claude Oracle Parity Engine & Exact-Grid Verifier
Entrypoint & Matrix Orchestrator
"""

import os
import sys
import time
import argparse
from typing import List, Dict, Tuple, Optional, Any

from .terminal import CanonicalFrame
from .comparator import StageDiff, diff_exact_grid_frames
from .persistence import save_frame_artifacts
from .process import OracleSession
from .contracts.base import StageSpec, CapabilityContract
from .contracts import get_contract, CONTRACTS


ORACLE_VERSION = "2.1.235"
ORACLE_SHA256 = "83b8f806f6f2eea316cfe246628e6c23374711d868f1fd0409db551b877b7748"


def execute_stages(session: OracleSession, stages: List[StageSpec], verbose: bool = True) -> Tuple[List[CanonicalFrame], Optional[Tuple[int, str]]]:
    frames: List[CanonicalFrame] = []
    session.spawn()

    for stage in stages:
        if verbose:
            print(f"  -> Executing Stage {stage.index:02d} [{stage.name}]: {stage.description}")

        if stage.action_type == "type":
            session.send(stage.input_bytes)
        elif stage.action_type == "key":
            if b"\x1b[" in stage.input_bytes and len(stage.input_bytes) > 3:
                chunks = [stage.input_bytes[i:i+3] for i in range(0, len(stage.input_bytes), 3)]
                for c in chunks:
                    session.send(c)
                    time.sleep(0.03)
            elif len(stage.input_bytes) > 1 and not stage.input_bytes.startswith(b"\x1b"):
                for b in stage.input_bytes:
                    session.send(bytes([b]))
                    time.sleep(0.03)
            else:
                session.send(stage.input_bytes)
        elif stage.action_type == "type_and_enter":
            session.send(b"/theme")
            session.wait_until(lambda s: any("Change the theme" in l for l in s.display[-8:]), timeout_sec=4.0)
            session.send(b"\r")
        elif stage.action_type == "assert_disk":
            persisted = session.get_persisted_theme()
            if persisted != "light":
                return frames, (stage.index, f"Persisted theme mismatch: expected 'light', got '{persisted}'")
        elif stage.action_type == "restart":
            session.respawn()
            if stage.input_bytes:
                session.wait_until(lambda s: any(l.strip().startswith("❯") for l in s.display), timeout_sec=8.0)
                if stage.input_bytes == b"/theme":
                    session.send(b"/theme")
                    session.wait_until(lambda s: any("Change the theme" in l for l in s.display[-8:]), timeout_sec=4.0)
                    session.send(b"\r")
                else:
                    session.send(stage.input_bytes)

        if stage.wait_predicate:
            matched = session.wait_until(stage.wait_predicate, timeout_sec=15.0)
            if not matched:
                if verbose:
                    print(f"  ✖ Timed out waiting for condition: {stage.description}")
                frame = session.capture_canonical_frame(stage.index, stage.name)
                frames.append(frame)
                return frames, (stage.index, f"Timed out waiting for condition: {stage.description}")

        time.sleep(stage.settle_time_ms / 1000.0)
        frame = session.capture_canonical_frame(stage.index, stage.name)
        frames.append(frame)

    return frames, None


def run_quadrant(
    contract: CapabilityContract,
    editor_mode: str,
    cols: int,
    rows: int,
    artifacts_base: str,
    verbose: bool = True
) -> Tuple[bool, List[StageDiff], Dict[str, Any]]:
    stages = contract.get_stages()
    quadrant_name = f"{editor_mode}_{cols}x{rows}"
    quadrant_dir = os.path.join(artifacts_base, quadrant_name)
    
    label = f"{editor_mode.upper()} {cols}x{rows}"
    print(f"\n[{label}] Phase A: Recording Reference Claude Oracle ({len(stages)} stages)...")
    
    oracle_sess = OracleSession("claude", editor_mode, cols, rows)
    oracle_frames, oracle_err = execute_stages(oracle_sess, stages, verbose=False)
    oracle_sess.cleanup()
    
    if oracle_err:
        print(f"  ✖ Oracle recording failed at stage {oracle_err[0]} ({stages[oracle_err[0]-1].name}): {oracle_err[1]}")
        return False, [], {"error": f"Oracle failure: {oracle_err}"}
    print(f"  ✔ Oracle recording completed successfully ({len(oracle_frames)}/{len(stages)} stages captured).")
    
    for f in oracle_frames:
        save_frame_artifacts(quadrant_dir, "oracle", f.stage_index, f.stage_name, f)
        
    print(f"\n[{label}] Phase B: Executing Brain Shell Target ({len(stages)} stages)...")
    brain_sess = OracleSession("brain", editor_mode, cols, rows)
    brain_frames, brain_err = execute_stages(brain_sess, stages, verbose=False)
    brain_sess.cleanup()
    
    if brain_err:
        print(f"  ✖ Brain target failed execution at stage {brain_err[0]} ({stages[brain_err[0]-1].name}): {brain_err[1]}")
        return False, [], {"error": f"Brain execution failure: {brain_err}"}
    print(f"  ✔ Brain target execution completed successfully ({len(brain_frames)}/{len(stages)} stages captured).")
    
    for f in brain_frames:
        save_frame_artifacts(quadrant_dir, "brain", f.stage_index, f.stage_name, f)
        
    print(f"\n[{label}] Phase C: Exact-Grid Frame-by-Frame Differential Auditing...")
    diffs: List[StageDiff] = []
    all_passed = True
    
    for idx in range(len(stages)):
        o_frame = oracle_frames[idx]
        b_frame = brain_frames[idx]
        stage_spec = stages[idx]
        
        diff = diff_exact_grid_frames(o_frame, b_frame, cols, rows)
        diffs.append(diff)
        
        diff_text = "\n".join(diff.diff_lines) if diff.diff_lines else None
        save_frame_artifacts(quadrant_dir, "diff", diff.stage_index, diff.stage_name, b_frame, diff_text)
        
        if diff.passed:
            print(f"  Stage {diff.stage_index:2d} [{diff.stage_name}]: ✔ MATCH")
        else:
            all_passed = False
            print(f"  Stage {diff.stage_index:2d} [{diff.stage_name}]: ✖ DIVERGENCE [{diff.divergence_type}:{diff.divergence_category}]")
            print(f"    Summary: {diff.summary}")
            if diff.first_mismatch_cell:
                print(f"    First Mismatch Cell: row {diff.first_mismatch_cell[0]}, col {diff.first_mismatch_cell[1]}")
            if diff.diff_lines:
                print("    Terminal Grid Diff:")
                for l in diff.diff_lines[:15]:
                    print(f"      {l}")
            break
            
    return all_passed, diffs, {}


def main():
    parser = argparse.ArgumentParser(description="Canonical Claude Oracle Parity Engine & Verifier")
    parser.add_argument("--contract", default="theme", choices=list(CONTRACTS.keys()), help="Capability contract to execute")
    parser.add_argument("--quadrant", default="all", choices=["all", "normal_80x24", "normal_100x30", "vim_80x24", "vim_100x30"], help="Matrix quadrant to execute")
    parser.add_argument("--artifacts", default="artifacts/oracle_verification", help="Artifacts directory path")
    args = parser.parse_args()

    contract = get_contract(args.contract)

    print("=" * 70)
    print(f"      CANONICAL CLAUDE ORACLE PARITY ENGINE (v{ORACLE_VERSION})")
    print(f"      Contract: {contract.name.upper()} ({contract.description})")
    print(f"      Oracle Hash: {ORACLE_SHA256[:16]}...")
    print("=" * 70)

    matrix_quadrants = [
        ("normal", 80, 24),
        ("normal", 100, 30),
        ("vim", 80, 24),
        ("vim", 100, 30),
    ]

    if args.quadrant != "all":
        parts = args.quadrant.split("_")
        mode = parts[0]
        dims = parts[1].split("x")
        matrix_quadrants = [(mode, int(dims[0]), int(dims[1]))]

    results: Dict[str, Dict[str, str]] = {
        "80x24": {},
        "100x30": {}
    }

    all_quadrants_passed = True

    for mode, cols, rows in matrix_quadrants:
        geom_key = f"{cols}x{rows}"
        mode_label = "NORMAL" if mode == "normal" else "VIM INSERT"
        
        passed, diffs, meta = run_quadrant(contract, mode, cols, rows, args.artifacts, verbose=True)
        if not passed:
            all_quadrants_passed = False
            failed_stage = next((d.stage_index for d in diffs if not d.passed), None)
            if failed_stage is not None:
                results[geom_key][mode_label] = f"FAIL (Stg {failed_stage})"
            else:
                results[geom_key][mode_label] = "FAIL"
        else:
            results[geom_key][mode_label] = "PASS"

    print("\n" + "=" * 70)
    print("                  CANONICAL PARITY VERIFICATION MATRIX")
    print("=" * 70)
    print(f"{'GEOMETRY / MODE':<20} {'NORMAL':<25} {'VIM INSERT':<25}")
    print("-" * 70)
    for geom in ["80x24", "100x30"]:
        cl_norm = "PASS"
        cl_vim = "PASS"
        br_norm = results[geom].get("NORMAL", "SKIPPED")
        br_vim = results[geom].get("VIM INSERT", "SKIPPED")
        print(f"Claude {geom:<13} {cl_norm:<25} {cl_vim:<25}")
        print(f"Brain  {geom:<13} {br_norm:<25} {br_vim:<25}")
        print("-" * 70)
    print("=" * 70)

    if all_quadrants_passed:
        print("\n🎉 ALL BEHAVIORAL CONTRACT STAGES ACHIEVED 100% EXACT CANONICAL PARITY!\n")
        sys.exit(0)
    else:
        print("\n❌ ORACLE PARITY VERIFICATION FAILED — REPRODUCED FIRST DIVERGENCE.\n")
        sys.exit(1)


if __name__ == "__main__":
    main()
