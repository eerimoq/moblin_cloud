"""The scene editor's requests through the device control route, against the fake Moblin."""

import uuid

import systest

from ..utils.context import Context
from ..utils.fake_twitch import user_id_of
from .remote_control import RemoteControlTestCase


def new_id() -> str:
    return str(uuid.uuid4()).upper()


class SceneEditorTestCase(RemoteControlTestCase):
    login = "editor"

    def setup(self):
        super().setup()
        if not self.moblin.is_fake():
            raise systest.TestCaseSkippedError("Only the fake Moblin has the scene editor's requests yet.")
        self.moblin.connect_to_backend(self.dashboard.moblin_cloud_key())
        self.wait_until(self.connected, "Moblin to connect")
        self.page_id = self.dashboard.page_id()
        self.device = self.dashboard.account()["devices"][0]["id"]

    def control(self, control: dict) -> dict:
        return self.dashboard.control_device(self.page_id, self.device, control)

    def scenes(self) -> list[dict]:
        return self.control({"getScenes": {}})["scenes"]

    def widgets(self) -> list[dict]:
        return self.control({"getWidgets": {}})["widgets"]


class SceneEditorReads(SceneEditorTestCase):
    """Read the scenes, widgets, video sources, media gallery and editor options Moblin answers."""

    def run(self):
        scenes = self.scenes()
        self.assert_equal([scene["name"] for scene in scenes], ["Main", "BRB", "Night"])
        self.assert_equal(scenes[0]["widgets"][0]["x"], 2.0)
        widgets = self.widgets()
        self.assert_in("Clock", [widget["name"] for widget in widgets])
        self.assert_in(
            "Front", [source["name"] for source in self.control({"getVideoSources": {}})["videoSources"]]
        )
        gallery = self.control({"getAlertsMediaGallery": {}})["gallery"]
        self.assert_equal(len(gallery["bundledSounds"]), 2)
        self.assert_true(self.control({"getAlertMedia": {"id": gallery["bundledImages"][0]["id"]}})["data"])
        self.assert_equal(self.control({"getAlertMedia": {"id": new_id()}})["data"], None)
        options = self.control({"getEditorOptions": {}})["options"]
        self.assert_equal(options["showAllSettings"], True)
        self.assert_in("fonts", options)
        video_source = next(widget for widget in widgets if widget["type"] == "Video source")
        self.assert_true(self.control({"getVideoSourceSnapshot": {"id": video_source["id"]}})["image"])
        self.assert_equal(self.control({"getVideoSourceSnapshot": {"id": widgets[0]["id"]}})["image"], None)
        languages = self.control({"getTranslationLanguages": {}})["languages"]
        self.assert_equal([language["identifier"] for language in languages], ["de", "sv"])
        self.control({"setNumericInput": {"enabled": True}})
        self.assert_equal(self.control({"getEditorOptions": {}})["options"]["numericInput"], True)
        events = self.dashboard.device_events(self.page_id, self.device)
        try:
            events.wait_for("snapshot")
            state = events.wait_for("state")["data"]
            self.assert_equal(state["canvasSize"], {"width": 1920, "height": 1080})
            self.assert_equal(state["mirrorFrontCameraOnStream"], True)
        finally:
            events.close()


