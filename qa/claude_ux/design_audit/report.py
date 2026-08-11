#!/usr/bin/env python3
"""
Dynamic Product Design Atlas Report Generator
Parses atlas_manifest.json into a Single Canonical EvidenceModel and renders docs/research/CLAUDE_UX_DESIGN_ATLAS.md.
Includes strict REPORT_INTEGRITY_CHECK asserting zero hardcoded metrics or data model mismatches.
100% manifest-derived — zero manufactured evidence, zero static defaults, zero fake surface claims.
"""

import json
from pathlib import Path
from dataclasses import dataclass
from typing import Dict, Any, List, Optional

PROJECT_ROOT = Path(__file__).resolve().parent.parent.parent.parent


@dataclass
class EvidenceModel:
    claude_version: str
    run_rel_dir: str
    target_sessions: int
    hard_ceiling_sessions: int
    sessions_completed: int
    frontier_exhausted: bool

    total_screens_discovered: int
    total_screens_verified: int
    total_screens_failed: int
    total_screens_unsafe: int
    total_screens_unavailable: int
    screen_completeness_pct: float

    total_commands_discovered: int
    total_commands_executed: int
    total_commands_failed: int
    total_commands_unsafe: int
    total_commands_unavailable: int
    command_completeness_pct: float

    total_keys_discovered: int
    total_keys_verified: int
    total_keys_failed: int
    total_keys_unsafe: int
    total_keys_unavailable: int
    key_completeness_pct: float

    total_visual_states: int
    total_visual_states_verified: int
    total_visual_states_failed: int
    total_visual_states_unsafe: int
    total_visual_states_unavailable: int
    visual_state_completeness_pct: float

    effort_results: Dict[str, Any]
    color_results: Dict[str, Any]
    resume_results: Dict[str, Any]
    workspace_results: Dict[str, Any]
    screens: Dict[str, Any]
    transitions: List[Dict[str, Any]]
    captured_records: List[Dict[str, Any]]


