#!/usr/bin/env python3
"""
Path-Replay Recursive Screen & Control Discovery Engine
Recursively explores screen states using exact path-from-root replay (replay(S.path_from_root) then press K).
Dynamically combines status-line advertised controls (STATUS_ADVERTISED) + source controls (SOURCE_CONFIRMED),
and assigns VERIFIED strictly upon empirically observed state transitions.
"""

import os
import sys
import time
import json
from pathlib import Path
from dataclasses import asdict
from datetime import datetime

PROJECT_ROOT = Path(__file__).resolve().parent.parent.parent.parent
sys.path.insert(0, str(PROJECT_ROOT))

from qa.claude_ux.driver.session import ClaudeSession
from qa.claude_ux.discovery.readiness import ReadinessStateMachine
from qa.claude_ux.design_audit.state_machine import (
    StructuralStateAnalyzer,
    ScreenFingerprint,
    ScreenChrome,
    ScreenState,
    StatusBarControl,
    classify_evidence,
)


class PathReplayDiscoverer:
    """Path-Replay Screen Explorer: Replays exact path_from_root before evaluating candidate keys."""

    GLOBAL_SOURCE_KEYS = [
        ("left", "Navigate to left panel", "SOURCE_CONFIRMED"),
        ("/", "Trigger slash command completion", "SOURCE_CONFIRMED"),
        ("ctrl+k", "Open global search dialog", "SOURCE_CONFIRMED"),
        ("?", "Trigger quick usage help", "SOURCE_CONFIRMED"),
        ("up", "Navigate selection up / history up", "SOURCE_CONFIRMED"),
        ("down", "Navigate selection down / history down", "SOURCE_CONFIRMED"),
        ("tab", "Accept completion / switch focus", "SOURCE_CONFIRMED"),
        ("enter", "Confirm selection / submit prompt", "SOURCE_CONFIRMED"),
        ("esc", "Dismiss active overlay / back", "SOURCE_CONFIRMED"),
    ]

    def __init__(self, run_dir: Path, max_depth: int = 4, max_states: int = 25):
        self.run_dir = run_dir
        self.max_depth = max_depth
        self.max_states = max_states
        self.discovered_screens = {}
        self.screen_transitions = []
        self.explored_edges = set()
        self.frontier = []

    def _replay_path(self, session: ClaudeSession, path: list) -> bool:
        """Replays sequence of key actions from Home root state."""
        for key in path:
            session.press(key)
            time.sleep(0.4)
        return True

    def discover_atlas(self, viewport: tuple = (80, 24)) -> dict:
        print(f"=== Starting Path-Replay Screen Discovery Engine ({viewport[0]}x{viewport[1]}) ===")
        
        # 1. Establish Root Home Screen
        session = ClaudeSession(self.run_dir, "atlas_root_home", viewport)
        try:
            if not session.launch():
                print("[Atlas Discovery Error] Launch failed")
                return {}

            readiness = ReadinessStateMachine(session.driver)
            ok, st, msg = readiness.evaluate_readiness(session.launch_record)
            if not ok:
                print(f"[Atlas Discovery Error] Readiness failed: {msg}")
                return {}

            init_lines = session.observe_terminal_state()
            init_fp, init_chrome, vp_str = StructuralStateAnalyzer.analyze(init_lines, viewport)
            root_id = init_fp.state_id()

            # Dynamic candidate keys = STATUS_ADVERTISED + SOURCE_CONFIRMED
            initial_controls = []
            for fc in init_chrome.footer_controls:
                initial_controls.append(fc)
            for k, act, ev in self.GLOBAL_SOURCE_KEYS:
                if not any(c.get("key") == k for c in initial_controls):
                    initial_controls.append({"key": k, "label": k, "action": act, "evidence": ev})

            root_state = ScreenState(
                screen_id=root_id,
                title="01 Home Screen",
                category=init_fp.screen_category,
                chrome=asdict(init_chrome),
                focused_element=init_fp.focused_element,
                overlay_identity=init_fp.overlay_identity,
                navigation_region=init_fp.navigation_region,
                structural_geometry=init_fp.structural_geometry,
                prompt_mode=init_fp.prompt_mode,
                text_sample=init_lines[:6],
                viewport=vp_str,
                available_controls=initial_controls,
                path_from_root=[]
            )
            root_state_dict = root_state.to_dict()
            root_state_dict["evidence_classification"] = "VERIFIED"
            self.discovered_screens[root_id] = root_state_dict
            print(f"Root Screen Discovered: {root_id} ({root_state.category})")

        finally:
            session.close()
            time.sleep(0.5)

        # 2. Path-Replay Exploration Frontier
        self.frontier = [root_id]

        while self.frontier and len(self.discovered_screens) < self.max_states:
            curr_id = self.frontier.pop(0)
            curr_state_dict = self.discovered_screens[curr_id]
            curr_path = curr_state_dict.get("path_from_root", [])
            depth = len(curr_path)

            if depth >= self.max_depth:
                continue

            candidate_controls = curr_state_dict.get("available_controls", [])
            for ctrl in candidate_controls:
                if len(self.discovered_screens) >= self.max_states:
                    break

                key_code = ctrl["key"]
                edge_key = (curr_id, key_code)
                if edge_key in self.explored_edges:
                    continue
                self.explored_edges.add(edge_key)

                print(f"  [Path {curr_path} + '{key_code}'] Exploring Edge: {curr_id} --[{key_code}]--> ", end="", flush=True)

                # Path-Replay: Launch fresh session and replay path_from_root before pressing candidate key
                edge_session = ClaudeSession(self.run_dir, f"atlas_path_{'_'.join(curr_path)}_{key_code.replace('+', '_')}", viewport)
                try:
                    if edge_session.launch():
                        edge_readiness = ReadinessStateMachine(edge_session.driver)
                        edge_ok, _, _ = edge_readiness.evaluate_readiness(edge_session.launch_record)
                        
                        if edge_ok:
                            # Replay path from root to reach parent state S
                            self._replay_path(edge_session, curr_path)

                            # Execute candidate key K
                            edge_session.press(key_code)
                            time.sleep(0.6)

                            # Observe resulting screen state
                            post_lines = edge_session.observe_terminal_state()
                            post_fp, post_chrome, post_vp_str = StructuralStateAnalyzer.analyze(post_lines, viewport)
                            post_id = post_fp.state_id()

                            is_state_changed = (post_id != curr_id)
                            evidence_level = classify_evidence(
                                action_executed=True,
                                transport_verified=edge_ok,
                                parent_state_known=True,
                                post_state_observed=bool(post_lines),
                                transition_matches_expectation=is_state_changed
                            )
                            ctrl["evidence"] = evidence_level

                            is_new_screen = post_id not in self.discovered_screens
                            if is_new_screen and is_state_changed:
                                new_path = curr_path + [key_code]
                                
                                new_controls = []
                                for fc in post_chrome.footer_controls:
                                    new_controls.append(fc)
                                for k, act, ev in self.GLOBAL_SOURCE_KEYS:
                                    if not any(c.get("key") == k for c in new_controls):
                                        new_controls.append({"key": k, "label": k, "action": act, "evidence": ev})

                                screen_obj = ScreenState(
                                    screen_id=post_id,
                                    title=f"Screen {post_fp.screen_category}",
                                    category=post_fp.screen_category,
                                    chrome=asdict(post_chrome),
                                    focused_element=post_fp.focused_element,
                                    overlay_identity=post_fp.overlay_identity,
                                    navigation_region=post_fp.navigation_region,
                                    structural_geometry=post_fp.structural_geometry,
                                    prompt_mode=post_fp.prompt_mode,
                                    text_sample=post_lines[:6],
                                    viewport=post_vp_str,
                                    available_controls=new_controls,
                                    path_from_root=new_path
                                )
                                screen_dict = screen_obj.to_dict()
                                screen_dict["evidence_classification"] = "VERIFIED"
                                self.discovered_screens[post_id] = screen_dict
                                self.frontier.append(post_id)

                            trans_entry = {
                                "source_screen_id": curr_id,
                                "path_to_source": curr_path,
                                "trigger_key": key_code,
                                "target_screen_id": post_id,
                                "is_state_changed": is_state_changed,
                                "is_new_screen": is_new_screen,
                                "evidence_classification": evidence_level
                            }
                            self.screen_transitions.append(trans_entry)
                            print(f"{post_id} [{'NEW SCREEN' if is_new_screen else ('CHANGED' if is_state_changed else 'UNCHANGED')}]")

                finally:
                    edge_session.close()
                    time.sleep(0.4)

        atlas_data = {
            "timestamp": datetime.now().isoformat(),
            "viewport": f"{viewport[0]}x{viewport[1]}",
            "total_screens_discovered": len(self.discovered_screens),
            "total_transitions_mapped": len(self.screen_transitions),
            "screens": self.discovered_screens,
            "transitions": self.screen_transitions
        }

        out_json = self.run_dir / "discovered_atlas_states.json"
        with open(out_json, "w") as f:
            json.dump(atlas_data, f, indent=2)

        print(f"\nPath-Replay Atlas Screen Discovery Complete.")
        print(f"Screens Discovered: {len(self.discovered_screens)} | Transitions Mapped: {len(self.screen_transitions)}")
        print(f"Saved atlas state graph to: {out_json}")
        return atlas_data


if __name__ == "__main__":
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    run_dir = PROJECT_ROOT / "qa" / "claude_ux" / "design_audit" / "runs" / f"discovery_{timestamp}"
    run_dir.mkdir(parents=True, exist_ok=True)
    discoverer = PathReplayDiscoverer(run_dir)
    discoverer.discover_atlas()
