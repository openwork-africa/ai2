export type Chapter = { id: string; label: string; title: string; body: string; image: string; alt: string };

export const CHAPTERS: Chapter[] = [
  {
    id: "monitor",
    label: "Monitor",
    title: "Every app you make, in one place.",
    body: "ai squared keeps the apps you and your agents build on this machine together. Running or stopped, each one is here with its logs and its API.",
    image: "/welcome/monitor.webp",
    alt: "A telescope on a hill watching a constellation of app windows, one of them green.",
  },
  {
    id: "secure",
    label: "Secure",
    title: "Your keys never reach your agents.",
    body: "Agents get a token, not the secret. The vault adds the key on the way out, only to the hosts you allow. And every run is audited, with a Modern Security Rating and a Run Risk Factor.",
    image: "/welcome/secure.webp",
    alt: "A vault door set in a shield, with a key beside it.",
  },
  {
    id: "consume",
    label: "Consume",
    title: "Open them, run them, use them.",
    body: "Start an app and use it in your browser. Your agents can use it too, through MCP and OpenAPI. First, setup gets the vault ready on this machine.",
    image: "/welcome/consume.webp",
    alt: "Jars of apps on a shelf, a hand lifting the green one.",
  },
];

export const welcome = {
  name: "ai squared",
  tagline: "Built to monitor, secure and consume your own apps.",
  next: "Next",
  setUp: "Set up",
  skip: "Skip",
  go: (label: string) => `Go to ${label}`,
};
