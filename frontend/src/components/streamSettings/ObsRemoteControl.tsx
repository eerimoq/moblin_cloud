import { Show } from "solid-js";
import { Secret, Section, TextRow, ToggleRow } from "../sceneEditor/Form";
import type { StreamFormProps } from "./StreamsSettings";
import { cleanUrl, isValidWebSocketUrl } from "../../data/streamUrls";
import { t } from "../../i18n";

export function ObsRemoteControlSettings(props: StreamFormProps) {
  const showAllSettings = () => props.streams.state.options?.showAllSettings === true;
  return (
    <>
      <Section header={t("WebSocket")}>
        <Secret allowed={props.remote.secrets} label={t("URL")}>
          <TextRow
            stack={props.stack}
            label={t("URL")}
            value={props.stream.obsWebSocketUrl}
            footers={[t("For example ws://232.32.45.332:4567.")]}
            validate={(value) => isValidWebSocketUrl(value) ?? isValidWebSocketUrl(cleanUrl(value))}
            onSubmit={(value) =>
              props.edit((draft) => {
                draft.obsWebSocketUrl = cleanUrl(value);
              })
            }
          />
        </Secret>
        <Secret allowed={props.remote.secrets} label={t("Password")}>
          <TextRow
            stack={props.stack}
            label={t("Password")}
            value={props.stream.obsWebSocketPassword}
            masked
            onSubmit={(value) =>
              props.edit((draft) => {
                draft.obsWebSocketPassword = value;
              })
            }
          />
        </Secret>
      </Section>
      <Section
        footer={t(
          "Moblin will periodically try to switch to the BRB scene if the stream is likely broken, and back to the main scene once everything seems to work again.",
        )}
      >
        <TextRow
          stack={props.stack}
          label={t("Main scene")}
          value={props.stream.obsMainScene}
          onSubmit={(value) =>
            props.edit((draft) => {
              draft.obsMainScene = value;
            })
          }
        />
        <TextRow
          stack={props.stack}
          label={t("BRB scene")}
          value={props.stream.obsBrbScene}
          onSubmit={(value) =>
            props.edit((draft) => {
              draft.obsBrbScene = value;
            })
          }
        />
        <ToggleRow
          label={t("Streaming directly to OBS")}
          on={props.stream.streamingDirectlyToObs}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.streamingDirectlyToObs = on;
            })
          }
        />
      </Section>
      <Show when={showAllSettings()}>
        <Section
          footer={t(
            "Moblin will switch to the BRB scene configured above when the current scene's SRT(LA) or RTMP video source is disconnected. Typically enable when using Moblin as SRT(LA) server at home, streaming to OBS on the same computer.",
          )}
        >
          <ToggleRow
            label={t("BRB scene when video source is broken")}
            on={props.stream.obsBrbSceneVideoSourceBroken}
            disabled={props.stream.obsBrbScene === ""}
            onToggle={(on) =>
              props.edit((draft) => {
                draft.obsBrbSceneVideoSourceBroken = on;
              })
            }
          />
        </Section>
      </Show>
      <Section footer={t("The name of the Source in OBS that receives the stream from Moblin.")}>
        <TextRow
          stack={props.stack}
          label={t("Source name")}
          value={props.stream.obsSourceName}
          onSubmit={(value) =>
            props.edit((draft) => {
              draft.obsSourceName = value;
            })
          }
        />
      </Section>
      <Show when={showAllSettings()}>
        <Section>
          <ToggleRow
            label={t("Auto start streaming when going live")}
            on={props.stream.obsAutoStartStream}
            onToggle={(on) =>
              props.edit((draft) => {
                draft.obsAutoStartStream = on;
              })
            }
          />
          <ToggleRow
            label={t("Auto stop streaming when ending stream")}
            on={props.stream.obsAutoStopStream}
            onToggle={(on) =>
              props.edit((draft) => {
                draft.obsAutoStopStream = on;
              })
            }
          />
        </Section>
        <Section>
          <ToggleRow
            label={t("Auto start recording when going live")}
            on={props.stream.obsAutoStartRecording}
            onToggle={(on) =>
              props.edit((draft) => {
                draft.obsAutoStartRecording = on;
              })
            }
          />
          <ToggleRow
            label={t("Auto stop recording when ending stream")}
            on={props.stream.obsAutoStopRecording}
            onToggle={(on) =>
              props.edit((draft) => {
                draft.obsAutoStopRecording = on;
              })
            }
          />
        </Section>
      </Show>
    </>
  );
}
