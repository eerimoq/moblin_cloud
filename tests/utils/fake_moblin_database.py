"""The settings the fake Moblin keeps, shaped like the parts of Moblin's Database it uses."""


def database(ids: dict[str, str], icon: str) -> dict:
    return {
        "streams": [
            {"id": ids["S1"], "name": "Twitch", "enabled": True},
            {"id": ids["S2"], "name": "Kick", "enabled": False},
        ],
        "scenes": [
            {
                "id": ids["C1"],
                "name": "Main",
                "enabled": True,
                "cameraPosition": "Back",
                "backCameraId": "com.apple.avfoundation.avcapturedevice.built-in_video:0",
                "widgets": [
                    {
                        "id": ids["SW1"],
                        "widgetId": ids["W1"],
                        "x": 2,
                        "y": 2,
                        "size": 100,
                        "alignment": "TopLeft",
                        "positioningLock": False,
                    },
                    {
                        "id": ids["SW2"],
                        "widgetId": ids["W2"],
                        "x": 1.3,
                        "y": 31,
                        "size": 30,
                        "alignment": "TopRight",
                        "positioningLock": False,
                    },
                ],
            },
            {
                "id": ids["C2"],
                "name": "BRB",
                "enabled": True,
                "cameraPosition": "None",
                "widgets": [
                    {
                        "id": ids["SW3"],
                        "widgetId": ids["W3"],
                        "x": 0,
                        "y": 0,
                        "size": 100,
                        "alignment": "Center",
                        "positioningLock": False,
                    }
                ],
            },
            {"id": ids["C3"], "name": "Night", "enabled": False, "cameraPosition": "Front", "widgets": []},
        ],
        "widgets": [
            {
                "id": ids["W1"],
                "name": "Clock",
                "type": "Text",
                "enabled": True,
                "text": {"formatString": "{shortTime}", "fontSize": 30},
            },
            {
                "id": ids["W2"],
                "name": "Alerts",
                "type": "Browser",
                "enabled": True,
                "browser": {"url": "https://example.com/alerts", "width": 500, "height": 500},
            },
            {"id": ids["W3"], "name": "BRB image", "type": "Image", "enabled": True},
        ],
        "videoSources": [
            {
                "id": "com.apple.avfoundation.avcapturedevice.built-in_video:0",
                "name": "Back Wide",
                "position": "Back",
            },
            {
                "id": "com.apple.avfoundation.avcapturedevice.built-in_video:1",
                "name": "Front Wide",
                "position": "Front",
            },
            {
                "id": "00000000-CAFE-BABE-BEEF-000000000000",
                "name": "Screen capture",
                "position": "Screen capture",
            },
            {"id": "00000000-FEED-B1AC-CAFE-000000000000", "name": "None", "position": "None"},
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
