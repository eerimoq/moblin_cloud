import { For, Show, type JSX } from "solid-js";
import { ask } from "../../../Modal";
import {
  ColorRow,
  LinkRow,
  MenuRow,
  NumberRow,
  PlainRow,
  Section,
  SliderRow,
  ToggleRow,
} from "../../Form";
import { FontRows, fontDesignItems, fontWeightItems, type WidgetFormProps } from "./shared";
import type { Stack } from "../../Stack";
import type { Editor } from "../../store";
import {
  newUuid,
  type FontDesign,
  type FontWeight,
  type HorizontalAlignment,
  type SettingsWidgetText,
} from "../../../../data/moblinSettings";
import { t, tRich } from "../../../../i18n";

const suggestionWeather = "{conditions} {temperature}";
const suggestionTime = "🕑 {shortTime}";
const suggestionCity = "{countryFlag} {city}";
const suggestionMovement = "📏 {distance} 💨 {speed} 🏔️ {altitude}";

const suggestions = (isPhone: boolean): { name: string; text: string }[] => [
  {
    name: t("Travel"),
    text: `${suggestionWeather}\n${suggestionTime}\n${suggestionCity}\n${suggestionMovement}`,
  },
  { name: t("Weather"), text: suggestionWeather },
  { name: t("Time"), text: suggestionTime },
  { name: t("Date"), text: "📅 {date}" },
  { name: t("Full date"), text: "📅 {fullDate}" },
  { name: t("Timer"), text: "⏳ {timer}" },
  { name: t("Stopwatch"), text: "⏱️ {stopwatch}" },
  { name: t("City"), text: suggestionCity },
  { name: t("Country"), text: "{countryFlag} {country}" },
  { name: t("Movement"), text: suggestionMovement },
  ...(isPhone ? [{ name: t("Apple workout heart rate"), text: "♥️ {heartRate}" }] : []),
  { name: t("Apple workout step count"), text: "🚶{stepCount}" },
  { name: t("Subtitles"), text: "{subtitles}" },
  { name: t("Muted"), text: "{muted}" },
  { name: t("Debug"), text: "{time}\n{bitrateAndTotal}\n{debugOverlay}" },
  { name: t("Tesla"), text: "🚗 Tesla\n⚙️ {teslaDrive}\n🔋 {teslaBatteryLevel}\n🔈 {teslaMedia}" },
  { name: t("Racing"), text: "🏎️ Racing 🏎️\n{lapTimes}" },
];

const speedUnits = (): [string, string][] => [
  [`${t("Meters per second")} [m/s]`, "m/s"],
  [`${t("Kilometers per hour")} [km/h]`, "km/h"],
  [`${t("Miles per hour")} [mph]`, "mph"],
];

const temperatureUnits = (): [string, string][] => [
  [`${t("Kelvin")} [K]`, "k"],
  [`${t("Celsius")} [°C]`, "c"],
  [`${t("Fahrenheit")} [°F]`, "f"],
];

const lengthUnits = (): [string, string][] => [
  [`${t("Meters")} [m]`, "m"],
  [`${t("Kilometers")} [km]`, "km"],
  [`${t("Feet")} [ft]`, "ft"],
  [`${t("Yards")} [yd]`, "yd"],
  [`${t("Miles")} [mi]`, "mi"],
  [`${t("Nautical miles")} [NM]`, "nmi"],
  [`${t("Light years")} [ly]`, "ly"],
];

type Variable = {
  title: string;
  description: string;
  units?: [string, string][];
  variable?: string;
};

function VariableRow(props: {
  editor: Editor;
  variable: Variable;
  stack: Stack;
  onAppend: (value: string) => void;
}) {
  return (
    <div class="flex flex-col px-4 py-2">
      <button
        type="button"
        class="text-left text-[20px] text-phone-tint"
        onClick={() => {
          if (props.variable.units) {
            const units = props.variable.units;
            const name = props.variable.variable!;
            props.stack.push({
              title: t("Unit"),
              render: () => (
                <Section>
                  <For each={units}>
                    {([label, symbol]) => (
                      <button
                        type="button"
                        class="flex min-h-[44px] w-full items-center px-4 py-2 text-left text-[17px] text-phone-tint"
                        onClick={() => {
                          props.onAppend(`{${name}:${symbol}}`);
                          props.stack.pop();
                        }}
                      >
                        {label}
                      </button>
                    )}
                  </For>
                </Section>
              ),
            });
          } else {
            props.onAppend(props.variable.title);
          }
        }}
      >
        {props.variable.title}
      </button>
      <span class="text-[17px]">{props.variable.description}</span>
    </div>
  );
}