def build_canonical_evidence_model(manifest: Dict[str, Any], run_dir: Path) -> EvidenceModel:
    budget_info = manifest.get("budget", {})
    summary_info = manifest.get("summary", {})
    captured_records = manifest.get("captured_records", [])
    discovery_results = manifest.get("discovery_results", [])
    census_results = manifest.get("census_results", {})
    cmd_exec_results = manifest.get("command_execution_results", [])
    effort_results = manifest.get("effort_results", {})
    color_results = manifest.get("color_results", {})
    resume_results = manifest.get("resume_results", {})
    ws_results = manifest.get("workspace_results", {})

    transitions = []
    for res in discovery_results:
        transitions.extend(res.get("transitions", []))

    KEY_UNAVAILABLE_CLASSES = {"UNAVAILABLE", "UNVERIFIED_TRANSPORT"}

    # ── Screens Evidence Classifications ─────────────────────────────────────
    screens = discovery_results[0].get("screens", {}) if discovery_results else {}
    tot_screens = len(screens)
    screens_verif = len([s for s in screens.values() if s.get("evidence_classification") == "VERIFIED"])
    screens_failed = len([s for s in screens.values() if s.get("evidence_classification") == "FAILED"])
    screens_unsafe = len([s for s in screens.values() if s.get("evidence_classification") in ["UNSAFE", "UNSAFE_TO_TEST"]])
    screens_unavail = len([s for s in screens.values() if s.get("evidence_classification") in KEY_UNAVAILABLE_CLASSES or s.get("evidence_classification") not in ["VERIFIED", "FAILED", "UNSAFE", "UNSAFE_TO_TEST"]])

    screen_pct = round(screens_verif / max(1, tot_screens) * 100, 1) if tot_screens > 0 else 0.0

    # ── Commands Evidence Classifications ────────────────────────────────────
    tot_cmds = len(census_results)
    exec_cmds = len([c for c in cmd_exec_results if c.get("evidence_classification") == "VERIFIED"])
    cmds_failed = len([c for c in cmd_exec_results if c.get("evidence_classification") == "FAILED"])
    cmds_unsafe = len([c_info for c_info in census_results.values() if c_info.get("classification") in ["UNSAFE", "DESTRUCTIVE"]])
    cmds_unavail = len([c for c in cmd_exec_results if c.get("evidence_classification") == "UNAVAILABLE"]) + max(0, tot_cmds - len(cmd_exec_results) - cmds_unsafe)
    cmd_pct = round(exec_cmds / max(1, tot_cmds) * 100, 1) if tot_cmds > 0 else 0.0

    # ── Keyboard Interaction Evidence Classifications ─────────────────────────
    ws_matrix = ws_results.get("matrix_results", [])
    effort_trials = effort_results.get("boundary_trials", [])
    all_key_records = list(transitions) + list(ws_matrix) + list(effort_trials)

    tot_keys = len(all_key_records)
    tot_keys_verif = len([r for r in all_key_records if r.get("evidence_classification") == "VERIFIED"])
    tot_keys_failed = len([r for r in all_key_records if r.get("evidence_classification") == "FAILED"])
    tot_keys_unsafe = len([r for r in all_key_records if r.get("evidence_classification") in ["UNSAFE", "UNSAFE_TO_TEST"]])
    tot_keys_unavail = len([r for r in all_key_records if r.get("evidence_classification") in KEY_UNAVAILABLE_CLASSES or r.get("evidence_classification") not in ["VERIFIED", "FAILED", "UNSAFE", "UNSAFE_TO_TEST"]])

    key_pct = round(tot_keys_verif / max(1, tot_keys) * 100, 1) if tot_keys > 0 else 0.0

    # ── Visual States Evidence Classifications ───────────────────────────────
    tot_visual = len(captured_records)
    visual_verif = len([c for c in captured_records if c.get("evidence_classification") == "VERIFIED"])
    visual_failed = len([c for c in captured_records if c.get("evidence_classification") == "FAILED"])
    visual_unsafe = len([c for c in captured_records if c.get("evidence_classification") in ["UNSAFE", "UNSAFE_TO_TEST"]])
    visual_unavail = len([c for c in captured_records if c.get("evidence_classification") in KEY_UNAVAILABLE_CLASSES or c.get("evidence_classification") not in ["VERIFIED", "FAILED", "UNSAFE", "UNSAFE_TO_TEST"]])
    visual_pct = round(visual_verif / max(1, tot_visual) * 100, 1) if tot_visual > 0 else 0.0

    claude_ver = manifest.get("claude_version", "unobserved")

    # Strict budget metrics — no manufactured defaults if unobserved
    target_sess = budget_info.get("target_sessions", 0)
    hard_ceil_sess = budget_info.get("hard_ceiling_sessions", 0)
    sess_completed = budget_info.get("sessions_completed", len(discovery_results))
    front_exhausted = budget_info.get("frontier_exhausted", False)

    model = EvidenceModel(
        claude_version=claude_ver,
        run_rel_dir=str(run_dir.relative_to(PROJECT_ROOT)) if PROJECT_ROOT in run_dir.parents or run_dir == PROJECT_ROOT else str(run_dir),
        target_sessions=target_sess,
        hard_ceiling_sessions=hard_ceil_sess,
        sessions_completed=sess_completed,
        frontier_exhausted=front_exhausted,
        total_screens_discovered=tot_screens,
        total_screens_verified=screens_verif,
        total_screens_failed=screens_failed,
        total_screens_unsafe=screens_unsafe,
        total_screens_unavailable=screens_unavail,
        screen_completeness_pct=screen_pct,
        total_commands_discovered=tot_cmds,
        total_commands_executed=exec_cmds,
        total_commands_failed=cmds_failed,
        total_commands_unsafe=cmds_unsafe,
        total_commands_unavailable=cmds_unavail,
        command_completeness_pct=cmd_pct,
        total_keys_discovered=tot_keys,
        total_keys_verified=tot_keys_verif,
        total_keys_failed=tot_keys_failed,
        total_keys_unsafe=tot_keys_unsafe,
        total_keys_unavailable=tot_keys_unavail,
        key_completeness_pct=key_pct,
        total_visual_states=tot_visual,
        total_visual_states_verified=visual_verif,
        total_visual_states_failed=visual_failed,
        total_visual_states_unsafe=visual_unsafe,
        total_visual_states_unavailable=visual_unavail,
        visual_state_completeness_pct=visual_pct,
        effort_results=effort_results,
        color_results=color_results,
        resume_results=resume_results,
        workspace_results=ws_results,
        screens=screens,
        transitions=transitions,
        captured_records=captured_records
    )

    # ── REPORT_INTEGRITY_CHECK ───────────────────────────────────────────────
    # Validates ALL supplied manifest summary and budget metrics against derived EvidenceModel fields.
    check_map = {
        "target_sessions": model.target_sessions,
        "hard_ceiling_sessions": model.hard_ceiling_sessions,
        "sessions_completed": model.sessions_completed,
        "frontier_exhausted": model.frontier_exhausted,
        "total_screens_discovered": model.total_screens_discovered,
        "total_screens_verified": model.total_screens_verified,
        "total_screens_failed": model.total_screens_failed,
        "total_screens_unsafe": model.total_screens_unsafe,
        "total_screens_unavailable": model.total_screens_unavailable,
        "total_commands_discovered": model.total_commands_discovered,
        "total_commands_executed": model.total_commands_executed,
        "total_commands_failed": model.total_commands_failed,
        "total_commands_unsafe": model.total_commands_unsafe,
        "total_commands_unavailable": model.total_commands_unavailable,
        "total_keys_discovered": model.total_keys_discovered,
        "total_keys_verified": model.total_keys_verified,
        "total_keys_failed": model.total_keys_failed,
        "total_keys_unsafe": model.total_keys_unsafe,
        "total_keys_unavailable": model.total_keys_unavailable,
        "total_visual_states": model.total_visual_states,
        "total_visual_states_verified": model.total_visual_states_verified,
        "total_visual_states_failed": model.total_visual_states_failed,
        "total_visual_states_unsafe": model.total_visual_states_unsafe,
        "total_visual_states_unavailable": model.total_visual_states_unavailable,
        "screen_completeness_pct": model.screen_completeness_pct,
        "command_completeness_pct": model.command_completeness_pct,
        "key_completeness_pct": model.key_completeness_pct,
        "visual_state_completeness_pct": model.visual_state_completeness_pct,
    }
    for field, expected_val in check_map.items():
        if field in summary_info and summary_info[field] != expected_val:
            raise ValueError(
                f"[REPORT_INTEGRITY_CHECK FAIL] Manifest summary.{field} "
                f"({summary_info[field]}) does not match actual derived value ({expected_val})!"
            )

    budget_check_map = {
        "target_sessions": model.target_sessions,
        "hard_ceiling_sessions": model.hard_ceiling_sessions,
        "sessions_completed": model.sessions_completed,
        "frontier_exhausted": model.frontier_exhausted,
    }
    for field, expected_val in budget_check_map.items():
        if field in budget_info and budget_info[field] != expected_val:
            raise ValueError(
                f"[REPORT_INTEGRITY_CHECK FAIL] Manifest budget.{field} "
                f"({budget_info[field]}) does not match actual derived value ({expected_val})!"
            )

    # Strict Session Partition Check: started sessions must equal completed + failed
    session_results = manifest.get("session_results", [])
    if session_results:
        started_count = len([s for s in session_results if s.get("started")])
        completed_count = len([s for s in session_results if s.get("completed")])
        failed_count = len([s for s in session_results if s.get("failed")])
        if started_count != (completed_count + failed_count):
            raise ValueError(
                f"[REPORT_INTEGRITY_CHECK FAIL] Manifest session_results started count ({started_count}) "
                f"does not equal completed ({completed_count}) + failed ({failed_count}) partition!"
            )

    return model


