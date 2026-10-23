/**
 * Chat bot commands, grouped from
 * https://github.com/eerimoq/moblin/blob/main/docs/chat-bot-help.md
 *
 * Commands are written out exactly as typed in chat so they can be copied.
 * Placeholders are written as <name> and optional parts as [<name>].
 */
import { t } from "../i18n";

export type Command = {
  command: string;
  text: string;
};

export type CommandGroup = {
  title: string;
  commands: Command[];
};

const filters = [
  "movie",
  "grayscale",
  "sepia",
  "triple",
  "twin",
  "pixellate",
  "4:3",
  "whirlpool",
  "pinch",
];

const reactions: [string, string][] = [
  ["fireworks", t("Trigger Apple fireworks reaction.")],
  ["balloons", t("Trigger Apple balloons reaction.")],
  ["hearts", t("Trigger Apple hearts reaction.")],
  ["confetti", t("Trigger Apple confetti reaction.")],
  ["lasers", t("Trigger Apple lasers reaction.")],
  ["rain", t("Trigger Apple rain reaction.")],
  ["glasses", t("Trigger glasses reaction.")],
  ["sparkle", t("Trigger sparkle reaction.")],
];

export const commandGroups: CommandGroup[] = [
  {
    title: t("Reactions"),
    commands: reactions.map(([name, text]) => ({
      command: `!moblin reaction ${name}`,
      text,
    })),
  },
  {
    title: t("Filters"),
    commands: filters.flatMap((name) => [
      { command: `!moblin filter ${name} on`, text: t("Turn on the {name} filter.", { name }) },
      { command: `!moblin filter ${name} off`, text: t("Turn off the {name} filter.", { name }) },
    ]),
  },
  {
    title: t("Music"),
    commands: [
      {
        command: "!moblin music add <song>",
        text: t("Add a song. Free text search or share link."),
      },
      { command: "!moblin music play", text: t("Play.") },
      { command: "!moblin music pause", text: t("Pause.") },
      { command: "!moblin music next [<count>]", text: t("Next song(s).") },
      { command: "!moblin music previous [<count>]", text: t("Previous song(s).") },
      { command: "!moblin music status", text: t("Show status.") },
      { command: "!moblin music clear", text: t("Clear the queue.") },
    ],
  },
  {
    title: t("Chat and AI"),
    commands: [
      { command: "!moblin ai ask <question>", text: t("Ask AI a question.") },
      {
        command: "!moblin custom <name>",
        text: t(
          "Send the text of given custom command to chat. Configure custom commands in Moblin.",
        ),
      },
      { command: "!moblin send <message>", text: t("Send given message to chat as the bot.") },
      { command: "!moblin help", text: t("Reply with a link to the full command list.") },
    ],
  },
  {
    title: t("Stream"),
    commands: [
      { command: "!moblin scene <name>", text: t("Change scene.") },
      { command: "!moblin stream start", text: t("Start streaming.") },
      { command: "!moblin stream stop", text: t("Stop streaming.") },
      { command: "!moblin stream title <title>", text: t("Set stream title.") },
      { command: "!moblin stream category <name>", text: t("Set stream category.") },
      { command: "!moblin twitch raid <channel>", text: t("Raid given Twitch channel.") },
      { command: "!moblin zoom <x>", text: t("Set zoom for the current camera.") },
      { command: "!moblin torch on", text: t("Turn the torch on.") },
      { command: "!moblin torch off", text: t("Turn the torch off.") },
      { command: "!moblin torch level <level>", text: t("Set torch level, from 1 to 100.") },
      {
        command: "!moblin vibrate on [<seconds>] [<intensity>] [<sharpness>]",
        text: t(
          "Vibrate the phone. Until turned off if no seconds is given. Intensity and sharpness from 1 to 100, default 100 and 50.",
        ),
      },
      { command: "!moblin vibrate off", text: t("Stop vibrating the phone.") },
      { command: "!moblin gimbal preset <name>", text: t("Move to given gimbal preset.") },
      { command: "!moblin obs fix", text: t("Fix OBS input.") },
      { command: "!moblin snapshot", text: t("Take snapshot.") },
      {
        command: "!moblin snapshot <message>",
        text: t("Take snapshot showing given message to the streamer."),
      },
    ],
  },
  {
    title: t("Audio and speech"),
    commands: [
      { command: "!moblin mute", text: t("Mute audio.") },
      { command: "!moblin unmute", text: t("Unmute audio.") },
      { command: "!moblin tts on", text: t("Turn on chat text to speech.") },
      { command: "!moblin tts off", text: t("Turn off chat text to speech.") },
      { command: "!moblin say <message>", text: t("Say given message.") },
    ],
  },
  {
    title: t("Widgets and alerts"),
    commands: [
      {
        command: "!moblin alert <name>",
        text: t("Trigger alerts. Configure alert names in alert widgets."),
      },
      { command: "!moblin fax <url>", text: t("Fax the streamer images.") },
      { command: "!moblin map zoom out", text: t("Zoom out map widget temporarily.") },
      { command: "!moblin widget <name> enable", text: t("Enable given widget.") },
      { command: "!moblin widget <name> disable", text: t("Disable given widget.") },
      {
        command: "!moblin widget <name> timer <number> add <seconds>",
        text: t("Add time to a timer."),
      },
      { command: "!moblin widget <name> wheelofluck spin", text: t("Spin the wheel of luck.") },
      {
        command: "!moblin widget <name> wheelofluck options <option>...",
        text: t("Replace the wheel of luck options."),
      },
      {
        command: "!moblin location data reset",
        text: t("Reset distance, altitude, average speed and slope."),
      },
      { command: "!moblin location data split", text: t("Reset split distance and altitude.") },
    ],
  },
  {
    title: t("Macros"),
    commands: [
      { command: "!moblin macro run <name>", text: t("Run given macro.") },
      { command: "!moblin macro cancel <name>", text: t("Cancel given macro.") },
    ],
  },
  {
    title: t("Tesla"),
    commands: [
      { command: "!moblin tesla trunk open", text: t("Open the trunk.") },
      { command: "!moblin tesla trunk close", text: t("Close the trunk.") },
      { command: "!moblin tesla media next", text: t("Next track.") },
      { command: "!moblin tesla media previous", text: t("Previous track.") },
      { command: "!moblin tesla media toggle-playback", text: t("Toggle playback.") },
    ],
  },
];
