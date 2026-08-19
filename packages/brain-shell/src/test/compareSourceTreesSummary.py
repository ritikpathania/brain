#!/usr/bin/env python3
import os
import hashlib

SRC_DIR = "/Users/ritikpathania/Developer/src"
VENDOR_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell/vendor/claude"

def hash_file(filepath):
    h = hashlib.sha256()
    with open(filepath, "rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest()

def get_all_files(base_dir):
    files = {}
    for root, dirs, filenames in os.walk(base_dir):
        dirs[:] = [d for d in dirs if d not in (".git", ".DS_Store", ".code-review-graph", "node_modules")]
        for fn in filenames:
            if fn in (".DS_Store",):
                continue
            full_path = os.path.join(root, fn)
            rel_path = os.path.relpath(full_path, base_dir)
            files[rel_path] = {
                "size": os.path.getsize(full_path),
                "hash": hash_file(full_path)
            }
    return files

def main():
    src_files = get_all_files(SRC_DIR)
    vendor_files = get_all_files(VENDOR_DIR)

    all_rel_paths = sorted(set(src_files.keys()) | set(vendor_files.keys()))

    identical = []
    modified = []
    only_in_src = []
    only_in_vendor = []

    for rel in all_rel_paths:
        in_src = rel in src_files
        in_vendor = rel in vendor_files

        if in_src and in_vendor:
            if src_files[rel]["hash"] == vendor_files[rel]["hash"]:
                identical.append(rel)
            else:
                modified.append((rel, src_files[rel], vendor_files[rel]))
        elif in_src:
            only_in_src.append(rel)
        else:
            only_in_vendor.append(rel)

    print("=========================================================================")
    print("           SOURCE CODE TREE DIFFERENTIAL SUMMARY REPORT                  ")
    print("=========================================================================")
    print(f"Total files in Developer/src:                {len(src_files)}")
    print(f"Total files in packages/brain-shell/vendor:  {len(vendor_files)}")
    print(f"Identical files (100% byte-for-byte match):  {len(identical)}")
    print(f"Modified files:                              {len(modified)}")
    print(f"Only in Developer/src:                       {len(only_in_src)}")
    print(f"Only in vendor/claude (shims/txt files):     {len(only_in_vendor)}")
    print("-------------------------------------------------------------------------")

    print("\n[MODIFIED FILES]:")
    for rel, s, v in modified:
        print(f"  * {rel} (src: {s['size']} bytes, vendor: {v['size']} bytes)")

    print("\n[ADDED COMPATIBILITY / SHIM FILES IN VENDOR]:")
    for rel in only_in_vendor:
        print(f"  * {rel}")

    print("=========================================================================")

if __name__ == "__main__":
    main()