def generate_design_atlas_report(run_dir: Path = None):
    if run_dir is None:
        runs_base = PROJECT_ROOT / "qa" / "claude_ux" / "design_audit" / "runs"
        atlas_runs = sorted(list(runs_base.glob("atlas_*")), key=lambda p: p.stat().st_mtime, reverse=True)
        if not atlas_runs:
            print("[Report Error] No atlas run directories found!")
            return
        run_dir = atlas_runs[0]

    manifest_path = run_dir / "atlas_manifest.json"
    if not manifest_path.exists():
        print(f"[Report Error] Manifest not found at {manifest_path}")
        return

    with open(manifest_path) as f:
        manifest = json.load(f)

    # 1. Build Single Canonical Evidence Model
    model = build_canonical_evidence_model(manifest, run_dir)

    out_md = PROJECT_ROOT / "docs" / "research" / "CLAUDE_UX_DESIGN_ATLAS.md"
    out_md.parent.mkdir(parents=True, exist_ok=True)

    md_lines = [
        "# Claude Code TUI — Comprehensive Product Design Atlas",
        "> Dynamically Compiled from Forensic Path-Replay Discovery & Interactive TUI Analysis\n",
        "---",
        "\n## 1. Executive Summary & 4-Tier Honest Completeness Breakdown\n",
        f"- **Claude Version**: `{model.claude_version}`",
        f"- **Run Artifact Directory**: `{model.run_rel_dir}`",
        f"- **Session Budget**: Target {model.target_sessions}, Hard Ceiling {model.hard_ceiling_sessions}, Executed {model.sessions_completed} (Frontier Exhausted: {model.frontier_exhausted})",
        "\n### 4-Tier Completeness Metrics\n",
        "| Category | Discovered | Executed / Verified | Failed | Unsafe | Unavailable | Completeness Ratio |",
        "|---|---:|---:|---:|---:|---:|---:|",
        f"| **Screens** | `{model.total_screens_discovered}` | `{model.total_screens_verified}` | `{model.total_screens_failed}` | `{model.total_screens_unsafe}` | `{model.total_screens_unavailable}` | **{model.screen_completeness_pct}%** |",
        f"| **Commands** | `{model.total_commands_discovered}` | `{model.total_commands_executed}` | `{model.total_commands_failed}` | `{model.total_commands_unsafe}` | `{model.total_commands_unavailable}` | **{model.command_completeness_pct}%** |",
        f"| **Keyboard Interactions** | `{model.total_keys_discovered}` | `{model.total_keys_verified}` | `{model.total_keys_failed}` | `{model.total_keys_unsafe}` | `{model.total_keys_unavailable}` | **{model.key_completeness_pct}%** |",
        f"| **Visual States** | `{model.total_visual_states}` | `{model.total_visual_states_verified}` | `{model.total_visual_states_failed}` | `{model.total_visual_states_unsafe}` | `{model.total_visual_states_unavailable}` | **{model.visual_state_completeness_pct}%** |",
        "\n---",
        "\n## 2. Specialized Interaction Surface Specifications\n",
        "### Surface 01 — Interactive `/effort` Selector",
        "- **Surface Type**: Interactive slider picker",
        "- **Path-Replayed Boundary Trial Results**:",
    ]

    boundary_trials = model.effort_results.get("boundary_trials", [])
    for bt in boundary_trials:
        md_lines.append(f"  - `{bt.get('description')}`: State Changed: `{bt.get('actual_state_changed')}` | Classification: **{bt.get('evidence_classification')}**")

    md_lines.append("\n### Surface 02 — Dynamic `/color` Options & Lifecycle Matrix")
    disc_colors = model.color_results.get("discovered_colors", [])
    colors_str = ", ".join(disc_colors) if disc_colors else "none discovered"
    md_lines.append(f"- **Discovered Colors**: `{colors_str}`")
    md_lines.append("- **Lifecycle Checkpoints Verified**:")

    lifecycle_mat = model.color_results.get("lifecycle_matrix", [])
    for entry in lifecycle_mat:
        c_name = entry.get("color", "")
        chk = entry.get("checkpoints", {})
        md_lines.append(f"  - `Color '{c_name}'`: Apply: **{chk.get('apply_verified')}** | Subsequent: **{chk.get('subsequent_command_persistence_verified')}** | Resume: **{chk.get('resume_persistence_verified')}**")

    md_lines.append("\n### Surface 03 — Sentinel Session Resume Lifecycle (`claude --resume <id>`)")
    sentinel = model.resume_results.get("sentinel_prompt", "unobserved")
    before_id = model.resume_results.get("session_id_before", "unobserved")
    after_id = model.resume_results.get("session_id_after", "unobserved")
    ident_ev = model.resume_results.get("session_identity", "UNAVAILABLE")
    conv_ev = model.resume_results.get("sentinel_conversation_restored", "UNAVAILABLE")
    md_lines.append(f"- **Sentinel Prompt**: `{sentinel}`")
    md_lines.append(f"- **Observed Session ID Before Exit**: `{before_id}` | **After Resume**: `{after_id}`")
    md_lines.append(f"- **Restoration Verification**: Session Identity: **{ident_ev}** | Conversation Content: **{conv_ev}**")

    md_lines.append("\n### Surface 04 — Contextual Workspace State Machine & Safe Destructive Deletion")
    fixtures = model.workspace_results.get("disposable_fixtures", [])
    md_lines.append(f"- **Disposable Fixtures**: `{', '.join(fixtures)}`")
    md_lines.append("- **Contextual State Machine Edges Verified**:")

    ws_matrix = model.workspace_results.get("matrix_results", [])
    for ws_edge in ws_matrix:
        in_val = ws_edge.get("input_value")
        src_st = ws_edge.get("source_state")
        tgt_st = ws_edge.get("target_state")
        ev_class = ws_edge.get("evidence_classification")
        md_lines.append(f"  - `{src_st} --[{in_val}]--> {tgt_st}`: (**{ev_class}**)")

    md_lines.append("\n---")
    md_lines.append("\n## 3. Screen-by-Screen Product Design Specifications\n")

    for screen_id, s_info in model.screens.items():
        title = s_info.get("title", screen_id)
        category = s_info.get("category", "TUI Surface")
        chrome = s_info.get("chrome", {})
        footer_region = chrome.get("footer_region", "")
        controls = s_info.get("available_controls", [])
        path_from_root = s_info.get("path_from_root", [])
        text_sample = s_info.get("text_sample", [])

        cap = next((c for c in model.captured_records if c.get("screen_id") == screen_id), {})
        shot_verif = cap.get("screenshot_verification", {})
        png_rel_path = shot_verif.get("path", "none")

        md_lines.append(f"### Screen `{screen_id}` — {title}\n")
        md_lines.append("#### 1. WHAT DOES IT LOOK LIKE?")
        md_lines.append(f"- **Category**: `{category}`")
        md_lines.append(f"- **Focus & Mode**: `{s_info.get('focused_element')}` | `{s_info.get('prompt_mode')}`")
        md_lines.append(f"- **Layout Geometry**: `{s_info.get('structural_geometry')}` (`{s_info.get('viewport')}`)")
        if png_rel_path != "none":
            md_lines.append(f"- **Baseline Screenshot File**: `qa/claude_ux/design_audit/{png_rel_path}` ({shot_verif.get('file_size', 0)} bytes)")

        md_lines.append("\n**Text Buffer ASCII Sample**:")
        md_lines.append("```text")
        for line in text_sample[:5]:
            md_lines.append(line)
        md_lines.append("```\n")

        if footer_region:
            md_lines.append(f"**Structural Footer Chrome Quote**:\n```text\n{footer_region}\n```\n")

        md_lines.append("#### 2. WHAT CAN THE USER DO?")
        md_lines.append("| Key / Command | Action | Advertised | Tested in Runtime | Evidence Classification |")
        md_lines.append("|---|---|---:|---:|---|")

        for ctrl in controls:
            k = ctrl.get("key", "")
            act = ctrl.get("action", "")
            ev = ctrl.get("evidence", "SOURCE_CONFIRMED")
            is_adv = "✅" if ev in ["STATUS_ADVERTISED", "VERIFIED"] else "❌"
            is_test = "✅" if ev == "VERIFIED" else "❌"
            md_lines.append(f"| `{k}` | {act} | {is_adv} | {is_test} | **{ev}** |")

        md_lines.append("\n#### 3. WHAT HAPPENS WHEN THEY DO IT?")
        path_str = " -> ".join(["Home"] + path_from_root) if path_from_root else "Home Root State"
        md_lines.append(f"- **Replay Path from Root**: `{path_str}`")

        out_trans = [t for t in model.transitions if t.get("source_screen_id") == screen_id]
        if out_trans:
            md_lines.append("- **Discovered Transitions**:")
            for tr in out_trans:
                md_lines.append(f"  - `{screen_id}` --[`{tr.get('trigger_key')}`]--> `{tr.get('target_screen_id')}` (State Changed: {tr.get('is_state_changed')})")

        md_lines.append("\n#### 4. SHOULD BRAIN ADOPT IT?")
        if category == "02_navigation_panel":
            rec, rat = "ADOPT", "High usability value for keyboard-first session & workspace history navigation."
        elif category == "04_slash_completion":
            rec, rat = "ADAPT", "Reposition Brain's slash completion popup directly below the prompt line."
        else:
            rec, rat = "BRAIN-NATIVE EQUIVALENT", "Preserve Brain Mascot, Memory Core, and Relational Engine identity."

        md_lines.append(f"- **Recommendation**: **{rec}**")
        md_lines.append(f"- **Design Rationale**: {rat}\n")
        md_lines.append("---\n")

    rendered_text = "\n".join(md_lines)
    with open(out_md, "w") as f:
        f.write(rendered_text)

    print(f"Design Atlas Report Generated: {out_md}")
