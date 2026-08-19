#!/usr/bin/env python3
import os

SRC_DIR = "/Users/ritikpathania/Developer/src"
VENDOR_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell/vendor/claude"

src_count = sum(len(files) for _, _, files in os.walk(SRC_DIR))
vendor_count = sum(len(files) for _, _, files in os.walk(VENDOR_DIR))

print(f"SRC_DIR total raw files: {src_count}")
print(f"VENDOR_DIR total raw files: {vendor_count}")
