import { createSignal, onMount, Show } from "solid-js";
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
  TextEditPage,
  ToggleRow,
} from "../../Form";
import { MaskCanvas, maskMinimumPoints } from "./MaskCanvas";
import { PositioningRectangle } from "./PositioningRectangle";
import type { WidgetFormProps } from "./shared";
import { asset } from "../../../../asset";
import {
  sameId,
  type SettingsVideoEffect,
  type VideoEffectType,
} from "../../../../data/moblinSettings";
import { t } from "../../../../i18n";

export const effectTypeName = (type: VideoEffectType): string =>
  ({
    shape: t("Shape"),
    grayScale: t("Gray scale"),
    sepia: t("Sepia"),
    whirlpool: t("Whirlpool"),
    pinch: t("Pinch"),
    removeBackground: t("Remove background"),
    dewarp360: t("Dewarp 360"),
    anamorphicLens: t("Anamorphic lens"),
    lut: t("LUT"),
    opacity: t("Opacity"),
    mask: t("Mask"),
  })[type];

const hasPage = (type: VideoEffectType) =>
  !["grayScale", "sepia", "whirlpool", "pinch"].includes(type);

const inverseFieldOfView = (zoom: number) => 180 - (360 * Math.atan(1 / zoom)) / Math.PI;

const zoomOfInverseFieldOfView = (inverseFieldOfView: number) =>
  1 / Math.tan(((180 - inverseFieldOfView) * Math.PI) / 360);

const swiftDouble = (value: number) =>
  Number.isInteger(value) && Math.abs(value) < 1e16 ? `${value}.0` : String(value);

