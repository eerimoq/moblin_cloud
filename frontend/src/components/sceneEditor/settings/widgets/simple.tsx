import { createMemo, Show } from "solid-js";
import Icon from "../../../Icon";
import {
  ButtonRow,
  ColorRow,
  LinkRow,
  MenuRow,
  NumberRow,
  PickerRow,
  ReorderableList,
  Section,
  SliderRow,
  TextRow,
  ToggleRow,
} from "../../Form";
import { symbolIcon, widgetIcon } from "../../icons";
import { EffectsSection } from "./effects";
import {
  FontRows,
  formatShortDuration,
  Lines,
  ModelSection,
  pickerItems,
  SensitivitySection,
  validUrl,
  VideoSourceRow,
  type WidgetFormProps,
} from "./shared";
import { WidgetSettings } from "../WidgetSettings";
import {
  newUuid,
  pomodoroBreakIcons,
  pomodoroFocusIcons,
  sameId,
  type BrowserMode,
  type ChatDisplayStyle,
} from "../../../../data/moblinSettings";
import { SceneResolver } from "../../../../data/sceneResolver";
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

export function CropForm(props: WidgetFormProps) {
  const crop = () => props.widget.crop;
  const browsers = () => props.editor.state.widgets.filter((widget) => widget.type === "Browser");
  const exists = () => browsers().some((widget) => sameId(widget.id, crop().sourceWidgetId));
  return (
    <Section>
      <MenuRow
        label={t("Source widget")}
        items={[
          ...(exists() ? [] : [{ id: "", name: "" }]),
          ...browsers().map((widget) => ({ id: widget.id, name: widget.name })),
        ]}
        selected={exists() ? crop().sourceWidgetId : ""}
        onPick={(id) =>
          props.edit((draft) => {
            if (id !== "") {
              draft.crop.sourceWidgetId = id;
            }
          })
        }
      />
      <NumberRow
        stack={props.stack}
        label={t("X")}
        value={crop().x}
        minimum={0}
        integer
        onSubmit={(x) =>
          props.edit((draft) => {
            draft.crop.x = x;
          })
        }
      />
      <NumberRow
        stack={props.stack}
        label={t("Y")}
        value={crop().y}
        minimum={0}
        integer
        onSubmit={(y) =>
          props.edit((draft) => {
            draft.crop.y = y;
          })
        }
      />
      <NumberRow
        stack={props.stack}
        label={t("Width")}
        value={crop().width}
        minimum={1}
        integer
        onSubmit={(width) =>
          props.edit((draft) => {
            draft.crop.width = width;
          })
        }
      />
      <NumberRow
        stack={props.stack}
        label={t("Height")}
        value={crop().height}
        minimum={1}
        integer
        onSubmit={(height) =>
          props.edit((draft) => {
            draft.crop.height = height;
          })
        }
      />
    </Section>
  );
}

export function MapForm(props: WidgetFormProps) {
  const map = () => props.widget.map;
  return (
    <>
      <Section footer={t("The map will rotate based of movement direction if disabled.")}>
        <ToggleRow
          label={t("North up")}
          on={map().northUp}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.map.northUp = on;
            })
          }
        />
      </Section>
      <Section header={t("Size")}>
        <SliderRow
          value={map().scale}
          minimum={500}
          maximum={10000}
          step={100}
          onChange={(scale) =>
            props.edit((draft) => {
              draft.map.scale = scale;
            })
          }
        />
      </Section>
      <Section
        header={t("Delay")}
        footer={t("To show the widget in sync with high latency cameras.")}
      >
        <SliderRow
          value={map().delay}
          minimum={0}
          maximum={10}
          step={0.5}
          format={(value) => value.toFixed(1)}
          onChange={(delay) =>
            props.edit((draft) => {
              draft.map.delay = delay;
            })
          }
        />
      </Section>
      <EffectsSection {...props} />
    </>
  );
}

