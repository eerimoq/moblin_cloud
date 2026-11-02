import { For, Show, type JSX } from "solid-js";
import Icon from "../../../Icon";
import {
  ButtonRow,
  ColorRow,
  LinkRow,
  MenuRow,
  PlainRow,
  ReorderableList,
  Section,
  SliderRow,
  TextRow,
  ToggleRow,
  ValueRow,
} from "../../Form";
import { fontDesignItems, fontWeightItems, validUrl, type WidgetFormProps } from "./shared";
import { PositioningRectangle } from "./PositioningRectangle";
import type { Editor } from "../../store";
import { asset } from "../../../../asset";
import {
  defaultAlert,
  defaultCheerBits,
  defaultKickGifts,
  defaultVoice,
  newUuid,
  sameId,
  type FontDesign,
  type FontWeight,
  type MediaGalleryItem,
  type SettingsAlert,
  type SettingsVoice,
  type SettingsWidget,
} from "../../../../data/moblinSettings";
import { language, t } from "../../../../i18n";

type AlertPath = (widget: SettingsWidget) => SettingsAlert | undefined;

type AlertProps = WidgetFormProps & { alert: AlertPath };

const editAlert = (props: AlertProps, change: (alert: SettingsAlert) => void) =>
  props.edit((draft) => {
    const alert = props.alert(draft);
    if (alert) {
      change(alert);
    }
  });

const gallery = (editor: Editor) => editor.state.gallery;

const mediaName = (items: MediaGalleryItem[], id: string) =>
  items.find((item) => sameId(item.id, id))?.name ?? "";

const fixAlertMedias = (editor: Editor) => {
  const current = gallery(editor);
  const firstImage = current?.bundledImages[0];
  const firstSound = current?.bundledSounds[0];
  if (!current || !firstImage || !firstSound) {
    return;
  }
  const images = [...current.bundledImages, ...current.customImages];
  const sounds = [...current.bundledSounds, ...current.customSounds];
  const missingImage = (alert: SettingsAlert) =>
    !images.some((image) => sameId(image.id, alert.imageId));
  const missingSound = (alert: SettingsAlert) =>
    !sounds.some((sound) => sameId(sound.id, alert.soundId));
  const alertsOf = (widget: SettingsWidget) => [
    widget.alerts.twitch.follows,
    widget.alerts.twitch.subscriptions,
    ...widget.alerts.chatBot.commands.map((command) => command.alert),
  ];
  const ids = editor.state.widgets
    .filter((widget) =>
      alertsOf(widget).some((alert) => missingImage(alert) || missingSound(alert)),
    )
    .map((widget) => widget.id);
  for (const id of ids) {
    editor.editWidget(id, (draft) => {
      for (const alert of alertsOf(draft)) {
        if (missingImage(alert)) {
          alert.imageId = firstImage.id;
        }
        if (missingSound(alert)) {
          alert.soundId = firstSound.id;
        }
      }
    });
  }
};

function MediaGalleryPage(props: AlertProps & { kind: "images" | "sounds" }) {
  const items = () =>
    gallery(props.editor)?.[props.kind === "images" ? "customImages" : "customSounds"] ?? [];
  return (
    <Section
      footer={
        props.kind === "images"
          ? t("Add and delete images on the phone.")
          : t("Add and delete sounds on the phone.")
      }
    >
      <For each={items()}>
        {(item) => (
          <div class="flex min-h-[44px] items-center px-4 text-[17px]">
            <span class="min-w-0 flex-1 truncate">{item.name}</span>
          </div>
        )}
      </For>
    </Section>
  );
}