function EffectPage(props: WidgetFormProps & { effectId: string }) {
  const effect = () => props.widget.effects.find((candidate) => candidate.id === props.effectId);
  const edit = (change: (effect: SettingsVideoEffect) => void) =>
    props.edit((draft) => {
      const target = draft.effects.find((candidate) => candidate.id === props.effectId);
      if (target) {
        change(target);
      }
    });
  const snapshot = () =>
    props.editor.state.snapshots[props.widget.id.toUpperCase()] ?? asset("gamla-linkoping.jpg");
  onMount(() => props.editor.loadSnapshot(props.widget.id));
  const portrait = () => props.editor.state.portrait;
  const [selectedPoint, setSelectedPoint] = createSignal<number | null>(null);
  const [selectedEdge, setSelectedEdge] = createSignal<number | null>(null);
  const luts = () => props.editor.state.options?.luts ?? [];
  const backgroundTypes: { id: "Transparent" | "Solid" | "Checkerboard"; name: string }[] = [
    { id: "Transparent", name: t("Transparent") },
    { id: "Solid", name: t("Solid") },
    { id: "Checkerboard", name: t("Checkerboard") },
  ];
  return (
    <Show when={effect()}>
      {(current) => (
        <>
          <Show when={current().type === "shape"}>
            <Section header={t("Corner radius")}>
              <SliderRow
                value={current().shape.cornerRadius}
                minimum={0}
                maximum={1}
                step={0.01}
                format={(value) => String(Math.trunc(value * 100))}
                onChange={(value) =>
                  edit((draft) => {
                    draft.shape.cornerRadius = value;
                  })
                }
              />
            </Section>
            <Section header={t("Border")}>
              <SliderRow
                label={t("Width")}
                value={current().shape.borderWidth}
                minimum={0}
                maximum={1}
                step={0.01}
                onChange={(value) =>
                  edit((draft) => {
                    draft.shape.borderWidth = value;
                  })
                }
              />
              <ColorRow
                label={t("Color")}
                color={current().shape.borderColor}
                onChange={(color) =>
                  edit((draft) => {
                    draft.shape.borderColor = color;
                  })
                }
              />
            </Section>
            <Section header={t("Crop")}>
              <PositioningRectangle
                image={snapshot()}
                portrait={portrait()}
                rect={{
                  x: current().shape.cropX,
                  y: current().shape.cropY,
                  width: current().shape.cropWidth,
                  height: current().shape.cropHeight,
                }}
                onChange={(rect) =>
                  edit((draft) => {
                    draft.shape.cropX = rect.x;
                    draft.shape.cropY = rect.y;
                    draft.shape.cropWidth = rect.width;
                    draft.shape.cropHeight = rect.height;
                  })
                }
              />
              <ToggleRow
                label={t("Enabled")}
                on={current().shape.cropEnabled}
                onToggle={(on) =>
                  edit((draft) => {
                    draft.shape.cropEnabled = on;
                  })
                }
              />
            </Section>
          </Show>
          <Show when={current().type === "removeBackground"}>
            <Section header={t("Color range")}>
              <ColorRow
                label={t("From")}
                color={current().removeBackground.from}
                onChange={(color) =>
                  edit((draft) => {
                    draft.removeBackground.from = color;
                  })
                }
              />
              <ColorRow
                label={t("To")}
                color={current().removeBackground.to}
                onChange={(color) =>
                  edit((draft) => {
                    draft.removeBackground.to = color;
                  })
                }
              />
            </Section>
          </Show>
          <Show when={current().type === "dewarp360"}>
            <Section header={t("Pan, tilt and zoom")}>
              <PlainRow>
                <Icon name="arrow-left" class="size-[18px] shrink-0" />
                <input
                  type="range"
                  class="min-w-0 flex-1 accent-white"
                  min={-180}
                  max={180}
                  step={1}
                  value={current().dewarp360.pan}
                  onInput={(event) =>
                    edit((draft) => {
                      draft.dewarp360.pan = Number(event.currentTarget.value);
                    })
                  }
                />
                <Icon name="arrow-right" class="size-[18px] shrink-0" />
              </PlainRow>
              <PlainRow>
                <Icon name="arrow-down" class="size-[18px] shrink-0" />
                <input
                  type="range"
                  class="min-w-0 flex-1 accent-white"
                  min={-90}
                  max={90}
                  step={1}
                  value={current().dewarp360.tilt}
                  onInput={(event) =>
                    edit((draft) => {
                      draft.dewarp360.tilt = Number(event.currentTarget.value);
                    })
                  }
                />
                <Icon name="arrow-up" class="size-[18px] shrink-0" />
              </PlainRow>
              <PlainRow>
                <Icon name="zoom-out" class="size-[18px] shrink-0" />
                <input
                  type="range"
                  class="min-w-0 flex-1 accent-white"
                  min={30}
                  max={170}
                  step={1}
                  value={Math.round(inverseFieldOfView(current().dewarp360.zoom))}
                  onInput={(event) =>
                    edit((draft) => {
                      draft.dewarp360.zoom = zoomOfInverseFieldOfView(
                        Number(event.currentTarget.value),
                      );
                    })
                  }
                />
                <Icon name="zoom-in" class="size-[18px] shrink-0" />
              </PlainRow>
            </Section>
          </Show>
          <Show when={current().type === "anamorphicLens"}>
            <Section>
              <LinkRow
                label={t("Desqueeze factor")}
                value={`${swiftDouble(current().anamorphicLens.scale)}x`}
                onClick={() =>
                  props.stack.push({
                    title: t("Desqueeze factor"),
                    render: () => (
                      <TextEditPage
                        value={swiftDouble(current().anamorphicLens.scale)}
                        validate={(text) => {
                          const scale = Number(text.trim());
                          if (text.trim() === "" || Number.isNaN(scale)) {
                            return t("Not a number");
                          }
                          if (scale <= 0) {
                            return t("Too small");
                          }
                          if (scale > 10) {
                            return t("Too big");
                          }
                          return null;
                        }}
                        onSubmit={(text) =>
                          edit((draft) => {
                            draft.anamorphicLens.scale = Number(text);
                          })
                        }
                        onDone={() => props.stack.pop()}
                      />
                    ),
                  })
                }
              />
            </Section>
          </Show>
          <Show when={current().type === "lut"}>
            <Section header={t("LUT")}>
              {[{ id: "", name: t("-- None --") }, ...luts()].map((lut) => (
                <button
                  type="button"
                  class="flex min-h-[44px] w-full items-center gap-3 px-4 py-2 text-left text-[17px]"
                  onClick={() =>
                    edit((draft) => {
                      draft.lut.lut = lut.id === "" ? null : lut.id;
                    })
                  }
                >
                  <span class="min-w-0 flex-1 truncate">{lut.name}</span>
                  <Show
                    when={
                      lut.id === "" ? current().lut.lut === null : sameId(current().lut.lut, lut.id)
                    }
                  >
                    <Icon name="check" class="size-[18px] text-phone-tint" />
                  </Show>
                </button>
              ))}
            </Section>
          </Show>
          <Show when={current().type === "opacity"}>
            <Section>
              <SliderRow
                value={current().opacity.opacity}
                minimum={0}
                maximum={1}
                step={0.01}
                onChange={(value) =>
                  edit((draft) => {
                    draft.opacity.opacity = value;
                  })
                }
              />
            </Section>
          </Show>
          <Show when={current().type === "mask"}>
            <Section header={t("Shape")}>
              <div class="px-4 py-3">
                <MaskCanvas
                  image={snapshot()}
                  portrait={portrait()}
                  points={current().mask.points}
                  tension={current().mask.tension}
                  selectedPoint={selectedPoint()}
                  selectedEdge={selectedEdge()}
                  onSelect={(point, edge) => {
                    setSelectedPoint(point);
                    setSelectedEdge(edge);
                  }}
                  onChange={(points) =>
                    edit((draft) => {
                      draft.mask.points = points;
                    })
                  }
                  onRefresh={() => props.editor.loadSnapshot(props.widget.id, true)}
                />
              </div>
              <Show when={selectedPoint() !== null && current().mask.points[selectedPoint()!]}>
                {(point) => (
                  <>
                    {(["x", "y"] as const).map((axis) => (
                      <PlainRow>
                        <span>{axis.toUpperCase()}</span>
                        <input
                          type="text"
                          inputmode="decimal"
                          class="min-w-0 flex-1 bg-transparent text-right outline-none"
                          value={point()[axis].toFixed(1)}
                          onChange={(event) => {
                            const value = Number(event.currentTarget.value);
                            if (Number.isFinite(value)) {
                              edit((draft) => {
                                draft.mask.points[selectedPoint()!]![axis] = Math.min(
                                  100,
                                  Math.max(0, value),
                                );
                              });
                            }
                          }}
                        />
                        <button
                          type="button"
                          class="text-phone-tint"
                          onClick={() =>
                            edit((draft) => {
                              const target = draft.mask.points[selectedPoint()!]!;
                              target[axis] = Math.max(0, target[axis] - 0.1);
                            })
                          }
                        >
                          <Icon name="minus-circle" class="size-[26px]" />
                        </button>
                        <button
                          type="button"
                          class="text-phone-tint"
                          onClick={() =>
                            edit((draft) => {
                              const target = draft.mask.points[selectedPoint()!]!;
                              target[axis] = Math.min(100, target[axis] + 0.1);
                            })
                          }
                        >
                          <Icon name="plus-circle" class="size-[26px]" />
                        </button>
                      </PlainRow>
                    ))}

                    <ButtonRow
                      label={t("Delete point")}
                      destructive
                      disabled={current().mask.points.length <= maskMinimumPoints}
                      onClick={() => {
                        const index = selectedPoint()!;
                        setSelectedPoint(null);
                        edit((draft) => {
                          draft.mask.points.splice(index, 1);
                        });
                      }}
                    />
                  </>
                )}
              </Show>
              <Show when={selectedPoint() === null && selectedEdge() !== null}>
                <ButtonRow
                  label={t("Create point")}
                  onClick={() => {
                    const edge = selectedEdge()!;
                    const points = current().mask.points;
                    const first = points[edge]!;
                    const second = points[(edge + 1) % points.length]!;
                    edit((draft) => {
                      draft.mask.points.splice(edge + 1, 0, {
                        x: (first.x + second.x) / 2,
                        y: (first.y + second.y) / 2,
                      });
                    });
                    setSelectedEdge(null);
                    setSelectedPoint(edge + 1);
                  }}
                />
              </Show>
            </Section>
            <Section>
              <ToggleRow
                label={t("Inverted")}
                on={current().mask.inverted}
                onToggle={(on) =>
                  edit((draft) => {
                    draft.mask.inverted = on;
                  })
                }
              />
              <SliderRow
                label={t("Smoothness")}
                value={current().mask.tension}
                minimum={0}
                maximum={0.5}
                step={0.01}
                onChange={(value) =>
                  edit((draft) => {
                    draft.mask.tension = value;
                  })
                }
              />
            </Section>
            <Section header={t("Background")}>
              <MenuRow
                label={t("Type")}
                items={backgroundTypes}
                selected={current().mask.backgroundType}
                onPick={(id) =>
                  edit((draft) => {
                    draft.mask.backgroundType = id as "Transparent" | "Solid" | "Checkerboard";
                  })
                }
              />
              <Show when={current().mask.backgroundType !== "Transparent"}>
                <ColorRow
                  label={t("Color")}
                  color={current().mask.backgroundColor}
                  onChange={(color) =>
                    edit((draft) => {
                      draft.mask.backgroundColor = color;
                    })
                  }
                />
              </Show>
              <Show when={current().mask.backgroundType === "Checkerboard"}>
                <ColorRow
                  label={t("Color 2")}
                  color={current().mask.backgroundColor2}
                  onChange={(color) =>
                    edit((draft) => {
                      draft.mask.backgroundColor2 = color;
                    })
                  }
                />
              </Show>
            </Section>
          </Show>
        </>
      )}
    </Show>
  );
}

