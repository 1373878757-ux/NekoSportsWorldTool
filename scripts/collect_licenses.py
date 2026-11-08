#!/usr/bin/env python3
"""Collect license texts for the locked, target-specific normal dependency graph.

Run from any directory; --output is the root of an otherwise prepared release
staging directory. No account data, build logs, or developer paths are copied.
Missing license texts are a packaging error rather than an omitted notice.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SOURCE_ROOT = REPO_ROOT / "scripts" / "license_sources"
LICENSE_PREFIXES = ("license", "licence", "copying", "copyright", "notice", "unlicense")
LICENSE_DIRECTORIES = {"licenses", "licences", "license", "licence", "legal"}


def license_files(package: dict) -> list[Path]:
    """Find the package's distributed license/notice documents, never following out."""
    root = Path(package["manifest_path"]).resolve().parent
    found: set[Path] = set()
    declared = package.get("license_file")
    if declared:
        path = (root / declared).resolve()
        if path.is_file() and path.is_relative_to(root):
            found.add(path)
    for child in root.iterdir():
        if child.is_file() and child.name.lower().startswith(LICENSE_PREFIXES):
            resolved = child.resolve()
            if resolved.is_relative_to(root):
                found.add(resolved)
        elif child.is_dir() and child.name.lower() in LICENSE_DIRECTORIES:
            for path in child.rglob("*"):
                resolved = path.resolve()
                if path.is_file() and resolved.is_relative_to(root):
                    found.add(resolved)
    # These text files are the licenses of egui's embedded font assets.
    if package["name"] == "epaint_default_fonts":
        for path in (root / "fonts").glob("*.txt"):
            if path.is_file() and path.resolve().is_relative_to(root):
                found.add(path.resolve())
    return sorted(found)


def upstream_documents(package: dict, manifest: dict) -> list[dict]:
    """Read hash-verified, pinned upstream texts omitted from crate archives."""
    key = f"{package['name']}@{package['version']}"
    documents = manifest.get("packages", {}).get(key, [])
    for document in documents:
        source = (SOURCE_ROOT / document["file"]).resolve()
        if not source.is_relative_to(SOURCE_ROOT.resolve()) or not source.is_file():
            raise RuntimeError(f"Missing cached upstream license for {key}")
        if hashlib.sha256(source.read_bytes()).hexdigest() != document["sha256"]:
            raise RuntimeError(f"Upstream license hash mismatch for {key}")
    return documents


def selected_packages(metadata: dict) -> list[dict]:
    """Follow normal edges only after Cargo has filtered the target platform."""
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    members = set(metadata["workspace_members"])
    root = metadata["resolve"].get("root")
    if root is None:
        candidates = [key for key in members if packages[key]["name"] == "nekosportsworldtool"]
        if len(candidates) != 1:
            raise RuntimeError("Cannot identify the application package in Cargo metadata")
        root = candidates[0]
    pending = [root]
    visited: set[str] = set()
    while pending:
        package_id = pending.pop()
        if package_id in visited:
            continue
        visited.add(package_id)
        for dependency in nodes[package_id].get("deps", []):
            if any(kind.get("kind") is None for kind in dependency["dep_kinds"]):
                pending.append(dependency["pkg"])
    return sorted(
        (packages[key] for key in visited if key not in members),
        key=lambda package: (package["name"], package["version"], package["id"]),
    )


def safe_name(value: str) -> str:
    return re.sub(r"[^A-Za-z0-9._-]", "_", value)


