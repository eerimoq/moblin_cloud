import { createSignal } from "solid-js";
import { LinkRow, Secret, Section, TextRow, useReadOnly } from "../sceneEditor/Form";
import { Toggle, rowClass } from "../MoblinParts";
import type { StreamFormProps } from "./StreamsSettings";
import { links } from "../../data/links";
import { cleanUrl } from "../../data/streamUrls";
import { t } from "../../i18n";

function GoLiveNotificationDiscordSettings(props: StreamFormProps) {
  const readOnly = useReadOnly();
  const [message, setMessage] = createSignal<string | null>(null);
  return (
    <>
      <Section
        header={t("Message")}
        footer={
          <span class="flex flex-col">
            <span>
              {t(
                "Markdown works. Add emojis as <:myEmojiName:8912739817498174>. Send \\:myEmojiName: in Discord to get it.",
              )}
            </span>
            <span>&nbsp;</span>
            <span>{t("A snapshot will also be uploaded.")}</span>
          </span>
        }
      >
        <textarea
          class="min-h-[110px] w-full resize-none bg-transparent px-4 py-2 text-[17px] outline-none placeholder:text-phone-border"
          value={message() ?? props.stream.goLiveNotificationDiscordMessage}
          placeholder={t("My text")}
          readOnly={readOnly()}
          autocapitalize="off"
          autocorrect="off"
          onInput={(event) => setMessage(event.currentTarget.value)}
          onBlur={() => {
            const typed = message();
            if (typed !== null) {
              props.edit((draft) => {
                draft.goLiveNotificationDiscordMessage = typed;
              });
              setMessage(null);
            }
          }}
        />
      </Section>
      <Section>
        <Secret allowed={props.remote.secrets} label={t("Webhook URL")}>
          <TextRow
            stack={props.stack}
            label={t("Webhook URL")}
            value={props.stream.goLiveNotificationDiscordWebhookUrl}
            placeholder="https://discord.com/api/webhooks/foobar"
            masked
            onSubmit={(value) =>
              props.edit((draft) => {
                draft.goLiveNotificationDiscordWebhookUrl = cleanUrl(value);
              })
            }
          />
        </Secret>
      </Section>
    </>
  );
}

export function GoLiveNotificationSettings(props: StreamFormProps) {
  return (
    <>
      <Section>
        <LinkRow
          label={t("Discord")}
          onClick={() =>
            props.stack.push({
              title: t("Discord"),
              render: () => <GoLiveNotificationDiscordSettings {...props} />,
            })
          }
        />
      </Section>
      <Section>
        <div class={`${rowClass} justify-between`}>
          <a
            class="min-w-0 flex-1 truncate text-phone-tint"
            href={`${links.website}/#streamers`}
            target="_blank"
            rel="noreferrer"
          >
            {t("Moblin website")}
          </a>
          <Toggle
            label={t("Moblin website")}
            on={props.stream.goLiveNotificationMoblinWebsite}
            disabled={false}
            onToggle={(on) =>
              props.edit((draft) => {
                draft.goLiveNotificationMoblinWebsite = on;
              })
            }
          />
        </div>
      </Section>
    </>
  );
}