export function EffectsSection(props: WidgetFormProps) {
  const openEffect = (effect: SettingsVideoEffect) =>
    props.stack.push({
      title: effectTypeName(effect.type),
      render: () => <EffectPage {...props} effectId={effect.id} />,
    });
  return (
    <Section
      header={t("Effects")}
      footer={t("Drag an effect to reorder it, and delete it with its trash button.")}
    >
      <ReorderableList
        items={props.widget.effects}
        render={(effect) => (
          <div class="flex min-h-[44px] items-center gap-2 pr-2 pl-3 text-[17px]">
            <button
              type="button"
              class="flex min-w-0 flex-1 items-center gap-2 text-left"
              disabled={!hasPage(effect.type)}
              onClick={() => openEffect(effect)}
            >
              <span class="min-w-0 flex-1 truncate">{effectTypeName(effect.type)}</span>
              <Show when={hasPage(effect.type)}>
                <Icon name="chevron-right" class="size-[16px] shrink-0 text-phone-border" />
              </Show>
            </button>
            <button
              type="button"
              role="switch"
              aria-checked={effect.enabled}
              aria-label={effectTypeName(effect.type)}
              class={`relative h-[31px] w-[51px] shrink-0 rounded-full transition-colors ${
                effect.enabled ? "bg-phone-switch" : "bg-phone-separator"
              }`}
              onClick={() =>
                props.edit((draft) => {
                  const target = draft.effects.find((candidate) => candidate.id === effect.id);
                  if (target) {
                    target.enabled = !target.enabled;
                  }
                })
              }
            >
              <span
                class={`absolute top-[2px] left-[2px] size-[27px] rounded-full bg-white shadow transition-transform ${
                  effect.enabled ? "translate-x-[20px]" : ""
                }`}
              />
            </button>
          </div>
        )}
        onMove={(from, to) =>
          props.edit((draft) => {
            const [moved] = draft.effects.splice(from, 1);
            draft.effects.splice(to, 0, moved!);
          })
        }
        onDelete={(effect) =>
          props.edit((draft) => {
            draft.effects = draft.effects.filter((candidate) => candidate.id !== effect.id);
          })
        }
      />
      <ButtonRow
        label={t("Create")}
        onClick={() => props.editor.set("effectWizard", props.widget.id)}
      />
    </Section>
  );
}