const now = () => new Date();

type VariableSection = { header?: string; footer?: JSX.Element; variables: Variable[] };

const variablePages = (editor: Editor): { title: string; sections: VariableSection[] }[] => [
  {
    title: t("General"),
    sections: [
      {
        variables: [
          { title: "{checkbox}", description: t("Show a checkbox") },
          { title: "{rating}", description: t("Show a 0-5 rating") },
          { title: "{muted}", description: t("Show muted") },
          { title: "{browserTitle}", description: t("Show browser title") },
          { title: "{gForce}", description: t("Show G-force") },
          { title: "{gForceRecentMax}", description: t("Show recent max G-force") },
          { title: "{gForceMax}", description: t("Show max G-force") },
        ],
      },
    ],
  },
  {
    title: t("Time"),
    sections: [
      {
        variables: [
          {
            title: "{time}",
            description: t("Show time as {time}", {
              time: now().toLocaleTimeString([], {
                hour: "numeric",
                minute: "2-digit",
                second: "2-digit",
              }),
            }),
          },
          {
            title: "{shortTime}",
            description: t("Show time as {time}", {
              time: now().toLocaleTimeString([], { hour: "numeric", minute: "2-digit" }),
            }),
          },
          {
            title: "{date}",
            description: t("Show date as {date}", { date: now().toLocaleDateString() }),
          },
          {
            title: "{fullDate}",
            description: t("Show date as {date}", {
              date: now().toLocaleDateString([], {
                weekday: "long",
                year: "numeric",
                month: "long",
                day: "numeric",
              }),
            }),
          },
          { title: "{timer}", description: t("Show a timer") },
          { title: "{stopwatch}", description: t("Show a stopwatch") },
          { title: "{lapTimes}", description: t("Show lap times") },
        ],
      },
    ],
  },
  {
    title: t("Location"),
    sections: [
      {
        variables: [
          { title: "{country}", description: t("Show country") },
          { title: "{countryFlag}", description: t("Show country flag") },
          { title: "{state}", description: t("Show state") },
          { title: "{area}", description: t("Show area") },
          { title: "{city}", description: t("Show city") },
          { title: "{neighborhood}", description: t("Show neighborhood") },
          { title: "{speed}", description: t("Show speed") },
          {
            title: "{speed:<unit>}",
            description: t("Show speed in given unit"),
            units: speedUnits(),
            variable: "speed",
          },
          { title: "{averageSpeed}", description: t("Show average speed") },
          {
            title: "{averageSpeed:<unit>}",
            description: t("Show average speed in given unit"),
            units: speedUnits(),
            variable: "averageSpeed",
          },
          { title: "{altitude}", description: t("Show altitude") },
          {
            title: "{altitude:<unit>}",
            description: t("Show altitude in given unit"),
            units: lengthUnits(),
            variable: "altitude",
          },
          { title: "{distance}", description: t("Show distance") },
          {
            title: "{distance:<unit>}",
            description: t("Show distance in given unit"),
            units: lengthUnits(),
            variable: "distance",
          },
          { title: "{splitDistance}", description: t("Show split distance") },
          {
            title: "{splitDistance:<unit>}",
            description: t("Show split distance in given unit"),
            units: lengthUnits(),
            variable: "splitDistance",
          },
          { title: "{altitudeAscent}", description: t("Show altitude ascent") },
          {
            title: "{altitudeAscent:<unit>}",
            description: t("Show altitude ascent in given unit"),
            units: lengthUnits(),
            variable: "altitudeAscent",
          },
          { title: "{altitudeDescent}", description: t("Show altitude descent") },
          {
            title: "{altitudeDescent:<unit>}",
            description: t("Show altitude descent in given unit"),
            units: lengthUnits(),
            variable: "altitudeDescent",
          },
          { title: "{splitAltitudeAscent}", description: t("Show split altitude ascent") },
          {
            title: "{splitAltitudeAscent:<unit>}",
            description: t("Show split altitude ascent in given unit"),
            units: lengthUnits(),
            variable: "splitAltitudeAscent",
          },
          { title: "{splitAltitudeDescent}", description: t("Show split altitude descent") },
          {
            title: "{splitAltitudeDescent:<unit>}",
            description: t("Show split altitude descent in given unit"),
            units: lengthUnits(),
            variable: "splitAltitudeDescent",
          },
          { title: "{slope}", description: t("Show slope") },
        ],
      },
    ],
  },
  {
    title: t("Weather"),
    sections: [
      {
        variables: [
          { title: "{conditions}", description: t("Show conditions") },
          { title: "{temperature}", description: t("Show temperature") },
          {
            title: "{temperature:<unit>}",
            description: t("Show temperature in given unit"),
            units: temperatureUnits(),
            variable: "temperature",
          },
          { title: "{feelsLikeTemperature}", description: t("Show feels like temperature") },
          {
            title: "{feelsLikeTemperature:<unit>}",
            description: t("Show feels like temperature in given unit"),
            units: temperatureUnits(),
            variable: "feelsLikeTemperature",
          },
          { title: "{wind}", description: t("Show wind") },
          {
            title: "{wind:<unit>}",
            description: t("Show wind in given unit"),
            units: speedUnits(),
            variable: "wind",
          },
        ],
        footer: tRich("Weather data is provided by Apple Weather. {link}.", {
          link: (
            <a
              class="underline"
              href="https://weatherkit.apple.com/legal-attribution.html"
              target="_blank"
              rel="noopener"
            >
              {t("Legal information")}
            </a>
          ),
        }),
      },
    ],
  },
  {
    title: t("Language"),
    sections: [
      {
        variables: [
          { title: "{subtitles}", description: t("Show subtitles in app language") },
          {
            title: "{subtitles:<language-identifier>}",
            description: t("Show subtitles in given language"),
          },
        ],
      },
    ],
  },
  {
    title: t("Workout"),
    sections: [
      {
        header: t("Apple workout"),
        variables: [
          ...((editor.state.options?.isPhone ?? true)
            ? [{ title: "{heartRate}", description: t("Show heart rate.") }]
            : []),
          { title: "{stepCount}", description: t("Show step count.") },
          { title: "{workoutDistance}", description: t("Show distance.") },
        ],
      },
      {
        header: t("Other devices"),
        variables: [
          ...(editor.state.options?.workoutDevices ?? []).flatMap((device) => [
            {
              title: `{heartRate:${device.name}}`,
              description: t('Show heart rate for heart rate device called "{name}"', {
                name: device.name,
              }),
            },
            { title: `{runningPace:${device.name}}`, description: t("Show running pace") },
            { title: `{runningCadence:${device.name}}`, description: t("Show running cadence") },
            {
              title: `{runningDistance:${device.name}}`,
              description: t("Show running distance"),
            },
          ]),
          { title: "{cyclingPower}", description: t("Show cycling power") },
          { title: "{cyclingCadence}", description: t("Show cycling cadence") },
          { title: "{cyclingSpeed}", description: t("Show cycling speed") },
        ],
      },
    ],
  },
  {
    title: t("Tesla"),
    sections: [
      {
        variables: [
          { title: "{teslaBatteryLevel}", description: t("Show Tesla battery level") },
          { title: "{teslaDrive}", description: t("Show Tesla drive information") },
          { title: "{teslaMedia}", description: t("Show Tesla media information") },
        ],
      },
    ],
  },
  {
    title: t("Streaming"),
    sections: [
      {
        variables: [
          { title: "{latestSubscriber}", description: t("Show latest subscriber") },
          { title: "{latestFollower}", description: t("Show latest follower") },
        ],
      },
    ],
  },
  {
    title: t("Debug"),
    sections: [
      {
        variables: [
          { title: "{bitrate}", description: t("Show bitrate") },
          {
            title: "{bitrateAndTotal}",
            description: t("Show bitrate and total number of bytes sent"),
          },
          { title: "{bonding}", description: t("Show bonding percentage split") },
          { title: "{resolution}", description: t("Show resolution") },
          { title: "{fps}", description: t("Show FPS") },
          { title: "{debugOverlay}", description: t("Show debug overlay (if enabled)") },
          { title: "{systemMonitor}", description: t("Show system monitor (if enabled)") },
        ],
      },
    ],
  },
];

