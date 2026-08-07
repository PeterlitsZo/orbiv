#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VERSION_RE = re.compile(
    r"^\d+\.\d+\.\d+(?:-[0-9A-Za-z](?:[0-9A-Za-z.-]*[0-9A-Za-z])?)?"
    r"(?:\+[0-9A-Za-z](?:[0-9A-Za-z.-]*[0-9A-Za-z])?)?$"
)


def run(args: list[str]) -> None:
    print(f"$ {' '.join(args)}")
    subprocess.run(args, cwd=ROOT, check=True)


def output(args: list[str]) -> str:
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def replace_workspace_version(version: str) -> None:
    manifest = ROOT / "Cargo.toml"
    lines = manifest.read_text().splitlines()

    section_start: int | None = None
    section_end = len(lines)
    for index, line in enumerate(lines):
        if line.strip() == "[workspace.package]":
            section_start = index
            continue

        if section_start is not None and index > section_start:
            stripped = line.strip()
            if stripped.startswith("[") and stripped.endswith("]"):
                section_end = index
                break

    if section_start is None:
        if lines and lines[-1] != "":
            lines.append("")
        lines.extend(["[workspace.package]", f'version = "{version}"'])
    else:
        for index in range(section_start + 1, section_end):
            if re.match(r"\s*version\s*=", lines[index]):
                lines[index] = f'version = "{version}"'
                break
        else:
            lines.insert(section_start + 1, f'version = "{version}"')

    manifest.write_text("\n".join(lines) + "\n")


def workspace_metadata() -> dict:
    return json.loads(
        output(["cargo", "metadata", "--no-deps", "--format-version", "1"])
    )


def update_inline_dependency_versions(
    manifest: Path, internal_package_names: set[str], version: str
) -> None:
    lines = manifest.read_text().splitlines()
    changed = False

    for index, line in enumerate(lines):
        for package_name in sorted(internal_package_names, key=len, reverse=True):
            pattern = re.compile(
                rf'^(\s*{re.escape(package_name)}\s*=\s*\{{)([^}}\n]*\bpath\s*=\s*"[^"]+"[^}}\n]*)(\}}\s*)$'
            )
            match = pattern.match(line)
            if not match:
                continue

            prefix, body, suffix = match.groups()
            if re.search(r'\bversion\s*=\s*"[^"]*"', body):
                body = re.sub(
                    r'\bversion\s*=\s*"[^"]*"', f'version = "{version}"', body
                )
            else:
                body = f'{body.rstrip()}, version = "{version}"'

            lines[index] = f"{prefix}{body}{suffix}"
            changed = True
            break

    if changed:
        manifest.write_text("\n".join(lines) + "\n")


def update_workspace_versions(version: str) -> None:
    replace_workspace_version(version)

    metadata = workspace_metadata()
    member_ids = set(metadata["workspace_members"])
    packages = [
        package for package in metadata["packages"] if package["id"] in member_ids
    ]
    internal_package_names = {package["name"] for package in packages}

    for package in packages:
        update_inline_dependency_versions(
            Path(package["manifest_path"]),
            internal_package_names,
            version,
        )


def push_current_branch() -> None:
    try:
        output(["git", "rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
    except subprocess.CalledProcessError:
        run(["git", "push", "origin", "HEAD"])
    else:
        run(["git", "push"])


def release(version: str) -> None:
    if not VERSION_RE.fullmatch(version):
        raise SystemExit(f"Invalid version: {version}")

    tag = f"v{version}"

    update_workspace_versions(version)
    run(["git", "add", "."])
    run(["git", "commit", "-m", f"chore: Release '{version}'."])
    run(["git", "tag", tag])
    push_current_branch()
    run(["git", "push", "origin", tag])
    run(["cargo", "publish", "--workspace"])


def main() -> None:
    parser = argparse.ArgumentParser(description="Release a new Orbit version.")
    parser.add_argument("version", help="Next version, for example 0.2.0.")
    args = parser.parse_args()

    try:
        release(args.version)
    except subprocess.CalledProcessError as error:
        sys.exit(error.returncode)


if __name__ == "__main__":
    main()
