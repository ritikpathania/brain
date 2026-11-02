"""License notice collection, audit verification, and inventory generation."""

import argparse
import hashlib
import json
import shutil
import sys
from pathlib import Path, PurePosixPath


class LicenseError(ValueError):
    """License collection or verification failed."""


def digest(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(64 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def collect_notices(inputs: list[dict], output: Path) -> list[dict]:
    canonical_output = output.resolve()
    canonical_output.mkdir(parents=True, exist_ok=True)

    collected = []
    inventory_items = []

    for item in inputs:
        name = item.get("name")
        version = item.get("version")
        source_path_str = item.get("notice_source")
        expected_sha = item.get("notice_sha256")
        output_filename = item.get("output_filename")

        if not all([name, version, source_path_str, expected_sha, output_filename]):
            raise LicenseError(f"incomplete license specification: {item}")

        source_path = Path(source_path_str).resolve()
        if not source_path.is_file():
            raise LicenseError(f"notice source not found for {name}: {source_path}")

        actual_sha = digest(source_path)
        if actual_sha != expected_sha:
            msg = f"notice digest mismatch for {name}: {expected_sha} != {actual_sha}"
            raise LicenseError(msg)

        target_file = canonical_output / output_filename
        shutil.copy2(source_path, target_file)

        target_rel = str(PurePosixPath(f"licenses/{output_filename}"))
        collected.append({"path": target_rel, "sha256": actual_sha})
        inventory_items.append(
            {
                "name": name,
                "version": version,
                "notice_file": output_filename,
                "sha256": actual_sha,
            }
        )

    # Write notice-inventory.json
    inventory_data = {
        "schema_version": 1,
        "items": inventory_items,
    }
    inv_path = canonical_output / "notice-inventory.json"
    inv_path.write_text(json.dumps(inventory_data, indent=2) + "\n")
    inv_sha = digest(inv_path)
    collected.append({"path": "licenses/notice-inventory.json", "sha256": inv_sha})

    collected.sort(key=lambda item: item["path"])
    return collected


def main():
    parser = argparse.ArgumentParser(
        description="Collect license notices for product artifact"
    )
    parser.add_argument(
        "--licenses-json",
        required=True,
        type=Path,
        help="Path to licenses.json declaration",
    )
    parser.add_argument(
        "--output", required=True, type=Path, help="Output licenses directory"
    )
    args = parser.parse_args()

    try:
        declarations = json.loads(args.licenses_json.read_text())
        items = declarations.get("items", [])
        collect_notices(items, args.output)
        print(f"Collected {len(items)} license notices into {args.output}")
    except Exception as e:
        sys.stderr.write(f"error: {e}\n")
        sys.exit(1)


if __name__ == "__main__":
    main()
