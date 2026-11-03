import { Bot, KeyRound, LayoutGrid, ShieldCheck, type LucideIcon } from "lucide-react";
import type { ComponentType } from "react";
import { VaultNavBadge } from "@/pages/vault/NavBadge";

export type NavEntry = {
  to: string;
  label: string;
  icon: LucideIcon;
  feature: string;
  match: (path: string) => boolean;
  badge?: ComponentType<{ reachable: boolean }>;
};

const under = (prefix: string) => (path: string) => path === prefix || path.startsWith(`${prefix}/`);

export const NAV: NavEntry[] = [
  { to: "/", label: "Apps", icon: LayoutGrid, feature: "apps", match: (path) => path === "/" || under("/apps")(path) },
  { to: "/reviews", label: "Reviews", icon: ShieldCheck, feature: "reviews", match: under("/reviews") },
  { to: "/vault", label: "Vault", icon: KeyRound, feature: "vault", match: under("/vault"), badge: VaultNavBadge },
  { to: "/harnesses", label: "Harnesses", icon: Bot, feature: "harness", match: under("/harnesses") },
];
