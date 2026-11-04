import { Show } from "solid-js";
import { NumberRow, PickerRow, Section, SliderRow, TextRow, ToggleRow } from "../../Form";
import { EffectsSection } from "./effects";
import { validUrl, type WidgetFormProps } from "./shared";
import { type BrowserMode } from "../../../../data/moblinSettings";
import { t } from "../../../../i18n";

export function BrowserForm(props: WidgetFormProps) {
  const browser = () => props.widget.browser;
  const modes: { id: BrowserMode; name: string }[] = [
    { id: "periodicAudioAndVideo", name: t("Periodic, audio and video") },
    { id: "audioAndVideoOnly", name: t("Audio and video only") },
    { id: "audioOnly", name: t("Audio only") },
  ];
  return (
    <>
      <Section>
        <TextRow
          stack={props.stack}
          label={t("URL")}
          value={browser().url}
          validate={validUrl}
          onSubmit={(url) =>
            props.edit((draft) => {
              draft.browser.url = url;
            })
          }
        />
        <TextRow
          stack={props.stack}
          label={t("Style sheet")}
          value={browser().styleSheet}
          placeholder="body {}"
          multiline
          footers={[
            t("For example:"),
            "",
            "body {background-color: powderblue;}",
            "h1 {color: blue;}",
            "p {color: red;}",
          ]}
          onSubmit={(styleSheet) =>
            props.edit((draft) => {
              draft.browser.styleSheet = styleSheet;
            })
          }
        />
        <NumberRow
          stack={props.stack}
          label={t("Width")}
          value={browser().width}
          minimum={1}
          maximum={3999}
          integer
          onSubmit={(width) =>
            props.edit((draft) => {
              draft.browser.width = width;
            })
          }
        />
        <NumberRow
          stack={props.stack}
          label={t("Height")}
          value={browser().height}
          minimum={1}
          maximum={3999}
          integer
          onSubmit={(height) =>
            props.edit((draft) => {
              draft.browser.height = height;
            })
          }
        />
      </Section>
      <Section
        footer={
          browser().localOnly
            ? t(
                "Tap the Interactive browser widgets quick button on the phone to hide and show browsers.",
              )
            : undefined
        }
      >
        <ToggleRow
          label={t("Local only")}
          on={browser().localOnly}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.browser.localOnly = on;
            })
          }
        />
      </Section>
      <Show when={!browser().localOnly}>
        <Section
          footer={
            <span class="flex flex-col">
              <span>
                {t(
                  'When "Audio and video only" mode is selected, images, text, GIFs etc. will only be shown when a video (.mp4/.mov) is playing, reducing overall energy consumption.',
                )}
              </span>
              <span>&nbsp;</span>
              <span>
                {t(
                  'When "Audio only" mode is selected, no video will be rendered at all. Only audio will play.',
                )}
              </span>
            </span>
          }
        >
          <PickerRow
            stack={props.stack}
            label={t("Mode")}
            items={modes}
            selected={browser().mode}
            onPick={(id) =>
              props.edit((draft) => {
                draft.browser.mode = id as BrowserMode;
              })
            }
          />
          <Show when={browser().mode === "periodicAudioAndVideo"}>
            <SliderRow
              label={t("Base FPS")}
              value={browser().fps}
              minimum={1}
              maximum={15}
              step={1}
              format={(value) => value.toFixed(1)}
              onChange={(fps) =>
                props.edit((draft) => {
                  draft.browser.fps = fps;
                })
              }
            />
          </Show>
        </Section>
      </Show>
      <Section
        footer={t(
          "Give the webpage access to various data in Moblin, for example chat messages and your location.",
        )}
      >
        <ToggleRow
          label={t("Moblin access")}
          on={browser().moblinAccess}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.browser.moblinAccess = on;
            })
          }
        />
        <Show when={browser().moblinAccess}>
          <ToggleRow
            label={t("Speech to text")}
            on={browser().speechToText}
            onToggle={(on) =>
              props.edit((draft) => {
                draft.browser.speechToText = on;
              })
            }
          />
        </Show>
      </Section>
      <EffectsSection {...props} />
    </>
  );
}
