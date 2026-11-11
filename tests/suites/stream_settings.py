"""The stream settings' requests through the device control route, against the fake Moblin."""

import uuid
from typing import Callable

from ..utils.context import Context
from ..utils.fake_twitch import user_id_of
from .scene_editor import SceneEditorTestCase


def new_id() -> str:
    return str(uuid.uuid4()).upper()


class StreamSettingsTestCase(SceneEditorTestCase):
    login = "streamsettings"

    def settings(self) -> dict:
        events = self.dashboard.device_events(self.page_id, self.device)
        try:
            snapshot = events.wait_for("snapshot")
            return snapshot["settings"] or events.wait_for("settings")["data"]
        finally:
            events.close()

    def streams(self) -> list[dict]:
        return self.settings()["streams"]

    def stream(self, stream_id: str) -> dict:
        return self.control({"getStream": {"id": stream_id}})["stream"]


class StreamSettingsChanges(StreamSettingsTestCase):
    """Create, change, select and delete streams, with Moblin's refusals answered 409."""

    def run(self):
        self.assert_equal([stream["name"] for stream in self.streams()], ["Twitch", "Kick"])
        self.stream_id = new_id()
        stream = {"id": self.stream_id, "name": "Remote", "url": "srt://remote.example:4000"}
        self.control({"createStream": {"stream": stream}})
        self.assert_equal(self.stream(self.stream_id)["enabled"], False)
        self.assert_backend_error(
            409,
            lambda: self.control({"createStream": {"stream": stream}}),
            "Moblin refused the change. Reload the streams and try again.",
        )
        self.assert_backend_error(
            409, lambda: self.control({"updateStream": {"stream": {**stream, "name": "Kick"}}})
        )
        self.control({"updateStream": {"stream": {**stream, "name": "Renamed", "enabled": True}}})
        self.assert_equal(
            [self.stream(self.stream_id)["name"], self.stream(self.stream_id)["enabled"]],
            ["Renamed", False],
        )
        self.control({"setLive": {"on": True}})
        self.assert_backend_error(
            409,
            lambda: self.control(
                {"updateStream": {"stream": {**stream, "name": "Renamed", "url": "srt://other.example:4000"}}}
            ),
        )
        self.control({"setLive": {"on": False}})
        self.control({"setStream": {"id": self.stream_id}})
        self.assert_true(self.stream(self.stream_id)["enabled"])
        self.assert_backend_error(409, lambda: self.control({"deleteStream": {"id": self.stream_id}}))
        twitch = self.streams()[0]["id"]
        self.control({"setStream": {"id": twitch}})
        self.control({"deleteStream": {"id": self.stream_id}})
        self.assert_equal([stream["name"] for stream in self.streams()], ["Twitch", "Kick"])
        self.assert_backend_error(422, lambda: self.control({"updateStream": {"stream": {"name": "No id"}}}))
        self.assert_backend_error(409, lambda: self.control({"getStream": {"id": self.stream_id}}))

    def teardown(self):
        self.control({"setLive": {"on": False}})
        self.control({"setStream": {"id": self.streams()[0]["id"]}})
        if any(stream["id"] == getattr(self, "stream_id", None) for stream in self.streams()):
            self.control({"deleteStream": {"id": self.stream_id}})
        self.control({"setLive": {"on": True}})


class StreamSettingsCurrentStream(StreamSettingsTestCase):
    """Moblin's settings name the stream that is in use, also after another one is picked."""

    def run(self):
        events = self.dashboard.device_events(self.page_id, self.device)
        try:
            settings = events.wait_for("settings")["data"]
            self.assert_equal(
                settings["currentStreamId"],
                next(stream["id"] for stream in settings["streams"] if stream["name"] == "Twitch"),
            )
            self.control({"setLive": {"on": False}})
            kick = next(stream["id"] for stream in settings["streams"] if stream["name"] == "Kick")
            self.control({"setStream": {"id": kick}})
            while events.wait_for("settings")["data"]["currentStreamId"] != kick:
                pass
        finally:
            events.close()

    def teardown(self):
        self.control({"setLive": {"on": False}})
        self.control({"setStream": {"id": self.streams()[0]["id"]}})
        self.control({"setLive": {"on": True}})


class StreamSettingsSecrets(StreamSettingsTestCase):
    """Assistants only see and change the streams' secrets when the streamer lets them."""

    def run(self):
        assistant = self.sign_in("secrets_mod")
        self.dashboard.add_assistant("twitch", "secrets_mod")
        assistant_id = user_id_of("secrets_mod")
        self.dashboard.set_assistant_remote_control("twitch", assistant_id, "control")
        self.assert_equal(assistant.assisted_account(self.page_id)["secrets"], False)

        kick_id = self.streams()[1]["id"]

        def kick_stream() -> dict:
            return assistant.control_device(self.page_id, self.device, {"getStream": {"id": kick_id}})[
                "stream"
            ]

        kick = kick_stream()
        self.assert_equal(kick["url"], "rtmps://")
        assistant.control_device(
            self.page_id,
            self.device,
            {"updateStream": {"stream": {**kick, "name": "Kick", "url": "rtmp://attacker.example/app/key"}}},
        )
        self.assert_equal(self.stream(kick_id)["url"], "rtmps://kick.example/app/key")
        self.created_id = new_id()
        assistant.control_device(
            self.page_id,
            self.device,
            {
                "createStream": {
                    "stream": {"id": self.created_id, "name": "Mod", "url": "srt://attacker.example:1"}
                }
            },
        )
        self.assert_equal(self.stream(self.created_id)["url"], "srt://my_public_ip:4000")

        def alerts(control: Callable[[dict], dict]) -> dict:
            widgets = control({"getWidgets": {}})["widgets"]
            return next(widget for widget in widgets if widget["type"] == "Alerts")

        def as_assistant(control: dict) -> dict:
            return assistant.control_device(self.page_id, self.device, control)

        widget = alerts(as_assistant)
        self.assert_equal(widget["alerts"]["ai"], {})
        widget["alerts"]["ai"] = {"baseUrl": "https://attacker.example/v1", "apiKey": "stolen"}
        as_assistant({"updateWidget": {"widget": widget}})
        self.assert_equal(
            alerts(self.control)["alerts"]["ai"],
            {"baseUrl": "https://ai.example/v1", "apiKey": "fake-ai-key"},
        )
        account = self.dashboard.set_assistant_secrets("twitch", assistant_id, True)
        self.assert_equal([entry["secrets"] for entry in account["twitchAssistants"]], [True])
        self.assert_equal(assistant.assisted_account(self.page_id)["secrets"], True)
        self.assert_equal(kick_stream()["url"], "rtmps://kick.example/app/key")
        self.dashboard.set_assistant_remote_control("twitch", assistant_id, "view")
        self.assert_equal(kick_stream()["url"], "rtmps://kick.example/app/key")
        self.assert_backend_error(
            403, lambda: as_assistant({"updateStream": {"stream": {**kick_stream(), "fps": 60}}})
        )

    def teardown(self):
        self.dashboard.remove_assistant("twitch", user_id_of("secrets_mod"))
        if any(stream["id"] == getattr(self, "created_id", None) for stream in self.streams()):
            self.control({"deleteStream": {"id": self.created_id}})


def tests(context: Context):
    return [
        StreamSettingsChanges(context),
        StreamSettingsCurrentStream(context),
        StreamSettingsSecrets(context),
    ]
