from ..utils.context import Context
from ..utils.fake_twitch import user_id_of
from ..utils.test_case import TestCase

LAYOUT = {"version": 1, "panes": [{"panel": "chat", "size": 40}, {"panel": "status", "device": "PHONE"}]}


class MonitorLayout(TestCase):
    """Keep the Monitor's layout for each user and page, which the web Monitor and mobtop share, starting a page the user has no layout for from their latest one."""

    def run(self):
        self.skip_if_real_twitch_sign_in()
        streamer = self.sign_in("monitorstreamer")
        assistant = self.sign_in("monitormod")
        page_id = streamer.page_id()
        streamer.add_assistant("twitch", "monitormod")
        self.assert_is_none(streamer.monitor_layout(page_id))
        streamer.save_monitor_layout(page_id, LAYOUT)
        self.assert_equal(streamer.monitor_layout(page_id), LAYOUT)
        self.assert_is_none(assistant.monitor_layout(page_id))
        own = {"version": 1, "panes": [{"panel": "recent"}]}
        assistant.save_monitor_layout(page_id, own)
        self.assert_equal(assistant.monitor_layout(page_id), own)
        self.assert_equal(assistant.monitor_layout(assistant.page_id()), own)
        changed = {"version": 1, "panes": []}
        assistant.save_monitor_layout(assistant.page_id(), changed)
        self.assert_equal(
            [assistant.monitor_layout(assistant.page_id()), assistant.monitor_layout(page_id)], [changed, own]
        )
        self.assert_equal(streamer.monitor_layout(page_id), LAYOUT)
        for invalid in [[], "panes", 1, None]:
            self.assert_backend_error(
                422,
                lambda invalid=invalid: streamer.save_monitor_layout(page_id, invalid),
                "the layout must be an object",
            )
        self.assert_refused(lambda: streamer.save_monitor_layout(page_id, {"panes": ["x" * 20_000]}))
        self.backend.restart()
        self.assert_equal(
            [streamer.monitor_layout(page_id), assistant.monitor_layout(page_id)], [LAYOUT, own]
        )
        streamer.remove_assistant("twitch", user_id_of("monitormod"))
        self.assert_backend_error(403, lambda: assistant.monitor_layout(page_id))
        self.assert_backend_error(403, lambda: assistant.save_monitor_layout(page_id, own))


def tests(context: Context):
    return [MonitorLayout(context)]
