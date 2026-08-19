#!/usr/bin/env python3
import os
import shutil

VENDOR_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell/vendor/claude"
SRC_DIR = "/Users/ritikpathania/Developer/src"

shims = [
    "entrypoints/sdk/controlTypes.ts",
    "entrypoints/sdk/coreTypes.generated.ts",
    "entrypoints/sdk/runtimeTypes.ts",
    "entrypoints/sdk/sdkUtilityTypes.ts",
    "entrypoints/sdk/settingsTypes.generated.ts",
    "entrypoints/sdk/toolTypes.ts",
    "ink/devtools.ts",
    "ink/global.d.ts",
    "tools/TungstenTool/TungstenLiveMonitor.tsx",
    "tools/TungstenTool/TungstenTool.ts",
    "tools/WorkflowTool/constants.ts",
    "types/connectorText.ts",
    "utils/permissions/yolo-classifier-prompts/auto_mode_system_prompt.txt",
    "utils/permissions/yolo-classifier-prompts/permissions_anthropic.txt",
    "utils/permissions/yolo-classifier-prompts/permissions_external.txt",
    "utils/ultraplan/prompt.txt",
]

for rel in shims:
    src_target = os.path.join(SRC_DIR, rel)
    vendor_src = os.path.join(VENDOR_DIR, rel)
    if os.path.exists(vendor_src):
        os.makedirs(os.path.dirname(src_target), exist_ok=True)
        shutil.copy2(vendor_src, src_target)
        print(f"Copied {rel} -> {src_target}")

print("All shims synced to Developer/src.")
