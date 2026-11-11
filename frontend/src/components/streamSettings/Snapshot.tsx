import { Secret, Section, TextRow, ToggleRow } from "../sceneEditor/Form";
import type { StreamFormProps } from "./StreamsSettings";
import { cleanUrl } from "../../data/streamUrls";
import { t } from "../../i18n";

export function SnapshotSettings(props: StreamFormProps) {
  return (
    <>
      <Section footer={t("Do not show widgets in snapshots.")}>
        <ToggleRow
          label={t("Clean snapshots")}
          on={props.stream.recording.cleanSnapshots}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.recording.cleanSnapshots = on;
            })
          }
        />
      </Section>
      <Section
        header={t("Discord")}
        footer={
          <span class="flex flex-col">
            <span>
              {t(
                "Automatically upload quick button snapshots and chat bot snapshots to channels in your Discord server.",
              )}
            </span>
            <span>&nbsp;</span>
            <span>
              {t("Create a webhook in your Discord server's settings and paste it's URL above.")}
            </span>
          </span>
        }
      >
        <Secret allowed={props.remote.secrets} label={t("Webhook URL")}>
          <TextRow
            stack={props.stack}
            label={t("Webhook URL")}
            value={props.stream.discordSnapshotWebhook}
            placeholder="https://discord.com/api/webhooks/foobar"
            masked
            onSubmit={(value) =>
              props.edit((draft) => {
                draft.discordSnapshotWebhook = cleanUrl(value);
              })
            }
          />
        </Secret>
        <Secret allowed={props.remote.secrets} label={t("Chat bot webhook URL")}>
          <TextRow
            stack={props.stack}
            label={t("Chat bot webhook URL")}
            value={props.stream.discordChatBotSnapshotWebhook}
            placeholder="https://discord.com/api/webhooks/foobar"
            masked
            onSubmit={(value) =>
              props.edit((draft) => {
                draft.discordChatBotSnapshotWebhook = cleanUrl(value);
              })
            }
          />
        </Secret>
        <ToggleRow
          label={t("Only when live")}
          on={props.stream.discordSnapshotWebhookOnlyWhenLive}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.discordSnapshotWebhookOnlyWhenLive = on;
            })
          }
        />
      </Section>
    </>
  );
}
