import time

from ..utils.context import Context
from ..utils.devices import RawDevice
from ..utils.events import EventStream
from ..utils.fake_twitch import user_id_of
from ..utils.frontend import Dashboard
from ..utils.test_case import TestCase


class EventsTestCase(TestCase):
    """The change streams the dashboard and the page refetch on, which only name what changed."""

    login = "eventsbase"

    def setup(self):
        self.skip_if_real_twitch_sign_in()
        self.dashboard = self.sign_in(self.login)
        self.page_id = self.dashboard.page_id()

    def page_events(self) -> EventStream:
        return EventStream(self.frontend, f"/pages/{self.page_id}/events")

    def account_events(self, dashboard: Dashboard | None = None, page_id: str | None = None) -> EventStream:
        return EventStream(dashboard or self.dashboard, f"/account/pages/{page_id or self.page_id}/events")

    def assert_event(self, events: EventStream, name: str):
        self.assert_in(name, self.wait_names(events), f"a {name} event")

    def wait_names(self, events: EventStream, timeout: float = 5) -> list[str]:
        deadline = time.monotonic() + timeout
        names: list[str] = []
        while time.monotonic() < deadline:
            try:
                names.append(events.next(0.5)[0])
            except TimeoutError:
                if names:
                    return names
        return names


class EventsPage(EventsTestCase):
    """Tell the public page when anything it shows changes."""

    login = "eventspage"

    def run(self):
        self.assert_backend_error(404, lambda: EventStream(self.frontend, "/pages/AAAAAAAAAAAAAAAA/events"))
        with self.page_events() as events:
            self.assert_equal(events.drain(), [], "nothing changed yet")
            self.dashboard.change_settings(message="Changed")
            self.assert_event(events, "page")
            device = RawDevice(self.backend.port, self.dashboard.moblin_cloud_key(), "EV1")
            self.assert_event(events, "page")
            device.push_wheel(["Dance"])
            self.assert_event(events, "page")
            self.dashboard.rename_device("EV1", "Renamed")
            self.assert_equal(
                [name for name in events.drain(1) if name == "page"], [], "names are not on the page"
            )
            device.close()
            self.assert_event(events, "page")
            self.dashboard.change_settings(message=None)
            self.assert_event(events, "page")


class EventsPageIcon(EventsTestCase):
    """Tell the public page when Moblin's icon, which the page shows, changes."""

    login = "eventspageicon"

    def run(self):
        device = RawDevice(self.backend.port, self.dashboard.moblin_cloud_key(), "EI1")
        viewer = self.frontend.viewer(self.page_id)
        with self.page_events() as events:
            events.drain(1)
            device.event({"icon": {"id": "AppIconKing"}})
            self.wait_until(lambda: viewer.profile()["icon"] == "king", "the icon to be saved", timeout=5)
            self.assert_event(events, "page")
        device.close()


class EventsAccount(EventsTestCase):
    """Tell the streamer's and the assistants' dashboards about account changes and new received items."""

    login = "eventsaccount"

    def run(self):
        assistant = self.sign_in("eventsmod")
        self.assert_backend_error(403, lambda: self.account_events(assistant))
        self.assert_backend_error(401, lambda: self.account_events(Dashboard(self.backend.url, None)))
        with self.account_events() as events, self.account_events(assistant, assistant.page_id()) as own:
            self.dashboard.add_assistant("twitch", "eventsmod")
            self.assert_event(events, "account")
            self.assert_event(own, "account")
            with self.account_events(assistant) as assisted:
                device = RawDevice(self.backend.port, self.dashboard.moblin_cloud_key(), "EA1")
                self.dashboard.rename_device("EA1", "Named")
                self.assert_event(events, "account")
                self.assert_event(assisted, "account")
                self.dashboard.test_tip("Counted")
                self.wait_until(lambda: bool(device.tips()), "the test tip")
                self.dashboard.set_assistant_preview("twitch", user_id_of("eventsmod"), True)
                self.assert_event(assisted, "account")
                device.close()
            self.dashboard.remove_assistant("twitch", user_id_of("eventsmod"))
            self.assert_event(own, "account")
        self.assert_backend_error(403, lambda: self.account_events(assistant))


class EventsRecent(EventsTestCase):
    """Tell the dashboard to refetch the received lists when an item is captured, resent or delivered."""

    login = "eventsrecent"

    def run(self):
        settings = self.context.page_with_paypal(self.dashboard)
        self.dashboard.change_settings(tips={"amounts": [100]})
        viewer = self.frontend.viewer(self.page_id)
        with self.account_events() as events:
            order_id = viewer.order_tip(100, "Eventful")
            self.context.paypal.pay_with_card(order_id, settings["merchantId"])
            viewer.capture_tip(order_id)
            self.assert_event(events, "recent")
            device = RawDevice(self.backend.port, self.dashboard.moblin_cloud_key(), "ER1")
            self.wait_until(lambda: bool(device.tips()), "the tip")
            self.assert_event(events, "recent")
            self.dashboard.resend(order_id)
            self.assert_event(events, "recent")
            device.close()
        self.dashboard.change_settings(tips=None)


class EventsAdminStats(EventsTestCase):
    """Push the operator's statistics to an admin at once and every two seconds, and to nobody else."""

    login = "eventsnotadmin"

    def run(self):
        self.assert_backend_error(403, lambda: EventStream(self.dashboard, "/admin/stats/events"))
        self.assert_backend_error(401, lambda: EventStream(self.frontend, "/admin/stats/events"))
        admin = self.sign_in("admin")
        with EventStream(admin, "/admin/stats/events") as events:
            started = time.monotonic()
            name, stats = events.next(2)
            self.assert_equal(name, "stats")
            assert isinstance(stats, dict)
            self.assert_equal(sorted(stats), sorted(admin.admin_stats()))
            events.wait_for("stats", 4)
            events.wait_for("stats", 4)
            took = time.monotonic() - started
            self.assert_true(3 <= took <= 7, f"two more after {took:.1f} seconds")


class EventsEndOnShutdown(EventsTestCase):
    """End every open stream when the backend shuts down, so a restart is not held up by them."""

    login = "eventsshutdown"

    def run(self):
        device = RawDevice(self.backend.port, self.dashboard.moblin_cloud_key(), "ES1")
        device.close()
        streams = [
            self.page_events(),
            self.account_events(),
            EventStream(self.dashboard, f"/account/pages/{self.page_id}/devices/ES1/events"),
            EventStream(self.sign_in("admin"), "/admin/stats/events"),
        ]
        started = time.monotonic()
        self.backend.restart()
        took = time.monotonic() - started
        for stream in streams:
            self.assert_true(stream.wait_for_end(5), "the stream ended")
            stream.close()
        self.assert_true(took < 10, f"the restart took {took:.1f} seconds")


def tests(context: Context):
    return [
        EventsPage(context),
        EventsPageIcon(context),
        EventsAccount(context),
        EventsRecent(context),
        EventsAdminStats(context),
        EventsEndOnShutdown(context),
    ]
