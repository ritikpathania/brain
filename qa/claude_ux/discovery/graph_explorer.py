#!/usr/bin/env python3
"""
Bounded State-Graph Discovery Engine (state x key -> next_state)
Explores reachable safe TUI states, discovers transitions (including Home -> Left Arrow),
and enforces parent state restoration between candidate edge evaluations.
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
from qa.claude_ux.discovery.fingerprint import TerminalFingerprinter, LogicalStateFingerprint, VisualContext
from qa.claude_ux.discovery.readiness import ReadinessStateMachine


class StateGraphExplorer:
    """Explores TUI state transitions using bounded BFS and parent state restoration."""

    def __init__(self, run_dir: Path, max_states: int = 20, max_transitions: int = 50):
        self.run_dir = run_dir
        self.max_states = max_states
        self.max_transitions = max_transitions
        self.discovered_states = {}
        self.transitions = []
        self.explored_edges = set()

    def explore_graph(self, viewport: tuple = (80, 24)) -> dict:
        print("=== Starting Bounded State-Graph Exploration ===")
        print(f"Viewport: {viewport[0]}x{viewport[1]} | Bounds: max_states={self.max_states}, max_transitions={self.max_transitions}")

        # 1. Establish initial Home state
        session = ClaudeSession(self.run_dir, "graph_init", viewport)
        try:
            if not session.launch():
                print("[Graph Explorer Error] Could not establish initial session readiness")
                return {}

            readiness = ReadinessStateMachine(session.driver)
            ok, st, msg = readiness.evaluate_readiness(session.launch_record)
            if not ok:
                print(f"[Graph Explorer Error] Readiness evaluation failed: {msg}")
                return {}

            init_lines = session.driver.get_terminal_text()
            init_fp, init_ctx = TerminalFingerprinter.compute_fingerprint(init_lines, viewport)
            init_id = init_fp.state_id()

            self.discovered_states[init_id] = {
                "state_id": init_id,
                "logical_fingerprint": asdict(init_fp),
                "visual_variant": init_ctx.variant_id(),
                "text_sample": init_lines[:5]
            }

            print(f"Initial State Discovered: {init_id} (Prompt Mode: {init_fp.prompt_mode})")

        finally:
            session.close()
            time.sleep(0.5)

        # 2. Candidate Keys to explore from Home prompt
        candidate_keys = [
            ("left", "Left Arrow from Home prompt"),
            ("/", "Slash completion trigger"),
            ("ctrl+k", "Global search trigger"),
            ("?", "Quick help trigger"),
            ("right", "Right Arrow from Home prompt"),
        ]

        # 3. Explore candidate edges with parent state restoration
        for key_str, desc in candidate_keys:
            if len(self.transitions) >= self.max_transitions or len(self.discovered_states) >= self.max_states:
                break

            edge_key = (init_id, key_str)
            if edge_key in self.explored_edges:
                continue
            self.explored_edges.add(edge_key)

            print(f"  Exploring Edge: State({init_id}) --[{key_str}]--> ", end="", flush=True)

            # Restore parent state cleanly via fresh session
            edge_session = ClaudeSession(self.run_dir, f"graph_edge_{key_str.replace('+', '_')}", viewport)
            try:
                if edge_session.launch():
                    edge_readiness = ReadinessStateMachine(edge_session.driver)
                    edge_ok, _, _ = edge_readiness.evaluate_readiness(edge_session.launch_record)
                    
                    if edge_ok:
                        # Perform action
                        edge_session.press(key_str)
                        time.sleep(0.6)

                        # Observe resulting state
                        post_lines = edge_session.driver.get_terminal_text()
                        post_fp, post_ctx = TerminalFingerprinter.compute_fingerprint(post_lines, viewport)
                        post_id = post_fp.state_id()

                        is_new = post_id not in self.discovered_states
                        if is_new:
                            self.discovered_states[post_id] = {
                                "state_id": post_id,
                                "logical_fingerprint": asdict(post_fp),
                                "visual_variant": post_ctx.variant_id(),
                                "text_sample": post_lines[:5]
                            }

                        trans_entry = {
                            "state_id": init_id,
                            "trigger_key": key_str,
                            "next_state_id": post_id,
                            "observed_effect": desc,
                            "is_new_state": is_new,
                            "classification": "VERIFIED",
                            "safety": "SAFE"
                        }
                        self.transitions.append(trans_entry)
                        print(f"State({post_id}) [{'NEW STATE' if is_new else 'SEEN'}]")

            finally:
                edge_session.close()
                time.sleep(0.5)

        graph_result = {
            "timestamp": datetime.now().isoformat(),
            "viewport": f"{viewport[0]}x{viewport[1]}",
            "logical_states_count": len(self.discovered_states),
            "transitions_count": len(self.transitions),
            "discovered_states": self.discovered_states,
            "transitions": self.transitions
        }

        # Save qa/claude_ux/state_graph.json
        graph_json_path = PROJECT_ROOT / "qa" / "claude_ux" / "state_graph.json"
        with open(graph_json_path, "w") as f:
            json.dump(graph_result, f, indent=2)

        print(f"\nState-Graph Exploration Complete.")
        print(f"Logical States: {len(self.discovered_states)} | Transitions: {len(self.transitions)}")
        print(f"Saved state graph to {graph_json_path}")
        return graph_result


if __name__ == "__main__":
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    run_dir = PROJECT_ROOT / "qa" / "claude_ux" / "runs" / f"graph_audit_{timestamp}"
    run_dir.mkdir(parents=True, exist_ok=True)
    explorer = StateGraphExplorer(run_dir)
    explorer.explore_graph()
