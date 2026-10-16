from logging import Logger
from pathlib import Path

TEST_DIR = Path(__file__).parent.parent.resolve()
REPO_DIR = TEST_DIR.parent
FILES_DIR = TEST_DIR / "files"
CACHE_DIR = TEST_DIR / "cache"


def manual_validation(logger: Logger, message: str):
    logger.info("🧪: %s", message)


def manual_requirement(logger: Logger, message: str):
    logger.info("👷‍♂️: %s", message)


def manual_confirmation(message: str):
    input(f"🧑‍🔧: {message} Press ENTER to continue.")
