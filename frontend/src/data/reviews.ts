import { t } from "../i18n";

/** Public reviews from the App Store listing, quoted in full. */
export type Review = {
  name: string;
  title: string;
  text: string;
  /** ISO date of the review. */
  date: string;
};

export const appStoreRating = { score: "4.9", count: "71" };

export const reviews: Review[] = [
  {
    name: "harmoneye",
    date: "2026-07-21",
    title: t("BEST APP TO LIVE STREAM EVER MADE"),
    text: t(
      "nothing beats Moblin at the moment they are becoming the industry standard of what a live stream app should be. all the continued updates has only made it infinitely better the possibilities that you can do feel endless especially being able to read chat and control my stream from my apple watch is the coolest thing ever.",
    ),
  },
  {
    name: "Mike's Music Mix",
    date: "2026-06-07",
    title: t("Disney streamer"),
    text: t(
      "Moblin is very easy to navigate great picture love it. Can't wait to see what comes next in updates!",
    ),
  },
  {
    name: "Mkm047",
    date: "2025-04-19",
    title: t("Best streaming app out there"),
    text: t(
      "Great app for streaming IRL's. Hands down the best out there. Made by an awesome developer. :) 10/10 recommend!",
    ),
  },
];
