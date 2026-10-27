"""The settings the fake Moblin keeps, shaped like the parts of Moblin's Database it uses."""


def database(ids: dict[str, str], icon: str) -> dict:
    return {
        "streams": [
            {"id": ids["S1"], "name": "Twitch", "enabled": True},
            {"id": ids["S2"], "name": "Kick", "enabled": False},
        ],
        "scenes": [
            {"id": ids["C1"], "name": "Main", "enabled": True},
            {"id": ids["C2"], "name": "BRB", "enabled": True},
            {"id": ids["C3"], "name": "Night", "enabled": False},
        ],
        "zoom": {"back": [{"id": ids["Z1"], "name": "1x", "x": 1}, {"id": ids["Z2"], "name": "2x", "x": 2}]},
        "iconImage": icon,
        "macros": {
            "macros": [
                {"id": ids["M1"], "name": "Intro"},
                {"id": ids["M2"], "name": "Scene loop"},
            ]
        },
        "autoSceneSwitchers": {"switcherId": None, "switchers": [{"id": ids["A1"], "name": "Evening"}]},
    }
