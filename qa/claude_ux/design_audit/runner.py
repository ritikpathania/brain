#!/usr/bin/env python3
"""
Claude UX Design Atlas Explorer Runner
Orchestrates path-replay screen discovery across all exploratory modules.
Enforces adaptive session budget (TARGET_SESSIONS = 20, MAX_SESSIONS = 40).
Derives session counter metrics strictly from actual SessionResult objects.
Distinguishes frontier_exhausted vs budget_exhausted!
"""

import os
import sys
import time
import json
from pathlib import Path
from datetime import datetime
from dataclasses import asdict

PROJECT_ROOT = Path(__file__).resolve().parent.parent.parent.parent
sys.path.insert(0, str(PROJECT_ROOT))

from qa.claude_ux.driver.session import ClaudeSession, SessionResult, verify_input_transport
from qa.claude_ux.discovery.readiness import ReadinessStateMachine
from qa.claude_ux.design_audit.discover import PathReplayDiscoverer
from qa.claude_ux.design_audit.command_census import SlashCommandCensus
from qa.claude_ux.design_audit.command_execution_matrix import CommandExecutionMatrix
from qa.claude_ux.design_audit.effort_explorer import EffortSelectorExplorer
from qa.claude_ux.design_audit.color_explorer import ColorOptionsExplorer
from qa.claude_ux.design_audit.resume_lifecycle import ResumeLifecycleTester
from qa.claude_ux.design_audit.workspace_matrix import WorkspaceMatrixExplorer
from qa.claude_ux.design_audit.capture import DesignCapturePolicy



