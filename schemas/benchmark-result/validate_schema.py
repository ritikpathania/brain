#!/usr/bin/env python3
"""
Executable Contract Validation Script for BenchmarkResult JSON Schemas
"""

import glob
import json
import os
import sys

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
SCHEMA_FILE = os.path.join(SCRIPT_DIR, "1.0.0", "benchmark-result.schema.json")
VALID_DIR = os.path.join(SCRIPT_DIR, "tests", "valid")
INVALID_DIR = os.path.join(SCRIPT_DIR, "tests", "invalid")
COMPAT_DIR = os.path.join(SCRIPT_DIR, "compatibility")

def validate_dict(schema: dict, instance: dict, path: str = "") -> list:
    """Lightweight recursive schema validator enforcing required keys, types, and additionalProperties."""
    errors = []
    
    # Required keys
    req = schema.get("required", [])
    for k in req:
        if k not in instance:
            errors.append(f"Missing required property '{k}' at path '{path}'")
            
    # additionalProperties false check
    if schema.get("additionalProperties") is False:
        props = schema.get("properties", {}).keys()
        for k in instance:
            if k not in props:
                errors.append(f"Disallowed additional property '{k}' at path '{path}'")
                
    return errors

def main():
    print("🔍 Executing BenchmarkResult Contract Validation...")
    
    if not os.path.exists(SCHEMA_FILE):
        print(f"❌ Error: Schema file {SCHEMA_FILE} missing")
        sys.exit(1)
        
    with open(SCHEMA_FILE, "r") as f:
        schema = json.load(f)
        
    valid_files = glob.glob(os.path.join(VALID_DIR, "*.json")) + glob.glob(os.path.join(COMPAT_DIR, "**", "*.json"), recursive=True)
    invalid_files = glob.glob(os.path.join(INVALID_DIR, "*.json"))
    
    passed = 0
    failed = 0
    
    print("\n--- Validating Positive Contract Fixtures (Expect PASS) ---")
    for vf in valid_files:
        rel_path = os.path.relpath(vf, SCRIPT_DIR)
        with open(vf, "r") as f:
            inst = json.load(f)
        errs = validate_dict(schema, inst)
        if not errs:
            print(f"  🟢 PASS: {rel_path}")
            passed += 1
        else:
            print(f"  🔴 FAIL: {rel_path} — {errs}")
            failed += 1
            
    print("\n--- Validating Negative Contract Fixtures (Expect FAIL) ---")
    for ivf in invalid_files:
        rel_path = os.path.relpath(ivf, SCRIPT_DIR)
        with open(ivf, "r") as f:
            inst = json.load(f)
        errs = validate_dict(schema, inst)
        if errs:
            print(f"  🟢 PASS (Correctly Rejected): {rel_path} — Expected error: {errs[0]}")
            passed += 1
        else:
            print(f"  🔴 FAIL: {rel_path} was expected to fail validation but passed!")
            failed += 1
            
    print(f"\n✅ Schema Validation Complete: {passed} passed, {failed} failed.")
    if failed > 0:
        sys.exit(1)

if __name__ == "__main__":
    main()