class SceneEditorChanges(SceneEditorTestCase):
    """Create, change, reorder and delete scenes and widgets, with Moblin's refusals answered 409."""

    def run(self):
        events = self.dashboard.device_events(self.page_id, self.device)
        try:
            events.wait_for("snapshot")
            widget_id = new_id()
            widget = {
                "id": widget_id,
                "name": "Remote text",
                "type": "Text",
                "enabled": True,
                "text": {"formatString": "{time}"},
            }
            self.control({"createWidget": {"widget": widget}})
            self.assert_backend_error(409, lambda: self.control({"createWidget": {"widget": widget}}))
            self.assert_equal(events.wait_for("scenesChanged"), {})
            scene_id = new_id()
            scene = {
                "id": scene_id,
                "name": "Remote",
                "cameraPosition": "Back",
                "enabled": True,
                "widgets": [
                    {
                        "id": new_id(),
                        "widgetId": widget_id,
                        "x": 10.0,
                        "y": 10.0,
                        "size": 50.0,
                        "alignment": "TopLeft",
                    }
                ],
            }
            self.control({"createScene": {"scene": scene}})
            self.assert_in("Remote", [scene["name"] for scene in self.scenes()])
            self.assert_in(scene_id, [scene["id"] for scene in events.wait_for("settings")["data"]["scenes"]])
            self.control({"updateScene": {"scene": {**scene, "name": "Renamed"}}})
            self.assert_equal(
                next(scene for scene in self.scenes() if scene["id"] == scene_id)["name"], "Renamed"
            )
            self.assert_backend_error(
                409, lambda: self.control({"updateScene": {"scene": {**scene, "id": new_id()}}})
            )
            self.assert_backend_error(
                422, lambda: self.control({"updateScene": {"scene": {"name": "No id"}}})
            )
            ids = [scene["id"] for scene in self.scenes()]
            self.control({"reorderScenes": {"ids": list(reversed(ids))}})
            self.assert_equal([scene["id"] for scene in self.scenes()], list(reversed(ids)))
            self.assert_backend_error(409, lambda: self.control({"reorderScenes": {"ids": ids[:1]}}))
            self.assert_backend_error(422, lambda: self.control({"reorderScenes": {"ids": ["not-a-uuid"]}}))
            image_widget = next(widget for widget in self.widgets() if widget["type"] == "Image")
            self.assert_backend_error(
                422,
                lambda: self.control({"setWidgetImage": {"id": image_widget["id"], "image": "iVBORw0KGgo="}}),
            )
            gallery = self.control({"getAlertsMediaGallery": {}})["gallery"]
            sound = gallery["customSounds"][0]
            self.assert_true(self.control({"getAlertMedia": {"id": sound["id"]}})["data"])
            self.control({"deleteAlertMedia": {"id": sound["id"]}})
            gallery = self.control({"getAlertsMediaGallery": {}})["gallery"]
            self.assert_not_in(sound["id"], [item["id"] for item in gallery["customSounds"]])
            self.assert_backend_error(409, lambda: self.control({"deleteAlertMedia": {"id": sound["id"]}}))
            player = {"id": new_id(), "name": "Remote player"}
            self.control({"setScoreboardPlayers": {"players": [player]}})
            self.assert_equal(
                self.control({"getEditorOptions": {}})["options"]["scoreboardPlayers"], [player]
            )
            self.control({"deleteWidget": {"id": widget_id}})
            self.assert_equal(
                next(scene for scene in self.scenes() if scene["id"] == scene_id)["widgets"], []
            )
            self.control({"deleteScene": {"id": scene_id}})
            self.assert_backend_error(409, lambda: self.control({"deleteScene": {"id": scene_id}}))
            self.assert_not_in(scene_id, [scene["id"] for scene in self.scenes()])
        finally:
            events.close()


class SceneEditorNeedsControl(SceneEditorTestCase):
    """An assistant who may only view the device gets no scenes."""

    def run(self):
        assistant = self.sign_in("editor_viewer")
        self.dashboard.add_assistant("twitch", "editor_viewer")
        self.dashboard.set_assistant_remote_control("twitch", user_id_of("editor_viewer"), "view")
        self.assert_backend_error(
            403, lambda: assistant.control_device(self.page_id, self.device, {"getScenes": {}})
        )

    def teardown(self):
        self.dashboard.remove_assistant("twitch", user_id_of("editor_viewer"))


def tests(context: Context):
    return [SceneEditorReads(context), SceneEditorChanges(context), SceneEditorNeedsControl(context)]
