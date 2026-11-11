import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PORTS = ROOT / "scripts" / "moblin_ports.json"
COVERED = [
    "frontend/src/components/sceneEditor",
    "frontend/src/data/moblinSettings.ts",
    "frontend/src/data/sceneResolver.ts",
    "frontend/src/data/widgetOutlines.ts",
    "frontend/src/data/sceneEditorGeometry.ts",
    "frontend/src/components/streamSettings",
    "frontend/src/data/moblinStreamSettings.ts",
    "frontend/src/data/streamUrls.ts",
    "frontend/src/data/streamWizard.ts",
]


def git(repository: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(["git", "-C", str(repository), *args], capture_output=True, text=True, check=check)


def load() -> dict:
    return json.loads(PORTS.read_text())


def save(data: dict) -> None:
    PORTS.write_text(json.dumps(data, indent=2) + "\n")


def port_ref(data: dict, port: dict, ref: str | None) -> str:
    return ref or port.get("branch", data["branch"])


def problems(data: dict, repository: Path, given: str | None) -> list[str]:
    found = []
    mapped = {file for port in data["ports"] for file in port["files"]}
    for path in COVERED:
        candidates = [ROOT / path] if (ROOT / path).is_file() else (ROOT / path).rglob("*")
        for candidate in candidates:
            if candidate.is_file():
                relative = candidate.relative_to(ROOT).as_posix()
                if relative not in mapped:
                    found.append(f"{relative} is not in {PORTS.name}")
    for file in sorted(mapped):
        if not (ROOT / file).exists():
            found.append(f"{file} does not exist")
    for port in data["ports"]:
        ref = port_ref(data, port, given)
        for path in port["moblin"]:
            if git(repository, "cat-file", "-e", f"{ref}:{path}", check=False).returncode != 0:
                found.append(f"{path} is not in Moblin's {ref}")
        commit = port["commit"]
        ancestor = git(repository, "merge-base", "--is-ancestor", commit, ref, check=False)
        if ancestor.returncode != 0:
            found.append(f"{commit} of {port['files'][0]} is not on Moblin's {ref}")
    return found


def report(data: dict, repository: Path, given: str | None, diff: bool) -> int:
    changed = 0
    for port in data["ports"]:
        ref = port_ref(data, port, given)
        span = f"{port['commit']}..{ref}"
        log = git(repository, "log", "--oneline", span, "--", *port["moblin"]).stdout.strip()
        if not log:
            continue
        changed += 1
        print("\n".join(port["files"]))
        print("\n".join(f"  {line}" for line in log.splitlines()))
        if diff:
            print(git(repository, "diff", span, "--", *port["moblin"]).stdout)
        print()
    if changed == 0:
        print("Everything is ported.")
    return changed


def mark(data: dict, repository: Path, given: str | None, files: list[str]) -> list[str]:
    wanted = {(Path.cwd() / file).resolve().relative_to(ROOT).as_posix() for file in files}
    marked = []
    for port in data["ports"]:
        if wanted & set(port["files"]):
            ref = port_ref(data, port, given)
            port["commit"] = git(repository, "rev-parse", "--short", ref).stdout.strip()
            marked.extend(port["files"])
    return marked


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Lists the Moblin commits not yet ported to the scene editor and stream settings."
    )
    parser.add_argument("--ref", help="the Moblin branch or commit to compare with")
    parser.add_argument("--diff", action="store_true", help="also show the Swift diffs")
    parser.add_argument(
        "--mark",
        nargs="+",
        metavar="FILE",
        help="mark the ports of these files as up to date with the Moblin ref",
    )
    args = parser.parse_args()
    data = load()
    repository = (ROOT / data["repository"]).resolve()
    ref = args.ref
    if not (repository / ".git").exists():
        sys.exit(f"Moblin is not checked out at {repository}.")
    if args.mark:
        marked = mark(data, repository, ref, args.mark)
        if not marked:
            sys.exit(f"None of the files are in {PORTS.name}.")
        save(data)
        print("\n".join(sorted(set(marked))))
        return
    found = problems(data, repository, ref)
    for problem in found:
        print(problem)
    if found:
        print()
    report(data, repository, ref, args.diff)
    if found:
        sys.exit(1)


if __name__ == "__main__":
    main()
