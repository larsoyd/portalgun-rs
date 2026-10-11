#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 larsoyd
# SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

import csv
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tomllib


ROOT = Path(__file__).resolve().parents[1]
POLYFORM = "PolyForm-Noncommercial-1.0.0"
MIXED = f"{POLYFORM} AND MIT"
CANONICAL_SHA256 = "ffcca38841adb694b6f380647e15f17c446a4d1656fed51a1e2041d064c94cc8"
RIG_SHA256 = "65586d2bebc8ea310d84533df5d716711fc42886f8d01024b4b716d25d4189bb"
NOTICE = b"Required Notice: Copyright (c) 2026 larsoyd\n"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def check_texts():
    for name in ("LICENSE", "crates/portal/LICENSE"):
        digest = hashlib.sha256((ROOT / name).read_bytes()).hexdigest()
        require(digest == CANONICAL_SHA256, f"{name}: canonical PolyForm text changed")
    for name in ("NOTICE", "crates/portal/NOTICE"):
        require((ROOT / name).read_bytes() == NOTICE, f"{name}: required notice changed")
    rig = ROOT / "licenses/ELDEN-RING-Combat-Rewrite-MIT.txt"
    require(hashlib.sha256(rig.read_bytes()).hexdigest() == RIG_SHA256,
            "upstream rig MIT notice changed")
    for name in ("LICENSING.md", "THIRD_PARTY.md", "CONTRIBUTING.md"):
        require((ROOT / name).is_file(), f"missing {name}")
    require("177a3b5ec14a9bde39b9ab458a3400b4038224e3" in
            (ROOT / "THIRD_PARTY.md").read_text(), "missing upstream rig revision")


def check_headers():
    count = 0
    for directory in ("src", "tests", "crates/portal/src",
                      "crates/portal/tests", "crates/portal/examples"):
        for path in sorted((ROOT / directory).rglob("*")):
            if path.suffix not in (".rs", ".wgsl"):
                continue
            name = path.relative_to(ROOT).as_posix()
            lines = path.read_text().splitlines()
            expected = MIXED if name == "src/body.rs" else POLYFORM
            identifiers = [line for line in lines
                           if line.startswith("// SPDX-License-Identifier:")]
            require(identifiers == [f"// SPDX-License-Identifier: {expected}"],
                    f"{name}: unexpected SPDX license")
            require(any(line.startswith("// SPDX-FileCopyrightText: ") and
                        line.removeprefix("// SPDX-FileCopyrightText: ").strip()
                        for line in lines[:6]), f"{name}: missing author notice")
            if name == "src/body.rs":
                require("// Copyright (c) 2026 Funny-Bones and contributors." in lines[:8],
                        "src/body.rs: missing upstream copyright")
                require("// SPDX-FileCopyrightText: 2026 Funny-Bones and contributors"
                        in lines[:6], "src/body.rs: missing upstream SPDX copyright")
            count += 1
    require(count > 0, "no source files found")
    return count


def check_metadata():
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
    require(manifest["workspace"]["package"]["license"] == POLYFORM,
            "unexpected workspace license")
    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version", "1"],
        cwd=ROOT, check=True, capture_output=True, text=True,
    )
    metadata = json.loads(result.stdout)
    members = {p["name"]: p for p in metadata["packages"]
               if p["id"] in metadata["workspace_members"]}
    require(set(members) == {"portal", "portalgun"}, "review new workspace members")
    for name, license in (("portal", POLYFORM), ("portalgun", MIXED)):
        package = members[name]
        require(package["license"] == license, f"{name}: unexpected package license")
        require(package["license_file"] is None, f"{name}: use SPDX license metadata")
        require(package["publish"] == [], f"{name}: registry publishing must be disabled")
    return metadata


def check_inventory(metadata):
    packages = {(p["name"], p["version"]): p for p in metadata["packages"]
                if p["source"] is not None}
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
    locked = {(p["name"], p["version"]) for p in lock["package"] if "source" in p}
    with (ROOT / "docs/dependency-licenses.csv").open(newline="") as source:
        rows = list(csv.DictReader(source))
    inventory = {(r["Crate"], r["Version"]): r for r in rows}
    require(len(inventory) == len(rows), "duplicate dependency inventory entry")
    require(set(inventory) == locked == set(packages),
            "dependency inventory, locked packages and metadata differ; review the audit")
    features = {n["id"]: n["features"] for n in metadata["resolve"]["nodes"]}
    for key, row in inventory.items():
        package = packages[key]
        require(row["License"] == package["license"], f"{key}: license metadata changed")
        require(row["Registry source"] == package["source"], f"{key}: source changed")
        require(sorted(filter(None, row["Enabled features"].split(","))) ==
                sorted(features[package["id"]]), f"{key}: features changed; review the audit")
        for field in ("Compatibility assessment", "Obligations", "Recommended action",
                      "Selected license", "Graph status"):
            require(bool(row[field].strip()), f"{key}: missing {field}")
        require("/home/" not in row["License evidence"], f"{key}: machine-specific evidence")
    return len(rows)


def main():
    check_texts()
    sources = check_headers()
    dependencies = check_inventory(check_metadata())
    print(f"License checks passed: 2 packages, {sources} source files, "
          f"{dependencies} dependencies. Distribution gates remain in LICENSING.md.")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError, subprocess.CalledProcessError) as error:
        print(f"License check failed: {error}", file=sys.stderr)
        if isinstance(error, subprocess.CalledProcessError):
            print(error.stderr, file=sys.stderr)
        sys.exit(1)