function ImageSelectorPage(props: AlertProps) {
  const alert = () => props.alert(props.widget)!;
  const images = () => {
    const current = gallery(props.editor);
    return current ? [...current.bundledImages, ...current.customImages] : [];
  };
  return (
    <>
      <Section>
        <For each={images()}>
          {(image) => (
            <button
              type="button"
              class="flex min-h-[54px] w-full items-center gap-3 px-4 py-2 text-left text-[17px]"
              onClick={() =>
                editAlert(props, (draft) => {
                  draft.imageId = image.id;
                })
              }
            >
              <span class="min-w-0 flex-1 truncate">{image.name}</span>
              <Show when={sameId(alert().imageId, image.id)}>
                <Icon name="check" class="size-[18px] text-phone-tint" />
              </Show>
            </button>
          )}
        </For>
      </Section>
      <Section footer={t("Number of times the GIF will be played each alert.")}>
        <SliderRow
          label={t("Repeat")}
          value={alert().imageLoopCount}
          minimum={1}
          maximum={10}
          step={1}
          format={(value) => String(value)}
          onChange={(value) =>
            editAlert(props, (draft) => {
              draft.imageLoopCount = value;
            })
          }
        />
      </Section>
      <Section>
        <LinkRow
          label={t("My images")}
          onClick={() =>
            props.stack.push({
              title: t("My images"),
              render: () => <MediaGalleryPage {...props} kind="images" />,
            })
          }
        />
      </Section>
    </>
  );
}

function SoundSelectorPage(props: AlertProps) {
  const alert = () => props.alert(props.widget)!;
  const sounds = () => {
    const current = gallery(props.editor);
    return current ? [...current.bundledSounds, ...current.customSounds] : [];
  };
  return (
    <>
      <Section>
        <For each={sounds()}>
          {(sound) => (
            <div class="flex min-h-[44px] items-center gap-3 px-4 py-2 text-[17px]">
              <button
                type="button"
                class="flex min-w-0 flex-1 items-center gap-3 text-left"
                onClick={() =>
                  editAlert(props, (draft) => {
                    draft.soundId = sound.id;
                  })
                }
              >
                <span class="min-w-0 flex-1 truncate">{sound.name}</span>
                <Show when={sameId(alert().soundId, sound.id)}>
                  <Icon name="check" class="size-[18px] text-phone-tint" />
                </Show>
              </button>
            </div>
          )}
        </For>
      </Section>
      <Section>
        <LinkRow
          label={t("My sounds")}
          onClick={() =>
            props.stack.push({
              title: t("My sounds"),
              render: () => <MediaGalleryPage {...props} kind="sounds" />,
            })
          }
        />
      </Section>
    </>
  );
}

function AlertMedia(props: AlertProps) {
  const alert = () => props.alert(props.widget)!;
  const current = () => gallery(props.editor);
  return (
    <>
      <Section header={t("Media")}>
        <MenuRow
          label={t("Type")}
          items={[
            { id: "gifAndSound", name: t("GIF and sound") },
            { id: "video", name: t("Video") },
          ]}
          selected={alert().mediaType}
          onPick={(id) =>
            editAlert(props, (draft) => {
              draft.mediaType = id as "gifAndSound" | "video";
            })
          }
        />
      </Section>
      <Section
        footer={alert().mediaType === "gifAndSound" ? undefined : t("Pick the video on the phone.")}
      >
        <Show
          when={alert().mediaType === "gifAndSound"}
          fallback={<ValueRow label={t("Video")} value={alert().videoName} />}
        >
          <LinkRow
            label={t("Image")}
            value={mediaName(
              [...(current()?.bundledImages ?? []), ...(current()?.customImages ?? [])],
              alert().imageId,
            )}
            onClick={() =>
              props.stack.push({
                title: t("Image"),
                render: () => <ImageSelectorPage {...props} />,
              })
            }
          />
          <LinkRow
            label={t("Sound")}
            value={mediaName(
              [...(current()?.bundledSounds ?? []), ...(current()?.customSounds ?? [])],
              alert().soundId,
            )}
            onClick={() =>
              props.stack.push({
                title: t("Sound"),
                render: () => <SoundSelectorPage {...props} />,
              })
            }
          />
        </Show>
      </Section>
    </>
  );
}

function AlertPosition(props: AlertProps) {
  const alert = () => props.alert(props.widget)!;
  return (
    <>
      <Section header={t("Position")}>
        <MenuRow
          label={t("Type")}
          items={[
            { id: "Scene", name: t("Scene") },
            { id: "Face", name: t("Face") },
          ]}
          selected={alert().positionType}
          onPick={(id) =>
            editAlert(props, (draft) => {
              draft.positionType = id as "Scene" | "Face";
            })
          }
        />
      </Section>
      <Show when={alert().positionType === "Face"}>
        <Section>
          <PositioningRectangle
            image={asset("alert-face.png")}
            aspect="515 / 635"
            rect={alert().facePosition}
            onChange={(rect) =>
              editAlert(props, (draft) => {
                draft.facePosition = rect;
              })
            }
          />
        </Section>
      </Show>
    </>
  );
}

