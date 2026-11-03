export type Chapter = { id: string; title: string; body: string; image: string; alt: string };

export const CHAPTERS: Chapter[] = [
  {
    id: "monitor",
    title: "ai squared keeps every agent on this machine in view while it works.",
    body: "Claude Code, Codex, pi, fx and the rest live on this machine, and ai squared keeps them in view: which ones are installed and signed in, which are running right now, and what each run did, line by line, as it happens.",
    image: "/welcome/monitor.webp",
    alt: "A telescope on a hill watching three rounded agents at work among floating app windows, one of them green.",
  },
  {
    id: "secure",
    title: "ai squared treats every agent as if it might be compromised.",
    body: "Every run is reviewed against exploits published in the last few days, and a reviewer reads every file it changed. Each call is held against the job you gave the run, and anything that writes waits for your yes. A tricked agent holds a token, never a key, so it has nothing to leak.",
    image: "/welcome/secure.webp",
    alt: "A vault door set in a shield, with a key beside it.",
  },
  {
    id: "consume",
    title: "Your apps stay close, where you can care for them and your agents can call on them, and every one you finish makes the next one easier to begin.",
    body: "Start them, stop them, read their logs and change their settings from one shelf. Your agents reach each one through MCP and its OpenAPI document. Set up comes next, and it gets the vault ready on this machine.",
    image: "/welcome/consume.webp",
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
