import argparse
import json

import pyperclip

from .utils.config import Config
from .utils.moblin import settings_for


def main():
    parser = argparse.ArgumentParser(
        description="Generate Moblin settings that connect its remote control streamer to the tester."
    )
    parser.add_argument("--force-stdout", action="store_true")
    args = parser.parse_args()
    config = Config()
    url = f"ws://{config.tester_ip_address()}:{config.remote_control_port()}"
    settings = json.dumps(settings_for(url, None), indent=4)
    if args.force_stdout:
        print(settings)
    else:
        pyperclip.copy(settings)
        print("Settings copied to clipboard.")


main()