function AlertColorsAndFont(props: AlertProps) {
  const alert = () => props.alert(props.widget)!;
  const designs = fontDesignItems();
  const weights = fontWeightItems();
  return (
    <Show when={alert().positionType === "Scene"}>
      <Section header={t("Colors")}>
        <ColorRow
          label={t("Text")}
          color={alert().textColor}
          onChange={(color) =>
            editAlert(props, (draft) => {
              draft.textColor = color;
            })
          }
        />
        <ColorRow
          label={t("Accent")}
          color={alert().accentColor}
          onChange={(color) =>
            editAlert(props, (draft) => {
              draft.accentColor = color;
            })
          }
        />
      </Section>
      <Section header={t("Font")}>
        <SliderRow
          label={t("Size")}
          value={alert().fontSize}
          minimum={10}
          maximum={80}
          step={5}
          format={(value) => String(Math.round(value))}
          onChange={(value) =>
            editAlert(props, (draft) => {
              draft.fontSize = Math.round(value);
            })
          }
        />
        <MenuRow
          label={t("Design")}
          items={designs}
          selected={alert().fontDesign}
          onPick={(id) =>
            editAlert(props, (draft) => {
              draft.fontDesign = id as FontDesign;
            })
          }
        />
        <MenuRow
          label={t("Weight")}
          items={weights}
          selected={alert().fontWeight}
          onPick={(id) =>
            editAlert(props, (draft) => {
              draft.fontWeight = id as FontWeight;
            })
          }
        />
      </Section>
    </Show>
  );
}

const languageName = (code: string) => {
  try {
    return new Intl.DisplayNames([language], { type: "language" }).of(code) ?? code;
  } catch {
    return code;
  }
};

const voiceFlag = (language: string) => {
  const region = language.split(/[-_]/)[1] ?? "";
  return /^[A-Z]{2}$/.test(region)
    ? String.fromCodePoint(...[...region].map((letter) => 127397 + letter.charCodeAt(0)))
    : "";
};

const voiceLabel = (flag: string, name: string) => (flag === "" ? name : `${flag} ${name}`);

const isAppleVoice = (voice: SettingsVoice | undefined, identifier: string) =>
  voice !== undefined && voice.type === "apple" && voice.apple.voice === identifier;

const selectedVoiceName = (
  voices: { name: string; identifier: string }[],
  voice: SettingsVoice | undefined,
) => {
  if (!voice) {
    return undefined;
  }
  switch (voice.type) {
    case "ttsMonster":
      return voice.ttsMonster.name;
    case "elevenLabs":
      return voice.elevenLabs.name;
    default:
      return (
        voices.find((candidate) => candidate.identifier === voice.apple.voice)?.name ?? t("Unknown")
      );
  }
};

function VoicesPage(props: AlertProps) {
  const alert = () => props.alert(props.widget)!;
  const languages = () => props.editor.state.options?.appleVoices ?? [];
  return (
    <Section footer={t("TTS Monster and ElevenLabs voices are picked on the phone.")}>
      <For each={languages()}>
        {(entry) => (
          <LinkRow
            label={languageName(entry.language)}
            value={selectedVoiceName(
              entry.voices,
              alert().textToSpeechLanguageVoices[entry.language],
            )}
            onClick={() =>
              props.stack.push({
                title: languageName(entry.language),
                render: () => (
                  <>
                    <Section>
                      <For each={entry.voices}>
                        {(voice) => (
                          <button
                            type="button"
                            class="flex min-h-[44px] w-full items-center gap-3 px-4 py-2 text-left text-[17px]"
                            onClick={() =>
                              editAlert(props, (draft) => {
                                draft.textToSpeechLanguageVoices[entry.language] = {
                                  ...defaultVoice(),
                                  apple: { voice: voice.identifier },
                                };
                              })
                            }
                          >
                            <span class="min-w-0 flex-1 truncate">
                              {voiceLabel(voiceFlag(voice.language), voice.name)}
                            </span>
                            <Show
                              when={isAppleVoice(
                                alert().textToSpeechLanguageVoices[entry.language],
                                voice.identifier,
                              )}
                            >
                              <Icon name="check" class="size-[18px] text-phone-tint" />
                            </Show>
                          </button>
                        )}
                      </For>
                    </Section>
                    <Section>
                      <PlainRow>
                        <span>
                          {t(
                            "Download enhanced and premium voices in iOS Settings → Accessibility → Live Speech → Preferred Voices.",
                          )}
                        </span>
                      </PlainRow>
                    </Section>
                    <Section>
                      <ButtonRow
                        label={t("Reset")}
                        onClick={() =>
                          editAlert(props, (draft) => {
                            delete draft.textToSpeechLanguageVoices[entry.language];
                          })
                        }
                      />
                    </Section>
                  </>
                ),
              })
            }
          />
        )}
      </For>
    </Section>
  );
}