export function SceneForm(props: WidgetFormProps) {
  const resolver = createMemo(
    () => new SceneResolver(props.editor.state.scenes, props.editor.state.widgets),
  );
  const scenes = () => resolver().selectableScenes(props.widget);
  return (
    <Section header={t("Scene")}>
      {scenes().map((scene) => (
        <button
          type="button"
          class="flex min-h-[44px] w-full items-center gap-3 px-4 py-2 text-left text-[17px]"
          onClick={() =>
            props.edit((draft) => {
              draft.scene.sceneId = scene.id;
            })
          }
        >
          <span class="min-w-0 flex-1 truncate">{scene.name}</span>
          <Show when={sameId(scene.id, props.widget.scene.sceneId)}>
            <Icon name="check" class="size-[18px] text-phone-tint" />
          </Show>
        </button>
      ))}
    </Section>
  );
}

export function QrCodeForm(props: WidgetFormProps) {
  return (
    <>
      <Section>
        <TextRow
          stack={props.stack}
          label={t("Message")}
          value={props.widget.qrCode.message}
          onSubmit={(message) =>
            props.edit((draft) => {
              draft.qrCode.message = message;
            })
          }
        />
      </Section>
      <EffectsSection {...props} />
    </>
  );
}

export function SnapshotForm(props: WidgetFormProps) {
  return (
    <>
      <Section>
        <MenuRow
          label={t("Showtime")}
          items={pickerItems([3, 5, 10, 15, 30, 60, 120], "s")}
          selected={String(props.widget.snapshot.showtime)}
          onPick={(id) =>
            props.edit((draft) => {
              draft.snapshot.showtime = Number(id);
            })
          }
        />
      </Section>
      <EffectsSection {...props} />
    </>
  );
}

export function ChatEmoteComboForm(props: WidgetFormProps) {
  return (
    <Section>
      <MenuRow
        label={t("Minimum combo")}
        items={pickerItems([2, 3, 4, 5, 6, 7, 8, 9, 10])}
        selected={String(props.widget.chatEmoteCombo.minimumCombo)}
        onPick={(id) =>
          props.edit((draft) => {
            draft.chatEmoteCombo.minimumCombo = Number(id);
          })
        }
      />
      <MenuRow
        label={t("Timeout")}
        items={pickerItems([3, 4, 5, 6, 7, 8, 9, 10], "s")}
        selected={String(props.widget.chatEmoteCombo.resetAfter)}
        onPick={(id) =>
          props.edit((draft) => {
            draft.chatEmoteCombo.resetAfter = Number(id);
          })
        }
      />
    </Section>
  );
}

export function ImageForm(props: WidgetFormProps) {
  return (
    <>
      <Section footer={t("Pick the image on the phone.")}>
        <div class="flex w-full justify-center px-4 py-3">
          <Icon name="image" class="size-[20px] text-phone-border" />
        </div>
      </Section>
      <EffectsSection {...props} />
    </>
  );
}

