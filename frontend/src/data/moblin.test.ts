import { describe, expect, test } from "vitest";
import { quickButtonNameMatchesFilter } from "./moblin";

describe("quickButtonNameMatchesFilter", () => {
  test.each([
    ["Mute", ""],
    ["", ""],
    ["Mute", "m"],
    ["Mute", "mute"],
    ["Mute", "MU"],
    ["MUTE", "mu"],
    ["Instant replay", "rep"],
    ["Instant replay", "inst"],
    ["Scene  widgets", "wid"],
    ["Scene\twidgets", "wid"],
    ["4:3", "4:"],
    ["Ölbrunn", "öl"],
    ["Torch", " "],
    ["Scene widgets", "scene w"],
    ["Scene widgets", "s w"],
    ["Scene widgets", "Scene Widgets"],
    ["Scene widgets", "s  w"],
    ["Scene widgets", "scene "],
    ["Scene widgets", " w"],
    ["Start instant replay", "i r"],
  ])("%j matches %j", (name, filter) => {
    expect(quickButtonNameMatchesFilter(name, filter)).toBe(true);
  });

  test.each([
    ["", "m"],
    ["Mute", "ute"],
    ["Mute", "muted"],
    ["Instant replay", "play"],
    ["Instant replay", "inst x"],
    ["Scene widgets", "w s"],
    ["Scene widgets", "s w x"],
    ["Scene widgets", "s idgets"],
    ["Start instant replay", "s r"],
  ])("%j does not match %j", (name, filter) => {
    expect(quickButtonNameMatchesFilter(name, filter)).toBe(false);
  });
});