function AlertTextToSpeech(props: AlertProps) {
  const alert = () => props.alert(props.widget)!;
  return (
    <Section header={t("Text to speech")}>
      <ToggleRow
        label={t("Enabled")}
        on={alert().textToSpeechEnabled}
        onToggle={(on) =>
          editAlert(props, (draft) => {
            draft.textToSpeechEnabled = on;
          })
        }
      />
      <SliderRow
        label={t("Delay")}
        value={alert().textToSpeechDelay}
        minimum={0}
        maximum={5}
        step={0.5}
        format={(value) => value.toFixed(1)}
        onChange={(value) =>
          editAlert(props, (draft) => {
            draft.textToSpeechDelay = value;
          })
        }
      />
      <LinkRow
        label={t("Voices")}
        onClick={() =>
          props.stack.push({ title: t("Voices"), render: () => <VoicesPage {...props} /> })
        }
      />
    </Section>
  );
}

function AlertEditor(props: AlertProps & { before?: JSX.Element; simple?: boolean }) {
  return (
    <Show when={props.alert(props.widget)}>
      {(alert) => (
        <>
          <Section>
            <ToggleRow
              label={t("Enabled")}
              on={alert().enabled}
              onToggle={(on) =>
                editAlert(props, (draft) => {
                  draft.enabled = on;
                })
              }
            />
          </Section>
          {props.before}
          <AlertMedia {...props} />
          <AlertPosition {...props} />
          <Show when={!props.simple}>
            <AlertColorsAndFont {...props} />
            <AlertTextToSpeech {...props} />
          </Show>
        </>
      )}
    </Show>
  );
}

const pushAlert = (props: WidgetFormProps, title: string, alert: AlertPath) =>
  props.stack.push({ title, render: () => <AlertEditor {...props} alert={alert} /> });

const compactCount = (count: number) => {
  try {
    return new Intl.NumberFormat(language, { notation: "compact" }).format(count);
  } catch {
    return String(count);
  }
};

const cheerTitle = (bits: number, comparison: string) => {
  if (comparison === "=") {
    return bits === 1
      ? t("Cheer {count} bit", { count: compactCount(bits) })
      : t("Cheer {count} bits", { count: compactCount(bits) });
  }
  return t("Cheer {count}+ bits", { count: compactCount(bits) });
};

const kicksTitle = (amount: number, comparison: string) =>
  comparison === "="
    ? t("Kicks {count}", { count: compactCount(amount) })
    : t("Kicks {count}+", { count: compactCount(amount) });

const wholeNumber = (text: string) => (/^[+-]?\d+$/.test(text.trim()) ? null : t("Not a number"));

type Threshold = {
  id: string;
  bits?: number;
  amount?: number;
  comparisonOperator: "=" | ">=";
  alert: SettingsAlert;
};

