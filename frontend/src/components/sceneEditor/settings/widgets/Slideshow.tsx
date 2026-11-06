import { Show } from "solid-js";
import Icon from "../../../Icon";
import { ButtonRow, LinkRow, MenuRow, ReorderableList, Section } from "../../Form";
import { widgetIcon } from "../../icons";
import { formatShortDuration, type WidgetFormProps } from "./shared";
import { WidgetSettings } from "../WidgetSettings";
import { newUuid, sameId } from "../../../../data/moblinSettings";
import { t } from "../../../../i18n";

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
