import { createSignal, Show } from "solid-js";
import Icon from "../../Icon";
import { ButtonRow, Footer, MenuRow, ReorderableList, Section } from "../Form";
import { widgetIcon } from "../icons";
import { CheckList, InlineField, NextRow, Sheet } from "./Sheet";
import { formatShortDuration, Lines, validUrl } from "../settings/widgets/shared";
import { sports } from "../settings/widgets/Scoreboard";
import { SuggestionsRow } from "../settings/widgets/Text";
import type { Stack } from "../Stack";
import type { Editor } from "../store";
import {
  applyVideoSource,
  videoSourceItemId,
  videoSourceMenuItems,
  videoSourceOf,
} from "../videoSources";
import {
  defaultSceneWidget,
  defaultWheelOption,
  makeUniqueName,
  newUuid,
  newWidget,
  sameId,
  widgetTypeDescription,
  widgetTypeName,
  widgetTypes,
  type ScoreboardSport,
  type SettingsWidget,
  type SettingsWidgetType,
} from "../../../data/moblinSettings";
import { SceneResolver } from "../../../data/sceneResolver";
import { t } from "../../../i18n";

export function WidgetWizard(props: {
  editor: Editor;
  sceneId: string | null;
  onClose: () => void;
}) {
  const editor = props.editor;
  const [type, setType] = createSignal<SettingsWidgetType>("Text");
  const [name, setName] = createSignal("");
  const [widget, setWidget] = createSignal<SettingsWidget>(newWidget("Text"));
  const [sceneIds, setSceneIds] = createSignal<string[]>(props.sceneId ? [props.sceneId] : []);
  const nameError = () =>
    editor.state.widgets.some((candidate) => candidate.name === name())
      ? t("The name '{name}' is already in use.", { name: name() })
      : null;
  const edit = (change: (widget: SettingsWidget) => void) => {
    const next = structuredClone(widget());
    change(next);
    setWidget(next);
  };
  const create = () => {
    const created = structuredClone(widget());
    created.type = type();
    created.name =
      name() === ""
        ? makeUniqueName(
            t("My widget"),
            editor.state.widgets.map((candidate) => candidate.name),
          )
        : name();
    editor.addWidget(created);
    for (const scene of editor.state.scenes) {
      if (!sceneIds().some((id) => sameId(id, scene.id))) {
        continue;
      }
      const addable = new SceneResolver(editor.state.scenes, editor.state.widgets).addableWidgets(
        scene,
      );
      if (addable.some((candidate) => sameId(candidate.id, created.id))) {
        editor.editScene(scene.id, (draft) => {
          draft.widgets.push(defaultSceneWidget(created.id, created.type, created.scoreboard.type));
        });
      }
    }
    props.onClose();
  };
  const scenesPage = (stack: Stack) =>
    stack.push({
      title: t("Scenes to add the widget to"),
      render: () => (
        <>
          <CheckList
            items={editor.state.scenes}
            label={(scene) => scene.name}
            checked={(scene) => sceneIds().some((id) => sameId(id, scene.id))}
            onToggle={(scene) =>
              setSceneIds((ids) =>
                ids.some((id) => sameId(id, scene.id))
                  ? ids.filter((id) => !sameId(id, scene.id))
                  : [...ids, scene.id],
              )
            }
          />
          <div class="overflow-hidden rounded-[10px] bg-phone-row">
            <button
              type="button"
              class="flex min-h-[44px] w-full items-center justify-center px-4 py-2 text-[17px] text-phone-tint"
              onClick={create}
            >
              {t("Create")}
            </button>
          </div>
        </>
      ),
    });
  const phonePage = (stack: Stack, text: string) =>
    stack.push({
      title: basicTitle(),
      render: () => (
        <>
          <Footer>{text}</Footer>
          <NextRow onClick={() => scenesPage(stack)} />
        </>
      ),
    });
  const basicTitle = () => t("Basic {type} widget settings", { type: widgetTypeName(type()) });
  const linesValue = (items: { text: string }[]) => items.map((item) => item.text).join("\n");
  const slideWidgets = () =>
    editor.state.widgets.filter(
      (candidate) => candidate.type === "Text" || candidate.type === "Image",
    );
  const slideWidgetOf = (id: string | null) =>
    slideWidgets().find((candidate) => sameId(candidate.id, id));
  const slidePage = (stack: Stack, slideId: string) =>
    stack.push({
      title: basicTitle(),
      render: () => {
        const slide = () => widget().slideshow.slides.find((candidate) => candidate.id === slideId);
        const editSlide = (change: (slide: { widgetId: string | null; time: number }) => void) =>
          edit((draft) => {
            const target = draft.slideshow.slides.find((candidate) => candidate.id === slideId);
            if (target) {
              change(target);
            }
          });
        return (
          <Show when={slide()}>
            {(current) => (
              <Section>
                <MenuRow
                  label={t("Widget")}
                  items={[
                    { id: "", name: t("-- None --") },
                    ...slideWidgets().map((candidate) => ({
                      id: candidate.id,
                      name: candidate.name,
                    })),
                  ]}
                  selected={current().widgetId ?? ""}
                  onPick={(id) =>
                    editSlide((target) => {
                      target.widgetId = id === "" ? null : id;
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
                    editSlide((target) => {
                      target.time = Number(id);
                    })
                  }
                />
              </Section>
            )}
          </Show>
        );
      },
    });
  const typePage = (stack: Stack) => {
    switch (type()) {
      case "Text":
        return stack.push({
          title: basicTitle(),
          render: () => (
            <>
              <Section>
                <textarea
                  class="min-h-[110px] w-full resize-none bg-transparent px-4 py-2 text-[17px] outline-none placeholder:text-phone-border"
                  value={widget().text.formatString}
                  placeholder={t("My text")}
                  autocapitalize="off"
                  spellcheck={false}
                  onInput={(event) =>
                    edit((draft) => {
                      draft.text.formatString = event.currentTarget.value;
                    })
                  }
                />
              </Section>
              <SuggestionsRow
                stack={stack}
                value={widget().text.formatString}
                isPhone={editor.state.options?.isPhone ?? true}
                onPick={(text) =>
                  edit((draft) => {
                    draft.text.formatString = text;
                  })
                }
              />
              <NextRow
                disabled={widget().text.formatString === ""}
                onClick={() => scenesPage(stack)}
              />
            </>
          ),
        });
      case "Browser":
        return stack.push({
          title: basicTitle(),
          render: () => (
            <>
              <InlineField
                header={t("URL")}
                value={widget().browser.url}
                placeholder="https://example.com"
                error={validUrl(widget().browser.url)}
                onInput={(value) =>
                  edit((draft) => {
                    draft.browser.url = value;
                  })
                }
              />
              <NextRow
                disabled={widget().browser.url === "" || validUrl(widget().browser.url) !== null}
                onClick={() => scenesPage(stack)}
              />
            </>
          ),
        });
      case "Video source":
        return stack.push({
          title: basicTitle(),
          render: () => (
            <>
              <Section>
                <MenuRow
                  label={t("Video source")}
                  items={videoSourceMenuItems(editor.state.videoSources, widget().videoSource)}
                  selected={videoSourceItemId(widget().videoSource)}
                  onPick={(id) => {
                    const source = videoSourceOf(editor.state.videoSources, id);
                    if (source) {
                      edit((draft) => applyVideoSource(draft.videoSource, source));
                    }
                  }}
                />
              </Section>
              <NextRow onClick={() => scenesPage(stack)} />
            </>
          ),
        });
      case "Image":
        return phonePage(stack, t("Pick the image on the phone once the widget is created."));
      case "Slideshow":
        return stack.push({
          title: basicTitle(),
          render: () => (
            <>
              <Section
                header={t("Slides")}
                footer={t("Drag a slide to reorder it, and delete it with its trash button.")}
              >
                <ReorderableList
                  items={widget().slideshow.slides}
                  render={(slide) => (
                    <button
                      type="button"
                      class="flex min-h-[44px] w-full items-center gap-2 pr-2 pl-3 text-left text-[17px]"
                      onClick={() => slidePage(stack, slide.id)}
                    >
                      <Show
                        when={slideWidgetOf(slide.widgetId)}
                        fallback={<span class="flex-1">{t("-- None --")}</span>}
                      >
                        {(slideWidget) => (
                          <>
                            <Icon
                              name={widgetIcon(slideWidget().type)}
                              class="size-[18px] shrink-0 text-phone-tint"
                            />
                            <span class="min-w-0 flex-1 truncate">{slideWidget().name}</span>
                          </>
                        )}
                      </Show>
                      <span class="text-phone-border">{slide.time}s</span>
                      <Icon name="chevron-right" class="size-[16px] shrink-0 text-phone-border" />
                    </button>
                  )}
                  onMove={(from, to) =>
                    edit((draft) => {
                      const [moved] = draft.slideshow.slides.splice(from, 1);
                      draft.slideshow.slides.splice(to, 0, moved!);
                    })
                  }
                  onDelete={(slide) =>
                    edit((draft) => {
                      draft.slideshow.slides = draft.slideshow.slides.filter(
                        (candidate) => candidate.id !== slide.id,
                      );
                    })
                  }
                />
                <ButtonRow
                  label={t("Add")}
                  onClick={() =>
                    edit((draft) => {
                      draft.slideshow.slides.push({ id: newUuid(), widgetId: null, time: 15 });
                    })
                  }
                />
              </Section>
              <NextRow
                disabled={widget().slideshow.slides.length === 0}
                onClick={() => scenesPage(stack)}
              />
            </>
          ),
        });
      case "VTuber":
      case "PNGTuber":
        return phonePage(stack, t("Pick the model on the phone once the widget is created."));
      case "Wheel of luck":
        return stack.push({
          title: basicTitle(),
          render: () => (
            <>
              <Lines
                header={t("Options")}
                value={linesValue(widget().wheelOfLuck.options)}
                placeholder={t("My text")}
                onChange={(text) =>
                  edit((draft) => {
                    draft.wheelOfLuck.options = text
                      .trim()
                      .split("\n")
                      .map((line) => defaultWheelOption(newUuid(), line.trim()));
                  })
                }
              />
              <NextRow
                disabled={widget().wheelOfLuck.options.length === 0}
                onClick={() => scenesPage(stack)}
              />
            </>
          ),
        });
      case "Bingo card":
        return stack.push({
          title: basicTitle(),
          render: () => (
            <>
              <Lines
                header={t("Squares")}
                value={linesValue(widget().bingoCard.squares)}
                placeholder={t("My text")}
                onChange={(text) =>
                  edit((draft) => {
                    draft.bingoCard.squares = text.split("\n").map((line, index) => ({
                      ...(draft.bingoCard.squares[index] ?? {
                        id: newUuid(),
                        checked: false,
                      }),
                      text: line.trim(),
                    }));
                  })
                }
              />
              <NextRow onClick={() => scenesPage(stack)} />
            </>
          ),
        });
      case "Scoreboard":
        return stack.push({
          title: basicTitle(),
          render: () => (
            <>
              <Section>
                <MenuRow
                  label={t("Sport")}
                  items={sports()}
                  selected={widget().scoreboard.type}
                  onPick={(id) =>
                    edit((draft) => {
                      draft.scoreboard.type = id as ScoreboardSport;
                    })
                  }
                />
              </Section>
              <NextRow onClick={() => scenesPage(stack)} />
            </>
          ),
        });
      default:
        return scenesPage(stack);
    }
  };
  return (
    <Sheet
      onClose={props.onClose}
      root={(stack) => ({
        title: t("Create widget wizard"),
        render: () => (
          <>
            <Section footer={widgetTypeDescription(type())}>
              <MenuRow
                label={t("Type")}
                items={widgetTypes.map((candidate) => ({
                  id: candidate,
                  name: widgetTypeName(candidate),
                }))}
                selected={type()}
                onPick={(id) => setType(id as SettingsWidgetType)}
              />
            </Section>
            <InlineField
              header={t("Name")}
              value={name()}
              placeholder={t("My widget")}
              error={nameError()}
              onInput={setName}
            />
            <NextRow disabled={nameError() !== null} onClick={() => typePage(stack)} />
          </>
        ),
      })}
    />
  );
}
