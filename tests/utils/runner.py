import argparse
import logging
import shutil
import textwrap
from collections.abc import Callable
from pathlib import Path

import systest

from .backend import Backend
from .backend import build_backend
from .config import Config
from .context import Context
from .moblin import Moblin
from .utils import FILES_DIR

MakeTests = Callable[[Context], list]


class HelpFormatter(argparse.HelpFormatter):
    def _split_lines(self, text: str, width: int) -> list[str]:
        lines = []
        for line in text.splitlines():
            indent = " " * (len(line) - len(line.lstrip()) + 2)
            lines += textwrap.wrap(line, width, subsequent_indent=indent) or [""]
        return lines


def create_parser(description: str) -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=description, formatter_class=HelpFormatter)
    parser.add_argument(
        "--device",
        help="A real Moblin device in the configuration to test with, instead of the fake one in tests/utils/fake_moblin.py.",
    )
    parser.add_argument(
        "--interactive",
        action="store_true",
        help="Run all tests that requires user interaction, and show the browser.",
    )
    return parser


def _remove_previous_run_artifacts(name: str):
    shutil.rmtree(FILES_DIR, ignore_errors=True)
    FILES_DIR.mkdir(parents=True)
    (Path("logs") / f"{name}.log").unlink(missing_ok=True)


def run(name: str, parser: argparse.ArgumentParser, make_tests: MakeTests):
    _remove_previous_run_artifacts(name)
    sequencer = systest.setup(name, parser, add_date_to_log_filename=False)
    sequencer.remove_filtered_testcases = True
    sequencer.compact_output = True
    args = parser.parse_args()
    logging.getLogger("urllib3.connectionpool").setLevel(logging.INFO)
    logging.getLogger("websockets").setLevel(logging.WARNING)
    config = Config()
    build_backend()
    with Backend(config) as backend:
        with Moblin(config, backend, args.device) as moblin:
            moblin.disconnect_from_backend()
            context = Context(config, backend, moblin, args.interactive)
            run_tests(sequencer, context, make_tests)
    sequencer.report_and_exit(json=False, dot=False)


def run_tests(sequencer: systest.Sequencer, context: Context, make_tests: MakeTests):
    try:
        sequencer.run(*make_tests(context))
    finally:
        context.close()