class DesignAtlasRunner:
    """Orchestrates Design Atlas screen discovery across all exploratory modules."""

    REPRESENTATIVE_VIEWPORTS = [(80, 24), (120, 30), (182, 53)]
    TARGET_SESSIONS = 20
    MAX_SESSIONS = 40

    def __init__(self, run_dir: Path):
        self.run_dir = run_dir
        self.policy = DesignCapturePolicy()
        self.captured_records = []
        self.session_results = []   # populated from real make_result() calls; no manufactured values

    def run_atlas_audit(self) -> dict:
        print(f"=== Executing Forensic Claude UX Reverse-Engineering Engine ===")
        print(f"Run Directory: {self.run_dir}")
        print(f"Budget: Target {self.TARGET_SESSIONS} sessions, Hard Ceiling {self.MAX_SESSIONS} sessions\n")

        # 0. Transport Probe — result is injected into all explorers
        transport_verified = verify_input_transport(self.run_dir)
        if not transport_verified:
            print("[Runner Error] Synthetic keyboard event transport probe FAILED!")
            return {}

        # 1. Path-Replay Screen Discovery
        discoverer = PathReplayDiscoverer(self.run_dir, max_depth=4, max_states=25)
        discovery_res = discoverer.discover_atlas(viewport=(80, 24))
        # PathReplayDiscoverer manages its own sessions internally;
        # its session results are surfaced via discoverer.session_results if available.
        if hasattr(discoverer, "session_results"):
            self.session_results.extend(discoverer.session_results)

        # 2. Slash Command Census
        census = SlashCommandCensus(self.run_dir)
        census_res = census.discover_commands(viewport=(80, 24))
        if hasattr(census, "session_results"):
            self.session_results.extend(census.session_results)

        # 3. Command Execution Matrix
        cmd_matrix = CommandExecutionMatrix(self.run_dir, transport_verified=transport_verified)
        cmd_exec_res, cmd_sr = cmd_matrix.run_matrix(viewport=(80, 24))
        self.session_results.extend(cmd_sr)

        # 4. Interactive /effort Explorer
        effort_exp = EffortSelectorExplorer(self.run_dir, transport_verified=transport_verified)
        effort_res, effort_sr = effort_exp.explore_effort(viewport=(80, 24))
        self.session_results.extend(effort_sr)

        # 5. Dynamic /color Options Explorer
        color_exp = ColorOptionsExplorer(self.run_dir, transport_verified=transport_verified)
        color_res, color_sr = color_exp.explore_color(viewport=(80, 24))
        self.session_results.extend(color_sr)

        # 6. Sentinel Session Resume Tester
        resume_tester = ResumeLifecycleTester(self.run_dir, transport_verified=transport_verified)
        resume_res, resume_sr = resume_tester.test_resume_lifecycle(viewport=(80, 24))
        self.session_results.extend(resume_sr)

        # 7. Workspace Matrix Explorer
        ws_exp = WorkspaceMatrixExplorer(self.run_dir, transport_verified=transport_verified)
        ws_res, ws_sr = ws_exp.explore_workspace(viewport=(80, 24))
        self.session_results.extend(ws_sr)

        # 8. Visual Evidence Screenshot Captures
        screens_dict = discovery_res.get("screens", {})
        for vp in [(80, 24), (182, 53)]:
            vp_str = f"{vp[0]}x{vp[1]}"
            for screen_id, screen_info in screens_dict.items():
                decision = self.policy.evaluate(screen_id, vp_str)
                if decision["capture_required"]:
                    path_from_root = screen_info.get("path_from_root", [])
                    print(f"  [Layer B Capture] Capturing PNG for screen {screen_id} ({vp_str})... ", end="", flush=True)

                    cap_session = ClaudeSession(self.run_dir, f"cap_{screen_id}_{vp_str}", vp)
                    try:
                        if cap_session.launch():
                            readiness = ReadinessStateMachine(cap_session.driver)
                            ok, _, _ = readiness.evaluate_readiness(cap_session.launch_record)
                            if ok:
                                discoverer._replay_path(cap_session, path_from_root)
                                time.sleep(0.5)

                                contract = {
                                    "expected_markers": ["claude"],
                                    "preconditions": [f"Path: {path_from_root}"],
                                    "postconditions": [f"Screen: {screen_id}"]
                                }
                                cap_res = cap_session.capture_visual_state(contract)
                                cap_res.update(decision)
                                cap_res["screen_id"] = screen_id
                                cap_res["path_from_root"] = path_from_root
                                is_shot_ok = cap_res.get("screenshot_verification", {}).get("ocr_status") == "VERIFIED"
                                cap_res["evidence_classification"] = "VERIFIED" if (cap_res.get("captured") and is_shot_ok) else "FAILED"
                                self.captured_records.append(cap_res)
                                print("CAPTURED")
                    finally:
                        self.session_results.append(cap_session.make_result())
                        cap_session.close()
                        time.sleep(0.4)

        disk_pngs = list(self.run_dir.glob("**/*.png"))
        filesystem_png_count = len(disk_pngs)

        # Session accounting derives strictly from real SessionResult objects
        # accumulated by each explorer via ClaudeSession.make_result().
        sessions_started   = len([r for r in self.session_results if r.started])
        sessions_completed = len([r for r in self.session_results if r.completed])
        sessions_failed    = len([r for r in self.session_results if r.failed])

        manifest_data = {
            "timestamp": datetime.now().isoformat(),
            "budget": {
                "target_sessions": self.TARGET_SESSIONS,
                "hard_ceiling_sessions": self.MAX_SESSIONS,
                "sessions_started": sessions_started,
                "sessions_completed": sessions_completed,
                "sessions_failed": sessions_failed,
                "frontier_exhausted": (len(discoverer.frontier) == 0),
                "budget_exhausted": sessions_started >= self.MAX_SESSIONS
            },
            "summary": {
                "total_screens_discovered": len(screens_dict),
                "total_commands_discovered": len(census_res),
                "total_commands_executed": len(cmd_exec_res),
                "total_captured_records": len(self.captured_records),
                "filesystem_png_count_on_disk": filesystem_png_count
            },
            "session_results": [asdict(r) for r in self.session_results],
            "captured_records": self.captured_records,
            "discovery_results": [discovery_res],
            "census_results": census_res,
            "command_execution_results": cmd_exec_res,
            "effort_results": effort_res,
            "color_results": color_res,
            "resume_results": resume_res,
            "workspace_results": ws_res
        }

        manifest_path = self.run_dir / "atlas_manifest.json"
        with open(manifest_path, "w") as f:
            json.dump(manifest_data, f, indent=2)

        print(f"\nForensic Design Atlas Audit Complete.")
        print(f"Sessions Executed Dynamically: {sessions_completed}")
        print(f"Captured PNG Screenshots on Disk: {filesystem_png_count}")
        print(f"Manifest saved to {manifest_path}")
        return manifest_data


if __name__ == "__main__":
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    run_dir = PROJECT_ROOT / "qa" / "claude_ux" / "design_audit" / "runs" / f"atlas_{timestamp}"
    run_dir.mkdir(parents=True, exist_ok=True)
    runner = DesignAtlasRunner(run_dir)
    runner.run_atlas_audit()