export function SlideshowForm(props: WidgetFormProps) {
  const slides = () => props.widget.slideshow.slides;
  const widgets = () =>
    props.editor.state.widgets.filter(
      (widget) => widget.type === "Text" || widget.type === "Image",
    );
  const widgetOf = (id: string | null) => widgets().find((widget) => sameId(widget.id, id));
  const openSlide = (slideId: string) =>
    props.stack.push({
      title: t("Slide"),
      render: () => {
        const slide = () =>
          props.widget.slideshow.slides.find((candidate) => candidate.id === slideId);
        return (
          <Show when={slide()}>
            {(current) => (
              <>
                <Section>
                  <MenuRow
                    label={t("Widget")}
                    items={[
                      { id: "", name: t("-- None --") },
                      ...widgets().map((widget) => ({ id: widget.id, name: widget.name })),
                    ]}
                    selected={current().widgetId ?? ""}
                    onPick={(id) =>
                      props.edit((draft) => {
                        const target = draft.slideshow.slides.find(
                          (candidate) => candidate.id === slideId,
                        );
                        if (target) {
                          target.widgetId = id === "" ? null : id;
                        }
                      })
                    }
                  />
                  <MenuRow
                    label={t("Time")}
                    items={[5, 10, 15, 30, 45, 60, 90, 120, 180, 240, 300].map((seconds) => ({
                      id: String(seconds),
                      name: formatShortDuration(seconds),
                    }))}
                    selected={String(current().time)}
                    onPick={(id) =>
                      props.edit((draft) => {
                        const target = draft.slideshow.slides.find(
                          (candidate) => candidate.id === slideId,
                        );
                        if (target) {
                          target.time = Number(id);
                        }
                      })
                    }
                  />
                </Section>
                <Show when={widgetOf(current().widgetId)}>
                  {(widget) => (
                    <Section header={t("Shortcut")}>
                      <LinkRow
                        label={t("Widget")}
                        onClick={() =>
                          props.stack.push({
                            title: widget().name,
                            render: () => (
                              <WidgetSettings
                                editor={props.editor}
                                stack={props.stack}
                                widgetId={widget().id}
                                settings={null}
                              />
                            ),
                          })
                        }
                      />
                    </Section>
                  )}
                </Show>
              </>
            )}
          </Show>
        );
      },
    });
  return (
    <Section
      header={t("Slides")}
      footer={t("Drag a slide to reorder it, and delete it with its trash button.")}
    >
      <ReorderableList
        items={slides()}
        render={(slide) => (
          <button
            type="button"
            class="flex min-h-[44px] w-full items-center gap-2 pr-2 pl-3 text-left text-[17px]"
            onClick={() => openSlide(slide.id)}
          >
            <Show
              when={widgetOf(slide.widgetId)}
              fallback={<span class="flex-1">{t("-- None --")}</span>}
            >
              {(widget) => (
                <>
                  <Icon
                    name={widgetIcon(widget().type)}
                    class="size-[18px] shrink-0 text-phone-tint"
                  />
                  <span class="min-w-0 flex-1 truncate">{widget().name}</span>
                </>
              )}
            </Show>
            <span class="text-phone-border">{slide.time}s</span>
            <Icon name="chevron-right" class="size-[16px] shrink-0 text-phone-border" />
          </button>
        )}
        onMove={(from, to) =>
          props.edit((draft) => {
            const [moved] = draft.slideshow.slides.splice(from, 1);
            draft.slideshow.slides.splice(to, 0, moved!);
          })
        }
        onDelete={(slide) =>
          props.edit((draft) => {
            draft.slideshow.slides = draft.slideshow.slides.filter(
              (candidate) => candidate.id !== slide.id,
            );
          })
        }
      />
      <ButtonRow
        label={t("Add")}
        onClick={() =>
          props.edit((draft) => {
            draft.slideshow.slides.push({ id: newUuid(), widgetId: null, time: 15 });
          })
        }
      />
    </Section>
  );
}

export function VideoSourceForm(props: WidgetFormProps) {
  const source = () => props.widget.videoSource;
  return (
    <>
      <Section>
        <VideoSourceRow
          editor={props.editor}
          stack={props.stack}
          fields={source()}
          onChange={(change) => props.edit((draft) => change(draft.videoSource))}
        />
      </Section>
      <Section>
        <MenuRow
          label={t("Rotation")}
          items={[0, 90, 180, 270].map((degrees) => ({ id: String(degrees), name: `${degrees}°` }))}
          selected={String(source().rotation)}
          onPick={(id) =>
            props.edit((draft) => {
              draft.videoSource.rotation = Number(id);
            })
          }
        />
        <ToggleRow
          label={t("Mirror")}
          on={source().mirror}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.videoSource.mirror = on;
            })
          }
        />
      </Section>
      <Section header={t("Face tracking")}>
        <ToggleRow
          label={t("Enabled")}
          on={source().trackFaceEnabled}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.videoSource.trackFaceEnabled = on;
            })
          }
        />
        <SliderRow
          label={t("Zoom")}
          value={source().trackFaceZoom}
          minimum={0}
          maximum={1}
          step={0.01}
          onChange={(zoom) =>
            props.edit((draft) => {
              draft.videoSource.trackFaceZoom = zoom;
            })
          }
        />
      </Section>
      <EffectsSection {...props} />
    </>
  );
}

