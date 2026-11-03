export type Chapter = { id: string; title: string; body: string; image: string; alt: string; pull: number };

export const CHAPTERS: Chapter[] = [
  {
    id: "monitor",
    title: "ai squared watches every agent while it works.",
    body: "See which agents are signed in, which are running, and what each run did.",
    image: "/welcome/monitor.webp",
    pull: 0,
    alt: "A telescope on a hill watching three rounded agents at work among floating app windows, one of them green.",
  },
  {
    id: "secure",
    title: "ai squared treats every agent as if it might be compromised.",
    body: "Every run is reviewed, every write waits for your yes, and an agent only ever holds a token, never your keys.",
    image: "/welcome/secure.webp",
    pull: 0.2,
    alt: "A vault door set in a shield, with a key beside it.",
  },
  {
    id: "consume",
    title: "Your personal apps, organised.",
    body: "Run them from one place, and let your agents call on them too.",
    image: "/welcome/consume.webp",
    pull: 0.28,
    alt: "Jars of apps on a shelf, a hand lifting the green one.",
  },
];

export const welcome = {
  name: "ai squared",
  tagline: "Your agents watched, your keys kept, your apps together. Nothing leaves this machine.",
  next: "Next",
  setUp: "Set up",
  skip: "Skip",
  go: (index: number) => `Screen ${index + 1} of ${CHAPTERS.length}`,
};
