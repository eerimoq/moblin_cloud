"""The settings the fake Moblin keeps, shaped like the parts of Moblin's Database it uses."""

SCREEN_CAPTURE_ID = "00000000-CAFE-BABE-BEEF-000000000000"
NO_VIDEO_SOURCE_ID = "00000000-FEED-B1AC-CAFE-000000000000"


def scene_widget(scene_widget_id: str, widget_id: str, **layout) -> dict:
    return {
        "id": scene_widget_id,
        "widgetId": widget_id,
        "x": 0.0,
        "y": 0.0,
        "size": 100.0,
        "alignment": "TopLeft",
        "positioningLock": False,
        **layout,
    }


def widget(widget_id: str, name: str, kind: str, **settings) -> dict:
    return {"id": widget_id, "name": name, "type": kind, "enabled": True, "effects": [], **settings}


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
                "backCameraId": "back",
                "widgets": [
                    scene_widget(ids["SW1"], ids["W1"], x=2.0, y=2.0),
                    scene_widget(ids["SW2"], ids["W2"], x=2.0, y=2.0, size=30.0, alignment="TopRight"),
                    scene_widget(ids["SW3"], ids["W4"], size=28.0, alignment="BottomRight"),
                    scene_widget(ids["SW4"], ids["W5"], x=20.0, y=5.0),
                ],
            },
            {
                "id": ids["C2"],
                "name": "BRB",
                "enabled": True,
                "cameraPosition": "None",
                "widgets": [scene_widget(ids["SW5"], ids["W3"], x=10.0, y=10.0, size=50.0)],
            },
            {
                "id": ids["C3"],
                "name": "Night",
                "enabled": False,
                "cameraPosition": "Front",
                "frontCameraId": "front",
                "widgets": [
                    scene_widget(ids["SW6"], ids["W6"]),
                    scene_widget(ids["SW7"], ids["W1"], x=5.0, y=5.0, alignment="BottomLeft"),
                ],
            },
        ],
        "widgets": [
            widget(
                ids["W1"],
                "Clock",
                "Text",
                text={"formatString": "{shortTime}", "fontSize": 30, "horizontalAlignment": "Leading"},
            ),
            widget(ids["W2"], "Logo", "Image"),
            widget(
                ids["W3"],
                "Overlay",
                "Browser",
                browser={"url": "https://example.com", "width": 500, "height": 500},
            ),
            widget(
                ids["W4"],
                "Front camera",
                "Video source",
                videoSource={"cameraPosition": "Front", "frontCameraId": "front"},
            ),
            widget(ids["W5"], "Alerts", "Alerts", alerts={"twitch": {}, "kick": {}}),
            widget(ids["W6"], "BRB inside", "Scene", scene={"sceneId": ids["C2"]}),
        ],
        "alertsMediaGallery": {
            "bundledImages": [
                {"id": ids["G1"], "name": "Moblin pixels"},
                {"id": ids["G2"], "name": "White star"},
            ],
            "customImages": [{"id": ids["G5"], "name": "My GIF"}],
            "bundledSounds": [{"id": ids["G3"], "name": "Notification"}, {"id": ids["G4"], "name": "Fart"}],
            "customSounds": [{"id": ids["G6"], "name": "My sound"}],
        },
        "scoreboardPlayers": [],
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


def video_sources() -> list[dict]:
    return [
        {"id": "back", "name": "Back", "position": "Back"},
        {"id": "front", "name": "Front", "position": "Front"},
        {"id": SCREEN_CAPTURE_ID, "name": "Screen capture", "position": "Screen capture"},
        {"id": NO_VIDEO_SOURCE_ID, "name": "None", "position": "None"},
    ]


def editor_options(ids: dict[str, str], settings: dict) -> dict:
    return {
        "fonts": [
            {"family": "Helvetica", "styles": ["Helvetica", "Helvetica-Bold"]},
            {"family": "Courier", "styles": ["Courier", "Courier-Bold"]},
        ],
        "luts": [{"id": ids["L1"], "name": "Apple Log To Rec 709"}],
        "scoreboardPlayers": settings["scoreboardPlayers"],
        "webRemote": {
            "enabled": True,
            "port": 8080,
            "deviceName": "Fake Moblin",
            "addresses": ["fake-moblin.local"],
        },
        "workoutDevices": [{"id": ids["H1"], "name": "Heart rate strap"}],
        "streamDeckLayouts": [],
        "appleVoices": [
            {
                "language": "en",
                "voices": [
                    {
                        "name": "Samantha",
                        "identifier": "com.apple.voice.compact.en-US.Samantha",
                        "language": "en-US",
                    }
                ],
            }
        ],
        "showAllSettings": True,
        "isPhone": True,
        "forceSceneSwitchTransition": False,
        "workoutActive": False,
        "inactiveVideoSourceSceneIds": [],
        "canvasSize": {"width": 1920, "height": 1080},
    }