const count = (format: string, pattern: RegExp) => (format.match(pattern) ?? []).length;

const resized = <T,>(items: T[], length: number, create: () => T) =>
  items.length > length
    ? items.slice(0, length)
    : [...items, ...Array.from({ length: length - items.length }, create)];

const temperatureUnit = "k|c|f";

const normalizedFormat = (formatString: string) =>
  formatString.replaceAll("\\n", "\n").toLowerCase();

const isWorkoutVariable = (format: string) =>
  /\{(heartrate|activeenergyburned|power|stepcount|workoutdistance)(:[^}]*)?\}/.test(format);

function TextFormatWarnings(props: { editor: Editor; value: string }) {
  return (
    <Show
      when={
        isWorkoutVariable(normalizedFormat(props.value)) &&
        !(props.editor.state.options?.workoutActive ?? true)
      }
    >
      <Section>
        <PlainRow>
          {t(
            "⚠️ Start a workout using the Workout quick button or the Start workout button on your Apple Watch to update Apple workout variables.",
          )}
        </PlainRow>
      </Section>
    </Show>
  );
}

const setFormatString = (text: SettingsWidgetText, formatString: string) => {
  text.formatString = formatString;
  const format = normalizedFormat(formatString);
  text.timers = resized(text.timers, count(format, /\{timer\}/g), () => ({
    id: newUuid(),
    delta: 5,
    endTime: 0,
  }));
  text.stopwatches = resized(text.stopwatches, count(format, /\{stopwatch\}/g), () => ({
    id: newUuid(),
    totalElapsed: 0,
    running: false,
  }));
  text.checkboxes = resized(text.checkboxes, count(format, /\{checkbox\}/g), () => ({
    id: newUuid(),
    checked: false,
  }));
  text.ratings = resized(text.ratings, count(format, /\{rating\}/g), () => ({
    id: newUuid(),
    rating: 0,
  }));
  text.lapTimes = resized(text.lapTimes, count(format, /\{laptimes\}/g), () => ({
    id: newUuid(),
    lapTimes: [],
  }));
  text.subtitles = [...format.matchAll(/\{subtitles(?::([^}]+))?\}/g)].map((match) =>
    match[1] === undefined ? {} : { identifier: match[1] },
  );
  text.needsSubtitles = text.subtitles.length > 0;
  text.needsWeather = new RegExp(
    `\\{(conditions|temperature(:(${temperatureUnit}))?|feelsliketemperature(:(${temperatureUnit}))?|wind(:(m/s|km/h|mph))?)\\}`,
  ).test(format);
  text.needsGeography = /\{(country|countryflag|state|area|city|neighborhood)\}/.test(format);
  text.needsGForce = /\{(gforce|gforcerecentmax|gforcemax)\}/.test(format);
};

