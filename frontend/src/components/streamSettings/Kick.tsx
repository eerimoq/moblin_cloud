import { Show } from "solid-js";
import { LinkRow, PlainRow, Section, TextRow, ToggleRow } from "../sceneEditor/Form";
import type { StreamFormProps } from "./StreamsSettings";
import type { SettingsKickAlerts } from "../../data/moblinStreamSettings";
import { t } from "../../i18n";

type AlertsKey = "kickChatAlerts" | "kickToastAlerts";

function KickAlertsSettings(props: StreamFormProps & { alertsKey: AlertsKey; showBans: boolean }) {
  const alerts = () => props.stream[props.alertsKey];
  const edit = (change: (alerts: SettingsKickAlerts) => void) =>
    props.edit((draft) => change(draft[props.alertsKey]));
  const toggle = (label: string, key: keyof SettingsKickAlerts & string) => (
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
    <Section>
      {toggle(t("Subscriptions"), "subscriptions")}
      {toggle(t("Gift subscriptions"), "giftedSubscriptions")}
      {toggle(t("Rewards"), "rewards")}
      {toggle(t("Hosts"), "hosts")}
      <Show when={props.showBans}>{toggle(t("Bans and timeouts"), "bans")}</Show>
      {toggle(t("Kicks"), "kicks")}
      <TextRow
        stack={props.stack}
        label={t("Minimum kicks")}
        value={String(alerts().minimumKicks)}
        onSubmit={(value) =>
          edit((draft) => {
            draft.minimumKicks = /^[+-]?\d+$/.test(value) ? Number(value) : 0;
          })
        }
      />
    </Section>
  );
}

export function KickSettings(props: StreamFormProps) {
  const openAlerts = (title: string, alertsKey: AlertsKey, showBans: boolean) =>
    props.stack.push({
      title,
      render: () => <KickAlertsSettings {...props} alertsKey={alertsKey} showBans={showBans} />,
    });
  return (
    <>
      <Section footer={t("Log in and out on the phone.")}>
        <PlainRow>{props.stream.kickLoggedIn ? t("Logged in") : t("Not logged in")}</PlainRow>
      </Section>
      <Section>
        <TextRow
          stack={props.stack}
          label={t("Channel name")}
          value={props.stream.kickChannelName}
          onSubmit={(value) =>
            props.edit((draft) => {
              draft.kickChannelName = value;
              draft.kickChannelId = null;
              draft.kickSlug = null;
              draft.kickChatroomChannelId = null;
            })
          }
        />
      </Section>
      <Section header={t("Alerts")}>
        <LinkRow label={t("Chat")} onClick={() => openAlerts(t("Chat"), "kickChatAlerts", true)} />
        <LinkRow
          label={t("Toasts")}
          onClick={() => openAlerts(t("Toasts"), "kickToastAlerts", false)}
        />
      </Section>
    </>
  );
}