export function VTuberForm(props: WidgetFormProps) {
  const vTuber = () => props.widget.vTuber;
  return (
    <>
      <Section>
        <VideoSourceRow
          editor={props.editor}
          stack={props.stack}
          fields={vTuber()}
          onChange={(change) => props.edit((draft) => change(draft.vTuber))}
        />
      </Section>
      <ModelSection
        name={vTuber().modelName}
        footers={[
          t("Most VRM 0.x and 1.0 files and zipped Live2D Cubism models are supported."),
          t("Pick the model on the phone."),
        ]}
      />
      <Show when={vTuber().type === "VRM"}>
        <Section header={t("Camera")}>
          <SliderRow
            label={t("Vertical position")}
            value={vTuber().cameraPositionY}
            minimum={1}
            maximum={2}
            step={0.01}
            onChange={(value) =>
              props.edit((draft) => {
                draft.vTuber.cameraPositionY = value;
              })
            }
          />
          <SliderRow
            label={t("Field of view")}
            value={vTuber().cameraFieldOfView}
            minimum={10}
            maximum={30}
            step={1}
            onChange={(value) =>
              props.edit((draft) => {
                draft.vTuber.cameraFieldOfView = value;
              })
            }
          />
        </Section>
      </Show>
      <Section>
        <ToggleRow
          label={t("Mirror")}
          on={vTuber().mirror}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.vTuber.mirror = on;
            })
          }
        />
      </Section>
      <SensitivitySection
        sensitivity={vTuber().sensitivity}
        onChange={(sensitivity) =>
          props.edit((draft) => {
            draft.vTuber.sensitivity = sensitivity;
          })
        }
      />
      <Show when={vTuber().type === "VRM"}>
        <Section header={t("Angles")}>
          <SliderRow
            label={t("Arms")}
            value={vTuber().armsAngle}
            minimum={20}
            maximum={90}
            step={1}
            onChange={(value) =>
              props.edit((draft) => {
                draft.vTuber.armsAngle = value;
              })
            }
          />
        </Section>
      </Show>
    </>
  );
}

export function PngTuberForm(props: WidgetFormProps) {
  const pngTuber = () => props.widget.pngTuber;
  return (
    <>
      <Section>
        <VideoSourceRow
          editor={props.editor}
          stack={props.stack}
          fields={pngTuber()}
          onChange={(change) => props.edit((draft) => change(draft.pngTuber))}
        />
      </Section>
      <ModelSection
        name={pngTuber().modelName}
        footers={[t("A .save-file from PNGTuberPlus."), t("Pick the model on the phone.")]}
      />
      <SensitivitySection
        sensitivity={pngTuber().sensitivity}
        onChange={(sensitivity) =>
          props.edit((draft) => {
            draft.pngTuber.sensitivity = sensitivity;
          })
        }
      />
      <Section>
        <ToggleRow
          label={t("Mirror")}
          on={pngTuber().mirror}
          onToggle={(on) =>
            props.edit((draft) => {
              draft.pngTuber.mirror = on;
            })
          }
        />
      </Section>
    </>
  );
}

