import { LinkRow, MenuRow, PlainRow, Section, TextRow, ToggleRow } from "../sceneEditor/Form";
import type { StreamFormProps } from "./StreamsSettings";
import type { SettingsTwitchAlerts } from "../../data/moblinStreamSettings";
import { t } from "../../i18n";

type AlertsKey = "twitchChatAlerts" | "twitchToastAlerts";

const minimumWatchStreaks = [1, 3, 5, 10, 15, 20, 25];

function TwitchAlertsSettings(props: StreamFormProps & { alertsKey: AlertsKey }) {
  const alerts = () => props.stream[props.alertsKey];
  const edit = (change: (alerts: SettingsTwitchAlerts) => void) =>
    props.edit((draft) => change(draft[props.alertsKey]));
  const toggle = (label: string, key: keyof SettingsTwitchAlerts & string) => (
    <ToggleRow
      label={label}
      on={alerts()[key] === true}
      onToggle={(on) =>
        edit((draft) => {
          (draft as Record<string, unknown>)[key] = on;
        })
      }
    />
  );
  return (
    <>
      <Section>
        {toggle(t("Follows"), "follows")}
        {toggle(t("Subscriptions"), "subscriptions")}
        {toggle(t("Gift subscriptions"), "giftSubscriptions")}
        {toggle(t("Resubscriptions"), "resubscriptions")}
        {toggle(t("Rewards"), "rewards")}
        {toggle(t("Raids"), "raids")}
        {toggle(t("Bits"), "cheers")}
        <TextRow
          stack={props.stack}
          label={t("Minimum bits")}
          value={String(alerts().minimumCheerBits)}
          onSubmit={(value) =>
            edit((draft) => {
              draft.minimumCheerBits = /^[+-]?\d+$/.test(value) ? Number(value) : 0;
            })
          }
        />
        {toggle(t("Watch streaks"), "watchStreaks")}
        <MenuRow
          label={t("Minimum watch streak")}
          items={minimumWatchStreaks.map((streak) => ({
            id: String(streak),
            name: String(streak),
          }))}
          selected={String(alerts().minimumWatchStreak)}
          onPick={(id) =>
            edit((draft) => {
              draft.minimumWatchStreak = Number(id);
            })
          }
        />
      </Section>
      <Section footer={t("Also show events from other channels in a shared chat session.")}>
        {toggle(t("Shared chat"), "sharedChat")}
      </Section>
    </>
  );
}

export function TwitchSettings(props: StreamFormProps) {
  const openAlerts = (title: string, alertsKey: AlertsKey) =>
    props.stack.push({
      title,
      render: () => <TwitchAlertsSettings {...props} alertsKey={alertsKey} />,
    });
  return (
    <>
      <Section footer={t("Log in and out on the phone.")}>
        <PlainRow>{props.stream.twitchLoggedIn ? t("Logged in") : t("Not logged in")}</PlainRow>
      </Section>
      <Section footer={t("The name of your channel.")}>
        <TextRow
          stack={props.stack}
          label={t("Channel name")}
          value={props.stream.twitchChannelName}
          onSubmit={(value) =>
            props.edit((draft) => {
              draft.twitchChannelName = value;
            })
          }
        />
      </Section>
      <Section>
        <TextRow
          stack={props.stack}
          label={t("Channel id")}
          value={props.stream.twitchChannelId}
          onSubmit={(value) =>
            props.edit((draft) => {
              draft.twitchChannelId = value;
            })
          }
        />
      </Section>
      <Section header={t("Alerts")}>
        <LinkRow label={t("Chat")} onClick={() => openAlerts(t("Chat"), "twitchChatAlerts")} />
        <LinkRow label={t("Toasts")} onClick={() => openAlerts(t("Toasts"), "twitchToastAlerts")} />
      </Section>
    </>
  );
}
