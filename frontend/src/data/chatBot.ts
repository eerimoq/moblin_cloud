/**
 * Chat bot commands, grouped from
 * https://github.com/eerimoq/moblin/blob/main/docs/chat-bot-help.md
 *
 * Commands are written out exactly as typed in chat so they can be copied.
 * Placeholders are written as <name> and optional parts as [<name>].
 */
import { pages } from "./links";

export type Command = {
  command: string;
  text: string;
};

export type CommandGroup = {
  title: string;
  commands: Command[];
  more?: { text: string; link: string; href: string };
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
  ["fireworks", "Apple fireworks"],
  ["balloons", "Apple balloons"],
  ["hearts", "Apple hearts"],
  ["confetti", "Apple confetti"],
  ["lasers", "Apple lasers"],
  ["rain", "Apple rain"],
  ["glasses", "glasses"],
  ["sparkle", "sparkle"],
];

export const commandGroups: CommandGroup[] = [
  {
    title: "Reactions",
    commands: reactions.map(([name, label]) => ({
      command: `!moblin reaction ${name}`,
      text: `Trigger ${label} reaction.`,
    })),
  },
  {
    title: "Filters",
    commands: filters.flatMap((name) => [
      { command: `!moblin filter ${name} on`, text: `Turn on the ${name} filter.` },
      { command: `!moblin filter ${name} off`, text: `Turn off the ${name} filter.` },
    ]),
  },
  {
    title: "Music",
    commands: [
      { command: "!moblin music add <song>", text: "Add a song. Free text search or share link." },
      { command: "!moblin music play", text: "Play." },
      { command: "!moblin music pause", text: "Pause." },
      { command: "!moblin music next [<count>]", text: "Next song(s)." },
      { command: "!moblin music previous [<count>]", text: "Previous song(s)." },
      { command: "!moblin music status", text: "Show status." },
      { command: "!moblin music clear", text: "Clear the queue." },
    ],
    more: {
      text: "Viewers can also add songs without chat, on your own",
      link: "page",
      href: pages.page,
    },
  },
  {
    title: "Chat and AI",
    commands: [
      { command: "!moblin ai ask <question>", text: "Ask AI a question." },
      {
        command: "!moblin custom <name>",
        text: "Send the text of given custom command to chat. Configure custom commands in Moblin.",
      },
      { command: "!moblin send <message>", text: "Send given message to chat as the bot." },
      { command: "!moblin help", text: "Reply with a link to the full command list." },
    ],
  },
  {
    title: "Stream",
    commands: [
      { command: "!moblin scene <name>", text: "Change scene." },
      { command: "!moblin stream start", text: "Start streaming." },
      { command: "!moblin stream stop", text: "Stop streaming." },
      { command: "!moblin stream title <title>", text: "Set stream title." },
      { command: "!moblin stream category <name>", text: "Set stream category." },
      { command: "!moblin twitch raid <channel>", text: "Raid given Twitch channel." },
      { command: "!moblin zoom <x>", text: "Set zoom for the current camera." },
      { command: "!moblin torch on", text: "Turn the torch on." },
      { command: "!moblin torch off", text: "Turn the torch off." },
      { command: "!moblin torch level <level>", text: "Set torch level, from 1 to 100." },
      {
        command: "!moblin vibrate on [<seconds>] [<intensity>] [<sharpness>]",
        text: "Vibrate the phone. Until turned off if no seconds is given. Intensity and sharpness from 1 to 100, default 100 and 50.",
      },
      { command: "!moblin vibrate off", text: "Stop vibrating the phone." },
      { command: "!moblin gimbal preset <name>", text: "Move to given gimbal preset." },
      { command: "!moblin obs fix", text: "Fix OBS input." },
      { command: "!moblin snapshot", text: "Take snapshot." },
      {
        command: "!moblin snapshot <message>",
        text: "Take snapshot showing given message to the streamer.",
      },
    ],
  },
  {
    title: "Audio and speech",
    commands: [
      { command: "!moblin mute", text: "Mute audio." },
      { command: "!moblin unmute", text: "Unmute audio." },
      { command: "!moblin tts on", text: "Turn on chat text to speech." },
      { command: "!moblin tts off", text: "Turn off chat text to speech." },
      { command: "!moblin say <message>", text: "Say given message." },
    ],
  },
  {
    title: "Widgets and alerts",
    commands: [
      {
        command: "!moblin alert <name>",
        text: "Trigger alerts. Configure alert names in alert widgets.",
      },
      { command: "!moblin fax <url>", text: "Fax the streamer images." },
      { command: "!moblin map zoom out", text: "Zoom out map widget temporarily." },
      { command: "!moblin widget <name> enable", text: "Enable given widget." },
      { command: "!moblin widget <name> disable", text: "Disable given widget." },
      {
        command: "!moblin widget <name> timer <number> add <seconds>",
        text: "Add time to a timer.",
      },
      { command: "!moblin widget <name> wheelofluck spin", text: "Spin the wheel of luck." },
      {
        command: "!moblin widget <name> wheelofluck options <option>...",
        text: "Replace the wheel of luck options.",
      },
      {
        command: "!moblin location data reset",
        text: "Reset distance, altitude, average speed and slope.",
      },
      { command: "!moblin location data split", text: "Reset split distance and altitude." },
    ],
  },
  {
    title: "Macros",
    commands: [
      { command: "!moblin macro run <name>", text: "Run given macro." },
      { command: "!moblin macro cancel <name>", text: "Cancel given macro." },
    ],
  },
  {
    title: "Tesla",
    commands: [
      { command: "!moblin tesla trunk open", text: "Open the trunk." },
      { command: "!moblin tesla trunk close", text: "Close the trunk." },
      { command: "!moblin tesla media next", text: "Next track." },
      { command: "!moblin tesla media previous", text: "Previous track." },
      { command: "!moblin tesla media toggle-playback", text: "Toggle playback." },
    ],
  },
];