export function ChatForm(props: WidgetFormProps) {
  const chat = () => props.widget.chat;
  const showAll = () => props.editor.state.options?.showAllSettings ?? true;
  const styles: { id: ChatDisplayStyle; name: string }[] = [
    { id: "internationalName", name: t("International name") },
    { id: "internationalNameAndUsername", name: t("International name (Username)") },
    { id: "username", name: t("Username") },
  ];
  return (
    <>
      <Section header={t("Font")}>
        <SliderRow
          label={t("Size")}
          value={chat().fontSize}
          minimum={10}
          maximum={50}
          step={1}
          format={(value) => String(Math.round(value))}
          onChange={(fontSize) =>
            props.edit((draft) => {
              draft.chat.fontSize = fontSize;
            })
          }
        />
        <Show when={showAll()}>
          <FontRows
            editor={props.editor}
            stack={props.stack}
            family={chat().fontFamily}
            style={chat().fontStyle}
            onChange={(family, style) =>
              props.edit((draft) => {
                draft.chat.fontFamily = family;
                draft.chat.fontStyle = style;
              })
            }
          />
          <ToggleRow
            label={t("Bold name")}
            on={chat().boldUsername}
            onToggle={(on) =>
              props.edit((draft) => {
                draft.chat.boldUsername = on;
              })
            }
          />
          <ToggleRow
            label={t("Bold message")}
            on={chat().boldMessage}
            onToggle={(on) =>
              props.edit((draft) => {
                draft.chat.boldMessage = on;
              })
            }
          />
        </Show>
      </Section>
      <Section header={t("General")}>
        <MenuRow
          label={t("Messages")}
          items={pickerItems([1, 2, 3, 4, 5])}
          selected={String(chat().maximumNumberOfMessages)}
          onPick={(id) =>
            props.edit((draft) => {
              draft.chat.maximumNumberOfMessages = Number(id);
            })
          }
        />
        <SliderRow
          label={t("Height")}
          value={chat().height}
          minimum={0.1}
          maximum={1}
          step={0.01}
          format={(value) => `${Math.round(100 * value)}%`}
          onChange={(height) =>
            props.edit((draft) => {
              draft.chat.height = height;
            })
          }
        />
        <Show when={showAll()}>
          <MenuRow
            label={t("Display style")}
            items={styles}
            selected={chat().displayStyle}
            onPick={(id) =>
              props.edit((draft) => {
                draft.chat.displayStyle = id as ChatDisplayStyle;
              })
            }
          />
          <ToggleRow
            label={t("Badges")}
            on={chat().badges}
            onToggle={(on) =>
              props.edit((draft) => {
                draft.chat.badges = on;
              })
            }
          />
          <ToggleRow
            label={t("Shared chat icons")}
            on={chat().sharedChatIcons}
            onToggle={(on) =>
              props.edit((draft) => {
                draft.chat.sharedChatIcons = on;
              })
            }
          />
        </Show>
      </Section>
      <Section header={t("Colors")}>
        <Show when={showAll()}>
          <ColorRow
            label={t("Name")}
            color={chat().usernameColor}
            onChange={(color) =>
              props.edit((draft) => {
                draft.chat.usernameColor = color;
              })
            }
          />
          <ColorRow
            label={t("Message")}
            color={chat().messageColor}
            onChange={(color) =>
              props.edit((draft) => {
                draft.chat.messageColor = color;
              })
            }
          />
        </Show>
        <div class="flex items-center">
          <div class="min-w-0 flex-1">
            <ColorRow
              label={t("Background")}
              color={chat().backgroundColor}
              onChange={(color) =>
                props.edit((draft) => {
                  draft.chat.backgroundColor = color;
                })
              }
            />
          </div>
          <div class="pr-1">
            <ToggleRow
              label=""
              on={chat().backgroundColorEnabled}
              onToggle={(on) =>
                props.edit((draft) => {
                  draft.chat.backgroundColorEnabled = on;
                })
              }
            />
          </div>
        </div>
        <div class="flex items-center">
          <div class="min-w-0 flex-1">
            <ColorRow
              label={t("Border")}
              color={chat().shadowColor}
              onChange={(color) =>
                props.edit((draft) => {
                  draft.chat.shadowColor = color;
                })
              }
            />
          </div>
          <div class="pr-1">
            <ToggleRow
              label=""
              on={chat().shadowColorEnabled}
              onToggle={(on) =>
                props.edit((draft) => {
                  draft.chat.shadowColorEnabled = on;
                })
              }
            />
          </div>
        </div>
      </Section>
    </>
  );
}

