#!/usr/bin/env python3
import os
import sys
import json
import shutil
import platform

def detect_capabilities():
    term = os.environ.get("TERM", "")
    colorterm = os.environ.get("COLORTERM", "")
    term_prog = os.environ.get("TERM_PROGRAM", "")
    no_color = os.environ.get("NO_COLOR", "")
    vte_version = os.environ.get("VTE_VERSION", "")

    # Color Profile Detection
    if no_color or term == "dumb":
        color_profile = "NO_COLOR"
    elif colorterm in ("truecolor", "24bit") or term_prog in ("iTerm.app", "WezTerm", "Ghostty", "Alacritty", "Apple_Terminal"):
        color_profile = "Truecolor"
    elif "256color" in term:
        color_profile = "256-color"
    else:
        color_profile = "16-color"

    # Unicode Support
    unicode_support = term != "dumb" and "ASCII" not in term.upper()

    # OSC 8 Hyperlink Support
    osc8_supported = False
    if term_prog in ("iTerm.app", "WezTerm", "Ghostty", "vscode") or (vte_version and int(vte_version) >= 5000):
        osc8_supported = True

    # Clipboard Provider
    if shutil.which("pbcopy"):
        clipboard_provider = "pbcopy (macOS)"
    elif shutil.which("wl-copy"):
        clipboard_provider = "wl-clipboard (Wayland)"
    elif shutil.which("xclip"):
        clipboard_provider = "xclip (X11)"
    else:
        clipboard_provider = "In-Memory Fallback"

    return {
        "schema_version": "1.0",
        "timestamp": os.popen("date -u +'%Y-%m-%dT%H:%M:%SZ'").read().strip(),
        "environment": {
            "terminal": term_prog or "Unknown",
            "term": term,
            "colorterm": colorterm,
            "os": platform.system(),
            "architecture": platform.machine()
        },
        "capabilities": {
            "color_profile": {"value": color_profile, "level": "Required"},
            "unicode_support": {"value": unicode_support, "level": "Required"},
            "keyboard_routing": {"value": True, "level": "Required"},
            "osc8_hyperlinks": {"value": osc8_supported, "level": "Optional"},
            "clipboard_provider": {"value": clipboard_provider, "level": "Optional"}
        }
    }

def main():
    caps = detect_capabilities()
    out_file = sys.argv[1] if len(sys.argv) > 1 else "target/terminal_capabilities.json"
    os.makedirs(os.path.dirname(os.path.abspath(out_file)), exist_ok=True)
    with open(out_file, "w") as f:
        json.dump(caps, f, indent=2)
    print(json.dumps(caps, indent=2))

if __name__ == "__main__":
    main()