function ThresholdList(props: WidgetFormProps & { kind: "cheerBits" | "kickGifts" }) {
  const listOf = (widget: SettingsWidget): Threshold[] =>
    props.kind === "cheerBits" ? widget.alerts.twitch.cheerBits : widget.alerts.kick.kickGifts;
  const amountOf = (item: Threshold) => (props.kind === "cheerBits" ? item.bits! : item.amount!);
  const title = (item: Threshold) =>
    props.kind === "cheerBits"
      ? cheerTitle(amountOf(item), item.comparisonOperator)
      : kicksTitle(amountOf(item), item.comparisonOperator);
  const open = (id: string) => {
    const item = () => listOf(props.widget).find((candidate) => candidate.id === id);
    const editItem = (change: (item: Threshold) => void) =>
      props.edit((draft) => {
        const target = listOf(draft).find((candidate) => candidate.id === id);
        if (target) {
          change(target);
        }
      });
    props.stack.push({
      title: () => (item() ? title(item()!) : ""),
      render: () => (
        <AlertEditor
          {...props}
          alert={(widget) => listOf(widget).find((candidate) => candidate.id === id)?.alert}
          before={
            <Section>
              <TextRow
                stack={props.stack}
                label={props.kind === "cheerBits" ? t("Bits") : t("Amount")}
                value={String(amountOf(item()!))}
                validate={wholeNumber}
                onSubmit={(text) =>
                  editItem((target) => {
                    if (props.kind === "cheerBits") {
                      target.bits = Number(text);
                    } else {
                      target.amount = Number(text);
                    }
                  })
                }
              />
              <MenuRow
                label={t("Comparison")}
                items={[
                  { id: "=", name: "=" },
                  { id: ">=", name: ">=" },
                ]}
                selected={item()!.comparisonOperator}
                onPick={(value) =>
                  editItem((target) => {
                    target.comparisonOperator = value as "=" | ">=";
                  })
                }
              />
            </Section>
          }
        />
      ),
    });
  };
  return (
    <Section
      footer={
        <span class="flex flex-col">
          <span>
            {props.kind === "cheerBits"
              ? t("The first item that matches cheered bits will be played.")
              : t("The first item that matches kicks amount will be played.")}
          </span>
          <span>&nbsp;</span>
          <span>{t("Delete an item with its trash button.")}</span>
        </span>
      }
    >
      <ReorderableList
        items={listOf(props.widget)}
        render={(item) => (
          <button
            type="button"
            class="flex min-h-[44px] w-full items-center gap-2 pr-2 pl-3 text-left text-[17px]"
            onClick={() => open(item.id)}
          >
            <span class="min-w-0 flex-1 truncate">{title(item)}</span>
            <Icon name="chevron-right" class="size-[16px] shrink-0 text-phone-border" />
          </button>
        )}
        onMove={(from, to) =>
          props.edit((draft) => {
            const list = listOf(draft);
            const [moved] = list.splice(from, 1);
            list.splice(to, 0, moved!);
          })
        }
        onDelete={(item) =>
          props.edit((draft) => {
            if (props.kind === "cheerBits") {
              draft.alerts.twitch.cheerBits = draft.alerts.twitch.cheerBits.filter(
                (candidate) => candidate.id !== item.id,
              );
            } else {
              draft.alerts.kick.kickGifts = draft.alerts.kick.kickGifts.filter(
                (candidate) => candidate.id !== item.id,
              );
            }
          })
        }
      />
      <ButtonRow
        label={t("Create")}
        onClick={() =>
          props.edit((draft) => {
            if (props.kind === "cheerBits") {
              draft.alerts.twitch.cheerBits.push(defaultCheerBits(newUuid()));
            } else {
              draft.alerts.kick.kickGifts.push(defaultKickGifts(newUuid()));
            }
          })
        }
      />
    </Section>
  );
}

function TwitchPage(props: WidgetFormProps) {
  return (
    <Section>
      <LinkRow
        label={t("Follows")}
        onClick={() => pushAlert(props, t("Follows"), (widget) => widget.alerts.twitch.follows)}
      />
      <LinkRow
        label={t("Subscriptions")}
        onClick={() =>
          pushAlert(props, t("Subscriptions"), (widget) => widget.alerts.twitch.subscriptions)
        }
      />
      <LinkRow
        label={t("Raids")}
        onClick={() => pushAlert(props, t("Raids"), (widget) => widget.alerts.twitch.raids)}
      />
      <LinkRow
        label={t("Cheers")}
        onClick={() =>
          props.stack.push({
            title: t("Cheers"),
            render: () => <ThresholdList {...props} kind="cheerBits" />,
          })
        }
      />
    </Section>
  );
}