export function BingoCardForm(props: WidgetFormProps) {
  const bingo = () => props.widget.bingoCard;
  return (
    <>
      <Lines
        header={t("Squares")}
        value={bingo()
          .squares.map((square) => square.text)
          .join("\n")}
        placeholder={t("My text")}
        onChange={(text) =>
          props.edit((draft) => {
            const lines = text.split("\n");
            draft.bingoCard.squares = lines.map((line, index) => {
              const square = draft.bingoCard.squares[index] ?? {
                id: newUuid(),
                text: "",
                checked: false,
              };
              square.text = line.trim();
              return square;
            });
          })
        }
      />
      <Section header={t("Marks")}>
        {bingo().squares.map((square) => (
          <div class="flex min-h-[44px] items-center gap-3 px-4 py-2 text-[17px]">
            <span class="min-w-0 flex-1 truncate">{square.text}</span>
            <button
              type="button"
              class="text-phone-tint"
              onClick={() =>
                props.edit((draft) => {
                  const target = draft.bingoCard.squares.find(
                    (candidate) => candidate.id === square.id,
                  );
                  if (target) {
                    target.checked = !target.checked;
                  }
                })
              }
            >
              <Icon
                name={square.checked ? "square-split-diagonal" : "square"}
                class="size-[26px]"
              />
            </button>
          </div>
        ))}
        <ButtonRow
          label={t("Reset")}
          onClick={() =>
            props.edit((draft) => {
              draft.bingoCard.squares = draft.bingoCard.squares.map((square) => ({
                id: newUuid(),
                text: square.text,
                checked: false,
              }));
            })
          }
        />
      </Section>
      <Section header={t("Colors")}>
        <ColorRow
          label={t("Background")}
          color={bingo().backgroundColor}
          opacity
          onChange={(color) =>
            props.edit((draft) => {
              draft.bingoCard.backgroundColor = color;
            })
          }
        />
        <ColorRow
          label={t("Foreground")}
          color={bingo().foregroundColor}
          onChange={(color) =>
            props.edit((draft) => {
              draft.bingoCard.foregroundColor = color;
            })
          }
        />
      </Section>
    </>
  );
}

const focusIconNames: Record<string, string> = {
  "sun.max": t("Sun"),
  "bolt.circle": t("Bolt"),
  graduationcap: t("Cap"),
  "text.book.closed": t("Book"),
  "arrowtriangle.right.circle": t("Play"),
  "brain.head.profile": t("Brain"),
  pencil: t("Pencil"),
  flame: t("Flame"),
  timer: t("Timer"),
  target: t("Target"),
};

const breakIconNames: Record<string, string> = {
  "cup.and.saucer": t("Cup"),
  "figure.dance": t("Dance"),
  "figure.walk": t("Walk"),
  "music.note": t("Music"),
  leaf: t("Leaf"),
  gamecontroller: t("Controller"),
  "fork.knife": t("Fork and knife"),
};

