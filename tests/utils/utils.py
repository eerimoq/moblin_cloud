from collections.abc import Callable
from pathlib import Path

import systest

TEST_DIR = Path(__file__).parent.parent.resolve()
REPO_DIR = TEST_DIR.parent
FILES_DIR = TEST_DIR / "files"


def wait_until(check: Callable[[], bool], description: str, timeout: float = 60):
    """Like systest's wait_until, but checks 20 times a second instead of twice."""
    systest.wait_until(check, description, timeout, interval=0.05)
