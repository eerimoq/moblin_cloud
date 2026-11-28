import { createEffect, createSignal } from "solid-js";
import { parseTime, timeText } from "../data/youtube";
import { t } from "../i18n";
import { inputClass } from "./PageParts";

function secondsBetween(start: string, end: string) {
  const from = parseTime(start);
  const to = parseTime(end);
  return from !== null && to !== null && to > from ? to - from : null;
}

export function createTimeRange(options: {
  start?: string;
  end?: string;
  length: number;
  longest: () => number;
  tooLong: () => string;
  off: () => boolean;
}) {
  const [start, setStart] = createSignal(options.start ?? "0:00");
  const [end, setEnd] = createSignal(
    options.end ?? timeText((parseTime(start()) ?? 0) + options.length),
  );
  const seconds = () => secondsBetween(start(), end());
  let length = seconds() ?? options.length;

  const startError = () =>
    !options.off() && parseTime(start()) === null ? t("Enter a time like 1:30.") : "";
  const endError = () => {
    if (options.off()) {
      return "";
    }
    const to = parseTime(end());
    if (to === null) {
      return t("Enter a time like 1:30.");
    }
    const from = parseTime(start());
    if (from === null) {
      return "";
    }
    if (to <= from) {
      return t("Enter an end time after the start time.");
    }
    return to - from > options.longest() ? options.tooLong() : "";
  };

  const changeStart = (value: string) => {
    setStart(value);
    const from = parseTime(value);
    if (from !== null) {
      setEnd(timeText(from + length));
    }
  };

  const changeEnd = (value: string) => {
    setEnd(value);
    length = secondsBetween(start(), value) ?? length;
  };

  return {
    start,
    end,
    seconds,
    off: options.off,
    startError,
    endError,
    changeStart,
    changeEnd,
  };
}

export type TimeRange = ReturnType<typeof createTimeRange>;

export function TimeRangeInputs(props: { range: TimeRange }) {
  let startInput: HTMLInputElement | undefined;
  let endInput: HTMLInputElement | undefined;

  createEffect(
    () => props.range.startError(),
    (error) => {
      startInput?.setCustomValidity(error);
    },
  );
  createEffect(
    () => props.range.endError(),
    (error) => {
      endInput?.setCustomValidity(error);
    },
  );

  return (
    <div class="flex flex-wrap gap-3">
      <label class="flex flex-col gap-2">
        <span class="text-[15px] font-semibold text-ink-2">{t("Start at")}</span>
        <input
          ref={(element) => (startInput = element)}
          type="text"
          inputmode="numeric"
          required={!props.range.off()}
          maxlength={8}
          value={props.range.start()}
          onInput={(event) => props.range.changeStart(event.currentTarget.value)}
          placeholder="0:00"
          autocomplete="off"
          class={[inputClass, "max-w-32"]}
        />
      </label>
      <label class="flex flex-col gap-2">
        <span class="text-[15px] font-semibold text-ink-2">{t("End at")}</span>
        <input
          ref={(element) => (endInput = element)}
          type="text"
          inputmode="numeric"
          required={!props.range.off()}
          maxlength={8}
          value={props.range.end()}
          onInput={(event) => props.range.changeEnd(event.currentTarget.value)}
          placeholder="0:30"
          autocomplete="off"
          class={[inputClass, "max-w-32"]}
        />
      </label>
    </div>
  );
}