export function PomodoroTimerForm(props: WidgetFormProps) {
  const timer = () => props.widget.pomodoroTimer;
  const sounds = () => {
    const gallery = props.editor.state.gallery;
    return gallery ? [...gallery.bundledSounds, ...gallery.customSounds] : [];
  };
  const soundItems = (selected: string | null) => [
    ...(selected === null ? [{ id: "", name: t("-- None --") }] : []),
    ...sounds().map((sound) => ({ id: sound.id, name: sound.name })),
  ];
  return (
    <>
      <Section header={t("Durations")}>
        <MenuRow
          label={t("Focus")}
          items={pickerItems([1, 2, 3, 5, 10, 15, 20, 25, 30, 45, 60, 90, 120], " min")}
          selected={String(timer().focusDuration)}
          onPick={(id) =>
            props.edit((draft) => {
              draft.pomodoroTimer.focusDuration = Number(id);
            })
          }
        />
        <MenuRow
          label={t("Break")}
          items={pickerItems([1, 2, 3, 5, 7, 10, 15, 20, 25, 30], " min")}
          selected={String(timer().breakDuration)}
          onPick={(id) =>
            props.edit((draft) => {
              draft.pomodoroTimer.breakDuration = Number(id);
            })
          }
        />
      </Section>
      <Section header={t("Names")}>
        <TextRow
          stack={props.stack}
          label={t("Focus")}
          value={timer().focusName}
          onSubmit={(value) =>
            props.edit((draft) => {
              draft.pomodoroTimer.focusName = value;
            })
          }
        />
        <TextRow
          stack={props.stack}
          label={t("Break")}
          value={timer().breakName}
          onSubmit={(value) =>
            props.edit((draft) => {
              draft.pomodoroTimer.breakName = value;
            })
          }
        />
      </Section>
      <Section header={t("Icons")}>
        <div class="flex items-center">
          <Icon name={symbolIcon(timer().focusIcon)} class="ml-4 size-[18px] text-phone-tint" />
          <div class="min-w-0 flex-1">
            <MenuRow
              label={t("Focus")}
              items={pomodoroFocusIcons.map((icon) => ({
                id: icon,
                name: focusIconNames[icon] ?? icon,
              }))}
              selected={timer().focusIcon}
              onPick={(id) =>
                props.edit((draft) => {
                  draft.pomodoroTimer.focusIcon = id;
                })
              }
            />
          </div>
        </div>
        <div class="flex items-center">
          <Icon name={symbolIcon(timer().breakIcon)} class="ml-4 size-[18px] text-phone-tint" />
          <div class="min-w-0 flex-1">
            <MenuRow
              label={t("Break")}
              items={pomodoroBreakIcons.map((icon) => ({
                id: icon,
                name: breakIconNames[icon] ?? icon,
              }))}
              selected={timer().breakIcon}
              onPick={(id) =>
                props.edit((draft) => {
                  draft.pomodoroTimer.breakIcon = id;
                })
              }
            />
          </div>
        </div>
      </Section>
      <Section>
        <SliderRow
          label={t("Width")}
          value={timer().width}
          minimum={1}
          maximum={5}
          step={0.05}
          onChange={(width) =>
            props.edit((draft) => {
              draft.pomodoroTimer.width = width;
            })
          }
        />
      </Section>
      <Section header={t("Colors")}>
        <ColorRow
          label={t("Background")}
          color={timer().backgroundColor}
          opacity
          onChange={(color) =>
            props.edit((draft) => {
              draft.pomodoroTimer.backgroundColor = color;
            })
          }
        />
        <ColorRow
          label={t("Text")}
          color={timer().foregroundColor}
          onChange={(color) =>
            props.edit((draft) => {
              draft.pomodoroTimer.foregroundColor = color;
            })
          }
        />
        <ColorRow
          label={t("Focus")}
          color={timer().focusColor}
          onChange={(color) =>
            props.edit((draft) => {
              draft.pomodoroTimer.focusColor = color;
            })
          }
        />
        <ColorRow
          label={t("Break")}
          color={timer().breakColor}
          onChange={(color) =>
            props.edit((draft) => {
              draft.pomodoroTimer.breakColor = color;
            })
          }
        />
      </Section>
      <Section header={t("Sounds")}>
        <PickerRow
          stack={props.stack}
          label={t("Focus to break")}
          title={t("Sound")}
          items={soundItems(timer().focusToBreakSoundId)}
          selected={timer().focusToBreakSoundId ?? ""}
          onPick={(id) =>
            props.edit((draft) => {
              draft.pomodoroTimer.focusToBreakSoundId = id === "" ? null : id;
            })
          }
        />
        <PickerRow
          stack={props.stack}
          label={t("Break to focus")}
          title={t("Sound")}
          items={soundItems(timer().breakToFocusSoundId)}
          selected={timer().breakToFocusSoundId ?? ""}
          onPick={(id) =>
            props.edit((draft) => {
              draft.pomodoroTimer.breakToFocusSoundId = id === "" ? null : id;
            })
          }
        />
      </Section>
      <Section header={t("Chat messages")}>
        <TextRow
          stack={props.stack}
          label={t("Focus to break")}
          value={timer().focusToBreakChatMessage}
          onSubmit={(value) =>
            props.edit((draft) => {
              draft.pomodoroTimer.focusToBreakChatMessage = value;
            })
          }
        />
        <TextRow
          stack={props.stack}
          label={t("Break to focus")}
          value={timer().breakToFocusChatMessage}
          onSubmit={(value) =>
            props.edit((draft) => {
              draft.pomodoroTimer.breakToFocusChatMessage = value;
            })
          }
        />
      </Section>
    </>
  );
}
