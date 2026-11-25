import { describe, expect, it } from "vitest";
import {
  mediaLink,
  parseTime,
  timeText,
  youTubeStart,
  youTubeUrl,
  youTubeVideoId,
} from "./youtube";

describe("youTubeVideoId", () => {
  it.each([
    "dQw4w9WgXcQ",
    " https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=43s ",
    "https://youtube.com/watch?feature=shared&v=dQw4w9WgXcQ",
    "http://m.youtube.com/watch?v=dQw4w9WgXcQ",
    "https://music.youtube.com/watch?v=dQw4w9WgXcQ",
    "https://youtu.be/dQw4w9WgXcQ?t=43",
    "youtu.be/dQw4w9WgXcQ",
    "www.youtube.com/watch?v=dQw4w9WgXcQ",
    "https://www.youtube.com/shorts/dQw4w9WgXcQ",
    "https://www.youtube.com/live/dQw4w9WgXcQ?si=x",
    "https://www.youtube.com/embed/dQw4w9WgXcQ?start=43",
    "https://WWW.YouTube.com/watch?v=dQw4w9WgXcQ",
  ])("finds the video in %s", (link) => {
    expect(youTubeVideoId(link)).toBe("dQw4w9WgXcQ");
  });

  it.each([
    "",
    "dQw4w9WgXc",
    "dQw4w9WgXcQ!",
    "https://www.youtube.com/watch?v=dQw4w9WgXc",
    "https://www.youtube.com/watch",
    "https://www.youtube.com/clip/UgkxdQw4w9WgXcQ",
    "https://www.youtube.com/@RickAstleyYT",
    "https://www.youtube.com/",
    "https://evil.example/watch?v=dQw4w9WgXcQ",
    "https://youtube.com.evil.example/watch?v=dQw4w9WgXcQ",
    "https://evil.example/?https://youtu.be/dQw4w9WgXcQ",
    "https://vimeo.com/dQw4w9WgXcQ",
  ])("finds no video in %s", (link) => {
    expect(youTubeVideoId(link)).toBeNull();
  });
});

describe("youTubeStart", () => {
  it.each([
    ["https://youtu.be/dQw4w9WgXcQ?t=43", 43],
    ["https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=43s", 43],
    ["https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1m13s", 73],
    ["https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1h2m3s", 3723],
    ["https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=2m", 120],
    ["https://www.youtube.com/embed/dQw4w9WgXcQ?start=7", 7],
    ["youtu.be/dQw4w9WgXcQ?t=5", 5],
    ["https://www.youtube.com/watch?v=dQw4w9WgXcQ", null],
    ["https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=soon", null],
    ["https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1m5", null],
    ["dQw4w9WgXcQ", null],
  ])("reads the start of %s", (link, start) => {
    expect(youTubeStart(link)).toBe(start);
  });
});

describe("times", () => {
  it.each([
    ["0", 0],
    ["43", 43],
    ["1:13", 73],
    [" 01:02:03 ", 3723],
    ["90:00", 5400],
    ["", null],
    ["1:", null],
    ["1:2:3:4", null],
    ["1m13s", null],
    ["-1", null],
  ])("parses %s", (text, seconds) => {
    expect(parseTime(text)).toBe(seconds);
  });

  it.each([
    [0, "0:00"],
    [43, "0:43"],
    [73, "1:13"],
    [3600, "1:00:00"],
    [3723, "1:02:03"],
  ])("writes %d", (seconds, text) => {
    expect(timeText(seconds)).toBe(text);
  });
});

describe("youTubeUrl", () => {
  it("links to the start of the video", () => {
    expect(youTubeUrl("dQw4w9WgXcQ", 0)).toBe("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    expect(youTubeUrl("dQw4w9WgXcQ", 43)).toBe("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=43s");
  });
});

describe("mediaLink", () => {
  it.each([
    ["https://youtu.be/dQw4w9WgXcQ?t=43", { platform: "youtube", id: "dQw4w9WgXcQ" }],
    ["https://clips.twitch.tv/Funny-Clip_1", { platform: "twitch", id: "Funny-Clip_1" }],
    [" clips.twitch.tv/Funny-Clip_1?x=1 ", { platform: "twitch", id: "Funny-Clip_1" }],
    ["https://www.twitch.tv/someone/clip/Funny-Clip_1", { platform: "twitch", id: "Funny-Clip_1" }],
    ["https://m.twitch.tv/someone/clip/Funny-Clip_1", { platform: "twitch", id: "Funny-Clip_1" }],
    [
      "https://clips.twitch.tv/embed?clip=Funny-Clip_1&parent=x",
      { platform: "twitch", id: "Funny-Clip_1" },
    ],
  ])("finds what %s links to", (link, found) => {
    expect(mediaLink(link)).toEqual(found);
  });

  it.each([
    "",
    "https://www.twitch.tv/someone",
    "https://www.twitch.tv/videos/123",
    "https://clips.twitch.tv/",
    "https://clips.twitch.tv/embed",
    "https://clips.twitch.tv.evil.example/Slug",
    "https://kick.com/someone/clips/clip_01M86Z1Z62360AM4D5KT55H4GS",
    "https://vimeo.com/1",
  ])("finds nothing in %s", (link) => {
    expect(mediaLink(link)).toBeNull();
  });
});
