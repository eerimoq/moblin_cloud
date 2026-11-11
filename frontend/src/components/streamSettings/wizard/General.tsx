import { Show } from "solid-js";
import { rowClass, Toggle } from "../../MoblinParts";
import { ButtonRow, Section, ToggleRow } from "../../sceneEditor/Form";
import { ExternalLink, Field, type WizardPageProps } from "./shared";
import { t, tRich } from "../../../i18n";

export function General(props: WizardPageProps) {
  const wizard = props.wizard;
  return (
    <>
      <Field
        header={t("Stream name")}
        value={wizard.state.name}
        placeholder={t("Name")}
        onInput={(name) => wizard.set("name", name)}
      />
      <Show
        when={wizard.state.platform === "mobcam"}
        fallback={
          <Section footer={t("Live stream and record when the app is in background mode.")}>
            <ToggleRow
              label={t("Background streaming")}
              on={wizard.state.backgroundStreaming}
              onToggle={(on) => wizard.set("backgroundStreaming", on)}
            />
          </Section>
        }
      >
        <Section footer={t("Automatically go live when the app enters foreground.")}>
          <ToggleRow
            label={t("Auto go live")}
            on={wizard.state.autoGoLive}
            onToggle={(on) => wizard.set("autoGoLive", on)}
          />
        </Section>
      </Show>
      <Section>
        <div class={`${rowClass} justify-between`}>
          <span class="min-w-0 flex-1">
            {tRich("Send Go live notification to {link}", {
              link: (
                <ExternalLink href="https://moblin.app/#streamers">
                  {t("Moblin website")}
                </ExternalLink>
              ),
            })}
          </span>
          <Toggle
            label={t("Send Go live notification to Moblin website")}
            on={wizard.state.goLiveNotificationMoblinWebsite}
            disabled={false}
            onToggle={(on) => wizard.set("goLiveNotificationMoblinWebsite", on)}
          />
        </div>
      </Section>
      <Section>
        <ButtonRow
          label={t("Create")}
          disabled={wizard.state.name === ""}
          onClick={() => wizard.create()}
        />
      </Section>
    </>
  );
}