function KickPage(props: WidgetFormProps) {
  return (
    <Section>
      <LinkRow
        label={t("Subscriptions")}
        onClick={() =>
          pushAlert(props, t("Subscriptions"), (widget) => widget.alerts.kick.subscriptions)
        }
      />
      <LinkRow
        label={t("Gift subscriptions")}
        onClick={() =>
          pushAlert(
            props,
            t("Gift subscriptions"),
            (widget) => widget.alerts.kick.giftedSubscriptions,
          )
        }
      />
      <LinkRow
        label={t("Hosts")}
        onClick={() => pushAlert(props, t("Hosts"), (widget) => widget.alerts.kick.hosts)}
      />
      <LinkRow
        label={t("Rewards")}
        onClick={() => pushAlert(props, t("Rewards"), (widget) => widget.alerts.kick.rewards)}
      />
      <LinkRow
        label={t("Kicks")}
        onClick={() =>
          props.stack.push({
            title: t("Kicks"),
            render: () => <ThresholdList {...props} kind="kickGifts" />,
          })
        }
      />
    </Section>
  );
}

function NamedAlertList(props: WidgetFormProps & { kind: "commands" | "strings" }) {
  const commands = () => props.kind === "commands";
  const items = (): { id: string; label: string; alert: SettingsAlert }[] =>
    commands()
      ? props.widget.alerts.chatBot.commands.map((command) => ({
          id: command.id,
          label: command.name,
          alert: command.alert,
        }))
      : props.widget.alerts.speechToText.strings.map((string) => ({
          id: string.id,
          label: string.string,
          alert: string.alert,
        }));
  const open = (id: string) => {
    const label = () => items().find((candidate) => candidate.id === id)?.label ?? "";
    const setLabel = (value: string) =>
      props.edit((draft) => {
        if (commands()) {
          const target = draft.alerts.chatBot.commands.find((candidate) => candidate.id === id);
          if (target) {
            target.name = value.toLowerCase().replace(/\s+/g, "");
          }
        } else {
          const target = draft.alerts.speechToText.strings.find((candidate) => candidate.id === id);
          if (target) {
            target.string = value;
          }
        }
      });
    props.stack.push({
      title: commands() ? t("Command") : t("String"),
      render: () => (
        <AlertEditor
          {...props}
          simple={!commands()}
          alert={(widget) =>
            commands()
              ? widget.alerts.chatBot.commands.find((candidate) => candidate.id === id)?.alert
              : widget.alerts.speechToText.strings.find((candidate) => candidate.id === id)?.alert
          }
          before={
            <Section
              footer={
                commands()
                  ? t("Trigger with chat message '!moblin alert {name}'", { name: label() })
                  : t("Trigger by saying '{text}'.", { text: label() })
              }
            >
              <TextRow
                stack={props.stack}
                label={commands() ? t("Name") : t("String")}
                value={label()}
                onSubmit={setLabel}
              />
            </Section>
          }
        />
      ),
    });
  };
  return (
    <Section
      footer={
        <span class="flex flex-col">
          <span>
            {commands()
              ? t("Trigger alerts with chat bot commands.")
              : t("Trigger alerts when you say something.")}
          </span>
          <span>&nbsp;</span>
          <span>
            {commands()
              ? t("Delete a command with its trash button.")
              : t("Delete a string with its trash button.")}
          </span>
        </span>
      }
    >
      <ReorderableList
        items={items()}
        render={(item) => (
          <button
            type="button"
            class="flex min-h-[44px] w-full items-center gap-2 pr-2 pl-3 text-left text-[17px]"
            onClick={() => open(item.id)}
          >
            <span class={`min-w-0 flex-1 truncate ${commands() ? "capitalize" : ""}`}>
              {item.label}
            </span>
            <Icon name="chevron-right" class="size-[16px] shrink-0 text-phone-border" />
          </button>
        )}
        onDelete={(item) =>
          props.edit((draft) => {
            if (commands()) {
              draft.alerts.chatBot.commands = draft.alerts.chatBot.commands.filter(
                (candidate) => candidate.id !== item.id,
              );
            } else {
              draft.alerts.speechToText.strings = draft.alerts.speechToText.strings.filter(
                (candidate) => candidate.id !== item.id,
              );
            }
          })
        }
      />
      <ButtonRow
        label={t("Create")}
        onClick={() => {
          props.edit((draft) => {
            const id = newUuid();
            if (commands()) {
              draft.alerts.chatBot.commands.push({
                id,
                name: "myname",
                alert: defaultAlert(`${id}:alert`),
                imageType: "File",
              });
            } else {
              draft.alerts.speechToText.strings.push({
                id,
                string: "",
                alert: defaultAlert(`${id}:alert`),
              });
            }
          });
          fixAlertMedias(props.editor);
        }}
      />
    </Section>
  );
}

