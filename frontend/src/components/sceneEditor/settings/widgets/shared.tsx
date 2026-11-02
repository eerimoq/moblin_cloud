import { createSignal, Show } from "solid-js";
import { PickerRow, Section, SliderRow, type PickerItem } from "../../Form";
import type { Stack } from "../../Stack";
import type { Editor } from "../../store";
import {
  applyVideoSource,
  videoSourceItemId,
  videoSourceItems,
  videoSourceOf,
} from "../../videoSources";
import type {
  FontDesign,
  FontWeight,
  SettingsWidget,
  VideoSourceFields,
} from "../../../../data/moblinSettings";
import { t } from "../../../../i18n";

export type WidgetFormProps = {
  editor: Editor;
  stack: Stack;
  widget: SettingsWidget;
  edit: (change: (widget: SettingsWidget) => void) => void;
};

export const fontStyleName = (family: string, fontName: string) => {
  const prefix = family.replaceAll(" ", "");
  const name = fontName.replaceAll("-", "");
  if (name.startsWith(prefix)) {
    const suffix = name.slice(prefix.length);
    return suffix === "" ? "Regular" : suffix;
  }
  return fontName;
};

export const fontDesignItems = (): { id: FontDesign; name: string }[] => [
  { id: "Default", name: t("Default") },
  { id: "Serif", name: t("Serif") },
  { id: "Rounded", name: t("Rounded") },
  { id: "Monospaced", name: t("Monospaced") },
];

export const fontWeightItems = (): { id: FontWeight; name: string }[] => [
  { id: "Regular", name: t("Regular") },
  { id: "Light", name: t("Light") },
  { id: "Bold", name: t("Bold") },
];

export function FontRows(props: {
  editor: Editor;
  stack: Stack;
  family: string | null;
  style: string;
  onChange: (family: string | null, style: string) => void;
}) {
  const families = () => props.editor.state.options?.fonts ?? [];
  const styles = () => families().find((font) => font.family === props.family)?.styles ?? [];
  const familyItems = (): PickerItem[] => [
    { id: "", name: t("System") },
    ...families().map((font) => ({ id: font.family, name: font.family })),
  ];
  return (
    <>
      <PickerRow
        stack={props.stack}
        label={t("Family")}
        items={familyItems()}
        selected={props.family ?? ""}
        onPick={(id) => props.onChange(id === "" ? null : id, id === "" ? "" : (styles()[0] ?? ""))}
      />
      <Show when={props.family}>
        {(family) => (
          <PickerRow
            stack={props.stack}
            label={t("Style")}
            items={styles().map((style) => ({ id: style, name: fontStyleName(family(), style) }))}
            selected={props.style}
            disabled={styles().length === 1}
            onPick={(id) => props.onChange(family(), id)}
          />
        )}
      </Show>
    </>
  );
}

export const validUrl = (value: string) => {
  const url = value.trim();
  if (url === "") {
    return null;
  }
  if (/\s/.test(url)) {
    return t("Malformed URL");
  }
  const match = /^([A-Za-z][A-Za-z0-9+.-]*):(.*)$/.exec(url);
  const scheme = match?.[1] ?? null;
  const rest = match?.[2] ?? url;
  const host = rest.startsWith("//") ? (rest.slice(2).split(/[/?#]/)[0] ?? "") : "";
  if (host === "") {
    return t("Host missing");
  }
  if (scheme === null) {
    return t("Scheme missing");
  }
  if (scheme !== "http" && scheme !== "https") {
    return t("Unsupported scheme {scheme}", { scheme });
  }
  return null;
};

export function ModelSection(props: { name: string; footers: string[] }) {
  return (
    <Section
      header={t("Model")}
      footer={
        <span class="flex flex-col">
          {props.footers.map((footer) => (
            <span>{footer}</span>
          ))}
        </span>
      }
    >
      <div class="flex min-h-[44px] w-full items-center justify-center px-4 py-2 text-[17px] text-phone-border">
        <span class="min-w-0 truncate">{props.name === "" ? t("Select model") : props.name}</span>
      </div>
    </Section>
  );
}

export function VideoSourceRow(props: {
  editor: Editor;
  stack: Stack;
  fields: VideoSourceFields;
  onChange: (change: (fields: VideoSourceFields) => void) => void;
}) {
  return (
    <PickerRow
      stack={props.stack}
      label={t("Video source")}
      items={videoSourceItems(props.editor.state.videoSources)}
      selected={videoSourceItemId(props.fields)}
      onPick={(id) => {
        const source = videoSourceOf(props.editor.state.videoSources, id);
        if (source) {
          props.onChange((fields) => applyVideoSource(fields, source));
        }
      }}
    />
  );
}

export function SensitivitySection(props: {
  sensitivity: { mouth: number; eyes: number };
  onChange: (sensitivity: { mouth: number; eyes: number }) => void;
}) {
  return (
    <Section header={t("Sensitivity")}>
      <SliderRow
        label={t("Mouth")}
        value={props.sensitivity.mouth}
        minimum={0.05}
        maximum={3}
        step={0.01}
        onChange={(mouth) => props.onChange({ ...props.sensitivity, mouth })}
      />
      <SliderRow
        label={t("Eyes")}
        value={props.sensitivity.eyes}
        minimum={0.05}
        maximum={5}
        step={0.01}
        onChange={(eyes) => props.onChange({ ...props.sensitivity, eyes })}
      />
    </Section>
  );
}

export const pickerItems = (values: number[], suffix = "") =>
  values.map((value) => ({ id: String(value), name: `${value}${suffix}` }));

export function Lines(props: {
  header: string;
  value: string;
  placeholder?: string;
  onChange: (value: string) => void;
}) {
  const [text, setText] = createSignal<string | null>(null);
  return (
    <Section header={props.header}>
      <textarea
        class="min-h-[110px] w-full resize-none bg-transparent px-4 py-2 text-[17px] outline-none placeholder:text-phone-border"
        value={text() ?? props.value}
        placeholder={props.placeholder}
        onInput={(event) => setText(event.currentTarget.value)}
        onBlur={() => {
          const typed = text();
          if (typed !== null) {
            props.onChange(typed);
            setText(null);
          }
        }}
      />
    </Section>
  );
}

export const formatShortDuration = (seconds: number) => {
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = seconds % 60;
  const parts = [
    days === 1 ? t("1 day") : days > 1 ? t("{count} days", { count: days }) : null,
    hours > 0 ? t("{count} hr", { count: hours }) : null,
    minutes > 0 ? t("{count} min", { count: minutes }) : null,
    rest > 0 ? t("{count} sec", { count: rest }) : null,
  ].filter((part) => part !== null);
  return parts.length === 0 ? t("{count} sec", { count: 0 }) : parts.join(", ");
};