def collect(target: str, output: Path, offline: bool) -> dict:
    command = [
        "cargo", "metadata", "--locked", "--format-version", "1",
        "--filter-platform", target,
    ]
    if offline:
        command.append("--offline")
    completed = subprocess.run(
        command, cwd=REPO_ROOT, check=True, capture_output=True, text=True,
        encoding="utf-8",
    )
    metadata = json.loads(completed.stdout)
    packages = selected_packages(metadata)
    manifest_path = SOURCE_ROOT / "manifest.json"
    source_manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    documents = [
        (package, license_files(package), upstream_documents(package, source_manifest))
        for package in packages
    ]
    missing = [
        f"{package['name']} {package['version']}"
        for package, files, upstream in documents if not files and not upstream
    ]
    if missing:
        raise RuntimeError("Missing distributed license text: " + ", ".join(missing))

    font_root = REPO_ROOT / "assets" / "fonts"
    additional = [
        (font_root / "OFL-1.1.txt", Path("NotoSansSC") / "OFL-1.1.txt"),
        (font_root / "SOURCE.md", Path("NotoSansSC") / "SOURCE.md"),
        (
            REPO_ROOT / "vendor" / "winit-0.30.13" / "NEKO-PATCH.md",
            Path("winit-0.30.13") / "NEKO-PATCH.md",
        ),
    ]
    for source, _ in additional:
        if not source.is_file():
            raise RuntimeError(f"Missing required bundled-resource attribution: {source.name}")

    output.mkdir(parents=True, exist_ok=True)
    license_root = output / "licenses"
    lines = [
        "NekoSportsWorldTool - Third-party licenses and notices",
        "",
        f"Target: {target}",
        "Scope: locked Cargo normal-dependency closure for the default GUI build.",
        "Workspace code is covered by the separately included root LICENSE.",
        "This index records license metadata; the copied documents retain their original text.",
        "",
    ]
    records = []
    for package, files, upstream in documents:
        root = Path(package["manifest_path"]).resolve().parent
        destination = license_root / safe_name(f"{package['name']}-{package['version']}")
        copied = []
        for source in files:
            relative = source.relative_to(root)
            target_path = destination / relative
            target_path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target_path)
            copied.append(target_path.relative_to(output).as_posix())
        for document in upstream:
            source = SOURCE_ROOT / document["file"]
            relative = Path(*Path(document["file"]).parts[1:])
            target_path = destination / "upstream" / relative
            target_path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target_path)
            copied.append(target_path.relative_to(output).as_posix())
        if upstream:
            origin = destination / "UPSTREAM-LICENSE-SOURCES.json"
            origin.write_text(json.dumps(upstream, indent=2) + "\n", encoding="utf-8")
            copied.append(origin.relative_to(output).as_posix())
        repository = package.get("repository") or package.get("homepage") or "Not specified"
        lines.extend([
            f"{package['name']} {package['version']}",
            f"  Declared license: {package.get('license') or 'See distributed license documents'}",
            f"  Project: {repository}",
            "  Documents: " + ", ".join(copied),
            "",
        ])
        records.append({
            "name": package["name"], "version": package["version"],
            "license": package.get("license"), "documents": copied,
        })

    for source, relative in additional:
        destination = license_root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
    lines.extend([
        "Noto Sans SC (unmodified embedded font)",
        "  Copyright 2014-2021 Adobe, with Reserved Font Name 'Source'.",
        "  License: SIL Open Font License 1.1",
        "  Documents: licenses/NotoSansSC/OFL-1.1.txt, licenses/NotoSansSC/SOURCE.md",
        "",
        "Vendored winit lifecycle patch",
        "  The winit license documents are included with its package above.",
        "  Modification attribution: licenses/winit-0.30.13/NEKO-PATCH.md",
        "",
    ])
    (output / "THIRD-PARTY-NOTICES.txt").write_text("\n".join(lines), encoding="utf-8")
    return {"target": target, "packages": len(records), "license_documents": sum(len(r["documents"]) for r in records) + len(additional)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--offline", action="store_true", help="Use Cargo's existing cache only")
    arguments = parser.parse_args()
    try:
        summary = collect(arguments.target, arguments.output.resolve(), arguments.offline)
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.CalledProcessError) as error:
        if isinstance(error, subprocess.CalledProcessError):
            print(error.stderr or str(error), file=sys.stderr)
        else:
            print(str(error), file=sys.stderr)
        return 1
    print(json.dumps(summary, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
