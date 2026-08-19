#!/usr/bin/env python3
import os

SRC_DIR = "/Users/ritikpathania/Developer/src"
VENDOR_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell/vendor/claude"

def get_rel_files(base_dir):
    files = set()
    for root, dirs, filenames in os.walk(base_dir):
        dirs[:] = [d for d in dirs if d not in (".git", ".DS_Store", ".code-review-graph", "node_modules")]
        for fn in filenames:
            if fn in (".DS_Store",):
                continue
            full_path = os.path.join(root, fn)
            rel_path = os.path.relpath(full_path, base_dir)
            files.add(rel_path)
    return files

src_files = get_rel_files(SRC_DIR)
vendor_files = get_rel_files(VENDOR_DIR)

print("Files only in Developer/src:")
for f in sorted(src_files - vendor_files):
    print("  *", f)