function TextPage(props: WidgetFormProps) {
  const value = () => props.widget.text.formatString;
  const commit = (text: string) =>
    props.edit((widget) => {
      setFormatString(widget.text, text);
    });
  const append = (variable: string) => commit(`${value()}${variable}`);
  return (
    <>
      <Section>
        <textarea
          class="min-h-[110px] w-full resize-none bg-transparent px-4 py-2 text-[17px] outline-none placeholder:text-phone-border"
          value={value()}
          placeholder={t("My text")}
          autocapitalize="off"
          autocorrect="off"
          spellcheck={false}
          onInput={(event) => commit(event.currentTarget.value)}
        />
      </Section>
      <TextFormatWarnings editor={props.editor} value={value()} />
      <SuggestionsRow
        stack={props.stack}
        value={value()}
        isPhone={props.editor.state.options?.isPhone ?? true}
        onPick={commit}
      />
      <Section header={t("Variables")}>
        <For each={variablePages(props.editor)}>
          {(page) => (
            <LinkRow
              label={page.title}
              onClick={() =>
                props.stack.push({
                  title: page.title,
                  render: () => (
                    <For each={page.sections}>
                      {(section) => (
                        <Section header={section.header} footer={section.footer}>
                          <For each={section.variables}>
                            {(variable) => (
                              <VariableRow
                                editor={props.editor}
                                variable={variable}
                                stack={props.stack}
                                onAppend={append}
                              />
                            )}
                          </For>
                        </Section>
                      )}
                    </For>
                  ),
                })
              }
            />
          )}
        </For>
      </Section>
    </>
  );
}

export function SuggestionsRow(props: {
  stack: Stack;
  value: string;
  isPhone: boolean;
  onPick: (text: string) => void;
}) {
  return (
    <Section>
      <LinkRow
        label={t("Suggestions")}
        onClick={() =>
          props.stack.push({
            title: t("Suggestions"),
            render: () => (
              <Section>
                <For each={suggestions(props.isPhone)}>
                  {(suggestion) => (
                    <div class="flex flex-col px-4 py-2">
                      <button
                        type="button"
                        class="text-left text-[20px] text-phone-tint"
                        onClick={async () => {
                          if (
                            props.value === "" ||
                            (await ask(
                              t(
                                "Are you sure you want to replace the content of the current text widget?",
                              ),
                              t("Yes"),
                            ))
                          ) {
                            props.onPick(suggestion.text);
                            props.stack.pop();
                          }
                        }}
                      >
                        {suggestion.name}
                      </button>
                      <span class="text-[17px] whitespace-pre-line">{suggestion.text}</span>
                    </div>
                  )}
                </For>
              </Section>
            ),
          })
        }
      />
    </Section>
  );
}