const aiConfigured = (ai: SettingsWidget["alerts"]["ai"]) =>
  validUrl(ai.baseUrl) === null && ai.apiKey !== "" && ai.model !== "" && ai.role !== "";

function AiPage(props: WidgetFormProps) {
  const ai = () => props.widget.alerts.ai;
  return (
    <Section header={t("OpenAI compatible service")}>
      <TextRow
        stack={props.stack}
        label={t("Base URL")}
        value={ai().baseUrl}
        validate={validUrl}
        onSubmit={(value) =>
          props.edit((draft) => {
            draft.alerts.ai.baseUrl = value;
          })
        }
      />
      <TextRow
        stack={props.stack}
        label={t("API key")}
        value={ai().apiKey}
        masked
        onSubmit={(value) =>
          props.edit((draft) => {
            draft.alerts.ai.apiKey = value;
          })
        }
      />
      <TextRow
        stack={props.stack}
        label={t("Model")}
        value={ai().model}
        onSubmit={(value) =>
          props.edit((draft) => {
            draft.alerts.ai.model = value;
          })
        }
      />
      <TextRow
        stack={props.stack}
        label={t("Personality")}
        value={ai().role}
        multiline
        placeholder={t("You give fast and short answers.")}
        onSubmit={(value) =>
          props.edit((draft) => {
            draft.alerts.ai.role = value;
          })
        }
      />
    </Section>
  );
}

export function AlertsForm(props: WidgetFormProps) {
  const configured = () => aiConfigured(props.widget.alerts.ai);
  return (
    <>
      <Section>
        <LinkRow
          label="Twitch"
          icon="twitch"
          onClick={() =>
            props.stack.push({ title: "Twitch", render: () => <TwitchPage {...props} /> })
          }
        />
        <LinkRow
          label="Kick"
          icon="kick"
          onClick={() => props.stack.push({ title: "Kick", render: () => <KickPage {...props} /> })}
        />
        <LinkRow
          label={t("Moblin Cloud")}
          icon="cloud"
          onClick={() =>
            props.stack.push({
              title: t("Moblin Cloud"),
              render: () => (
                <Section>
                  <LinkRow
                    label={t("Message")}
                    onClick={() =>
                      pushAlert(props, t("Message"), (widget) => widget.alerts.moblinCloud.tips)
                    }
                  />
                </Section>
              ),
            })
          }
        />
        <LinkRow
          label={t("Chat bot")}
          onClick={() =>
            props.stack.push({
              title: t("Chat bot"),
              render: () => <NamedAlertList {...props} kind="commands" />,
            })
          }
        />
        <LinkRow
          label={t("Speech to text")}
          onClick={() =>
            props.stack.push({
              title: t("Speech to text"),
              render: () => <NamedAlertList {...props} kind="strings" />,
            })
          }
        />
      </Section>
      <Section>
        <div class="flex items-center">
          <button
            type="button"
            class="flex min-h-[44px] min-w-0 flex-1 items-center gap-3 px-4 py-2 text-left text-[17px]"
            onClick={() =>
              props.stack.push({ title: t("AI response"), render: () => <AiPage {...props} /> })
            }
          >
            <span class="min-w-0 flex-1 truncate">{t("AI response")}</span>
          </button>
          <div class="pr-1">
            <ToggleRow
              label=""
              on={props.widget.alerts.aiEnabled}
              disabled={!configured()}
              onToggle={(on) =>
                props.edit((draft) => {
                  draft.alerts.aiEnabled = on;
                })
              }
            />
          </div>
          <Icon name="chevron-right" class="mr-4 size-[16px] shrink-0 text-phone-border" />
        </div>
      </Section>
    </>
  );
}
