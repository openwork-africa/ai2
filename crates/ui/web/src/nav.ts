import { Bot, KeyRound, LayoutGrid, ListChecks, ShieldCheck, type LucideIcon } from "lucide-react";

export type NavEntry = {
  to: string;
  label: string;
  icon: LucideIcon;
  feature: string;
  match: (path: string) => boolean;
};

const under = (prefix: string) => (path: string) => path === prefix || path.startsWith(`${prefix}/`);

export const NAV: NavEntry[] = [
  { to: "/", label: "Apps", icon: LayoutGrid, feature: "apps", match: (path) => path === "/" || under("/apps")(path) },
  { to: "/setup", label: "Setup", icon: ListChecks, feature: "setup", match: under("/setup") },
  { to: "/reviews", label: "Reviews", icon: ShieldCheck, feature: "reviews", match: under("/reviews") },
  { to: "/vault", label: "Vault", icon: KeyRound, feature: "vault", match: under("/vault") },
  { to: "/harnesses", label: "Harnesses", icon: Bot, feature: "harness", match: under("/harnesses") },
];
