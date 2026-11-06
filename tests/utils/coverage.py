import shutil
import subprocess
from pathlib import Path

from .backend import BACKEND_DIR
from .backend import COVERAGE_BINARY
from .backend import COVERAGE_DIR


def _output(command: list[str]) -> str:
    return subprocess.run(command, capture_output=True, text=True, check=True).stdout.strip()


def _llvm_tool(name: str) -> str:
    for bundled in Path(_output(["rustc", "--print", "sysroot"])).glob(f"lib/rustlib/*/bin/{name}"):
        return str(bundled)
    found = shutil.which(name)
    if found is not None:
        return found
    if shutil.which("brew") is not None:
        brewed = Path(_output(["brew", "--prefix", "llvm"])) / "bin" / name
        if brewed.exists():
            return str(brewed)
    raise Exception(
        f"{name} not found. Install it with 'rustup component add llvm-tools' or 'brew install llvm'."
    )


class Coverage:
    def __init__(self):
        self._profdata = _llvm_tool("llvm-profdata")
        self._cov = _llvm_tool("llvm-cov")

    def report(self):
        profiles = [str(path) for path in COVERAGE_DIR.glob("*.profraw")]
        merged = COVERAGE_DIR / "backend.profdata"
        subprocess.run([self._profdata, "merge", "-sparse", *profiles, "-o", str(merged)], check=True)
        arguments = [str(COVERAGE_BINARY), f"-instr-profile={merged}", str(BACKEND_DIR / "src")]
        html_dir = COVERAGE_DIR / "html"
        subprocess.run(
            [self._cov, "show", "-format=html", f"-output-dir={html_dir}", *arguments],
            check=True,
        )
        subprocess.run([self._cov, "report", "-show-branch-summary=false", *arguments], check=True)
        print(f"\nLine by line in {html_dir / 'index.html'}\n")