export function TextForm(props: WidgetFormProps) {
  const text = () => props.widget.text;
  const designs = fontDesignItems();
  const weights = fontWeightItems();
  const alignments: { id: HorizontalAlignment; name: string }[] = [
    { id: "Leading", name: t("Leading") },
    { id: "Trailing", name: t("Trailing") },
    { id: "Center", name: t("Center") },
  ];
  return (
    <>
      <Section>
        <LinkRow
          label={t("Text")}
          value={text().formatString}
          onClick={() =>
            props.stack.push({ title: t("Text"), render: () => <TextPage {...props} /> })
          }
        />
      </Section>
      <TextFormatWarnings editor={props.editor} value={text().formatString} />
      <Section header={t("Colors")}>
        <ColorRow
          label={t("Background")}
          color={text().backgroundColor}
          opacity
          onChange={(color) =>
            props.edit((draft) => {
              draft.text.backgroundColor = color;
            })
          }
        />
        <ColorRow
          label={t("Foreground")}
          color={text().foregroundColor}
          opacity
          onChange={(color) =>
            props.edit((draft) => {
              draft.text.foregroundColor = color;
            })
          }
        />
      </Section>
      <Section header={t("Font")}>
        <SliderRow
          label={t("Size")}
          value={text().fontSize}
          minimum={10}
          maximum={200}
          step={5}
          format={(value) => String(Math.round(value))}
          onChange={(fontSize) =>
            props.edit((draft) => {
              draft.text.fontSize = Math.round(fontSize);
            })
          }
        />
        <FontRows
          editor={props.editor}
          stack={props.stack}
          family={text().fontFamily}
          style={text().fontStyle}
          onChange={(family, style) =>
            props.edit((draft) => {
              draft.text.fontFamily = family;
              draft.text.fontStyle = style;
            })
          }
        />
        <Show when={text().fontFamily === null}>
          <MenuRow
            label={t("Design")}
            items={designs}
            selected={text().fontDesign}
            onPick={(id) =>
              props.edit((draft) => {
                draft.text.fontDesign = id as FontDesign;
              })
            }
          />
          <MenuRow
            label={t("Weight")}
            items={weights}
            selected={text().fontWeight}
            onPick={(id) =>
              props.edit((draft) => {
                draft.text.fontWeight = id as FontWeight;
              })
            }
          />
          <ToggleRow
            label={t("Monospaced digits")}
            on={text().fontMonospacedDigits}
            onToggle={(on) =>
              props.edit((draft) => {
                draft.text.fontMonospacedDigits = on;
              })
            }
          />
        </Show>
      </Section>
      <Section header={t("Layout")}>
        <MenuRow
          label={t("Alignment")}
          items={alignments}
          selected={text().horizontalAlignment}
          onPick={(id) =>
            props.edit((draft) => {
              draft.text.horizontalAlignment = id as HorizontalAlignment;
            })
          }
        />
        <div class="flex items-center">
          <div class="min-w-0 flex-1">
            <NumberRow
              stack={props.stack}
              label={t("Minimum width")}
              value={text().width}
              minimum={1}
              maximum={3999}
              integer
              onSubmit={(width) =>
                props.edit((draft) => {
                  draft.text.width = width;
                })
              }
            />
          </div>
          <div class="pr-1">
            <ToggleRow
              label=""
              on={text().widthEnabled}
              onToggle={(on) =>
                props.edit((draft) => {
                  draft.text.widthEnabled = on;
                })
              }
            />
          </div>
        </div>
        <NumberRow
          stack={props.stack}
          label={t("Corner radius")}
          value={text().cornerRadius}
          minimum={0}
          maximum={999}
          integer
          onSubmit={(cornerRadius) =>
            props.edit((draft) => {
              draft.text.cornerRadius = cornerRadius;
            })
          }
        />
      </Section>
      <Section
        header={t("Delay")}
        footer={t("To show the widget in sync with high latency cameras.")}
      >
        <SliderRow
          value={text().delay}
          minimum={0}
          maximum={10}
          step={0.5}
          format={(value) => value.toFixed(1)}
          onChange={(delay) =>
            props.edit((draft) => {
              draft.text.delay = delay;
            })
          }
        />
      </Section>
    </>
  );
}
