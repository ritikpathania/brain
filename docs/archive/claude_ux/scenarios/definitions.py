#!/usr/bin/env python3
"""
Standard 20 Scenario Contracts with Explicit OCR Validation Contracts, Preconditions & Postconditions.
"""

SCENARIOS = [
    {
        "id": "01_home",
        "description": "Initial startup Home landing surface rendering identity logo",
        "expected_markers": ["claude", "welcome", "❯", "code", "help"],
        "preconditions": ["Terminal window created", "Claude interactive TUI process verified"],
        "actions": [("wait", 1.0)],
        "postconditions": ["Home screen rendered with prompt active"]
    },
    {
        "id": "02_home_focused_prompt",
        "description": "Home screen with active text in prompt input box",
        "expected_markers": ["hello", "❯"],
        "preconditions": ["Home screen visible"],
        "actions": [("type", "hello"), ("wait", 0.5)],
        "postconditions": ["Typed string visible in prompt line"]
    },
    {
        "id": "03_slash_completion",
        "description": "Slash command completion popup menu open below prompt",
        "expected_markers": ["/", "session", "theme", "help", "clear"],
        "preconditions": ["Prompt line active"],
        "actions": [("press", "esc"), ("type", "/"), ("wait", 0.6)],
        "postconditions": ["Slash completion list rendered below prompt line"]
    },
    {
        "id": "04_slash_filtered_completion",
        "description": "Slash completion list filtered by character sequence '/th'",
        "expected_markers": ["/th", "theme"],
        "preconditions": ["Slash completion visible"],
        "actions": [("type", "th"), ("wait", 0.5)],
        "postconditions": ["Theme command highlighted in filtered list"]
    },
    {
        "id": "05_ctrl_k_global_search",
        "description": "Global search discovery dialog opened via shortcut Ctrl+K",
        "expected_markers": ["search", "command"],
        "preconditions": ["Terminal window active"],
        "actions": [("press", "esc"), ("press", "ctrl+k"), ("wait", 0.6)],
        "postconditions": ["Global search modal visible with search input focused"]
    },
    {
        "id": "06_ctrl_k_filtered_search",
        "description": "Global search dialog filtered by term 'theme'",
        "expected_markers": ["theme"],
        "preconditions": ["Ctrl+K search dialog open"],
        "actions": [("type", "theme"), ("wait", 0.6)],
        "postconditions": ["Matching commands filtered in dialog"]
    },
    {
        "id": "07_theme_picker",
        "description": "Theme selection dialog presenting palette choices",
        "expected_markers": ["theme", "dark", "light"],
        "preconditions": ["Interactive prompt active"],
        "actions": [("press", "esc"), ("type", "/theme"), ("press", "enter"), ("wait", 0.8)],
        "postconditions": ["Theme picker options displayed"]
    },
    {
        "id": "08_help",
        "description": "Help overview menu listing supported commands",
        "expected_markers": ["help", "commands"],
        "preconditions": ["Interactive prompt active"],
        "actions": [("press", "esc"), ("type", "/help"), ("press", "enter"), ("wait", 0.8)],
        "postconditions": ["Help documentation text rendered"]
    },
    {
        "id": "09_status",
        "description": "Status overview panel presenting session cost & token limit",
        "expected_markers": ["status", "cost", "tokens", "session"],
        "preconditions": ["Interactive prompt active"],
        "actions": [("press", "esc"), ("type", "/status"), ("press", "enter"), ("wait", 0.8)],
        "postconditions": ["Status summary panel displayed"]
    },
    {
        "id": "10_workspace_query",
        "description": "Submitting workspace query to enter conversation view",
        "expected_markers": ["explain", "quantum"],
        "preconditions": ["Interactive prompt active"],
        "actions": [("press", "esc"), ("type", "explain quantum computing in 1 sentence"), ("wait", 0.5)],
        "postconditions": ["Query text typed into active prompt"]
    },
    {
        "id": "11_streaming_response",
        "description": "Active live streaming response in workspace conversation timeline",
        "expected_markers": ["quantum", "computing"],
        "preconditions": ["Workspace query submitted"],
        "actions": [("press", "enter"), ("wait", 1.2)],
        "postconditions": ["Assistant response timeline rendered"]
    },
    {
        "id": "12_scrolled_workspace",
        "description": "Workspace message history scrolled upward",
        "expected_markers": ["explain", "quantum"],
        "preconditions": ["Conversation timeline active"],
        "actions": [("press", "up"), ("press", "up"), ("wait", 0.5)],
        "postconditions": ["Viewport scrolled upward in conversation history"]
    },
    {
        "id": "13_unseen_message_state",
        "description": "Unseen message indicator pill when scrolled up during activity",
        "expected_markers": ["quantum"],
        "preconditions": ["Conversation timeline scrolled up"],
        "actions": [("wait", 0.5)],
        "postconditions": ["Unread message boundary line visible"]
    },
    {
        "id": "14_long_prompt",
        "description": "Multiline prompt with extensive input context",
        "expected_markers": ["detailed", "context"],
        "preconditions": ["Interactive prompt active"],
        "actions": [("press", "esc"), ("type", "Line 1: long detailed prompt context"), ("wait", 0.5)],
        "postconditions": ["Multiline text rendered inside prompt input box"]
    },
    {
        "id": "15_narrow_terminal",
        "description": "Compact terminal layout at 80x24",
        "expected_markers": ["❯"],
        "preconditions": ["Terminal window created"],
        "actions": [("resize", (80, 24)), ("wait", 0.5)],
        "postconditions": ["Layout reflowed to 80 column width"]
    },
    {
        "id": "16_wide_terminal",
        "description": "Extra wide terminal layout at 182x53",
        "expected_markers": ["❯"],
        "preconditions": ["Terminal window created"],
        "actions": [("resize", (182, 53)), ("wait", 0.5)],
        "postconditions": ["Layout expanded to 182 column width"]
    },
    {
        "id": "17_tall_terminal",
        "description": "Tall terminal layout at 156x52",
        "expected_markers": ["❯"],
        "preconditions": ["Terminal window created"],
        "actions": [("resize", (156, 52)), ("wait", 0.5)],
        "postconditions": ["Layout expanded to 52 row height"]
    },
    {
        "id": "18_escape_overlay",
        "description": "Dismissing open overlay menu via Escape key",
        "expected_markers": ["❯"],
        "preconditions": ["Overlay dialog open"],
        "actions": [("press", "esc"), ("wait", 0.5)],
        "postconditions": ["Overlay dismissed and focus returned to prompt"]
    },
    {
        "id": "19_tab_completion",
        "description": "Partial auto-completion confirm using Tab key",
        "expected_markers": ["/config"],
        "preconditions": ["Interactive prompt active"],
        "actions": [("type", "/con"), ("press", "tab"), ("wait", 0.5)],
        "postconditions": ["Tab completion accepted into prompt string"]
    },
    {
        "id": "20_command_execution",
        "description": "Executing slash command and displaying feedback message",
        "expected_markers": ["cost", "session"],
        "preconditions": ["Interactive prompt active"],
        "actions": [("press", "esc"), ("type", "/cost"), ("press", "enter"), ("wait", 0.8)],
        "postconditions": ["Command result rendered into timeline"]
    }
]

VIEWPORTS = [
    (80, 24),
    (96, 24),
    (120, 30),
    (156, 52),
    (182, 53),
]
