import { api } from "@/lib/api";

export type Score = { key: string; name: string; short: string; riskier: boolean; score: number | null };

export type Summary = {
  hash: string;
  folder: string;
  objective: string;
  generated_at: string;
  harness: string | null;
  ratings: Score[];
  findings: number;
  open_findings: number;
  exploits: number;
  report: boolean;
};

export type ReviewList = { reviews: Summary[]; can_start: boolean };

export type Factor = { name: string; score: number; why: string };
export type Rating = { name: string; short: string; score: number; factors: Factor[] };
export type Item = { index: number; text: string; score: number; factors: Factor[] };

export type Card = {
  key: string;
  name: string;
  short: string;
  riskier: boolean;
  rating: Rating | null;
  missing?: string;
  items?: Item[];
};

export type Technology = { name: string; kind: string; risk: number };
export type Exploit = { id: string; technology: string; severity: string; summary: string; fixed_in?: string };
export type Finding = {
  title: string;
  technology?: string;
  file?: string;
  severity: string;
  likelihood?: string;
  impact?: string;
  confidence?: string;
  fix?: string;
  status?: string;
};

export type Review = {
  version: string;
  folder: string;
  objective: string;
  generated_at: string;
  harness: string | null;
  ratings: Card[];
  technologies: Technology[];
  exploits: Exploit[];
  findings: Finding[];
  notes: string[];
};

export type NewReview = { folder: string; objective?: string };
export type Started = { job: string; hash: string };
export type JobEvent =
  | { event: "log"; line: string }
  | { event: "done"; ok: boolean; code: number | null; hash: string; message: string | null };

export const listReviews = () => api<ReviewList>("GET", "/api/reviews");
export const getReview = (hash: string) => api<Review>("GET", `/api/reviews/${encodeURIComponent(hash)}`);
export const startReview = (request: NewReview) => api<Started>("POST", "/api/reviews", request);

export type Tone = "good" | "fair" | "poor" | "none";

export const tone = (score: number | null | undefined, riskier: boolean): Tone => {
  if (score === null || score === undefined) return "none";
  const good = riskier ? 100 - score : score;
  if (good >= 80) return "good";
  if (good >= 50) return "fair";
  return "poor";
};

export const toneText: Record<Tone, string> = {
  good: "text-success",
  fair: "text-warning",
  poor: "text-destructive",
  none: "text-muted-foreground",
};

export const toneFill: Record<Tone, string> = {
  good: "bg-success",
  fair: "bg-warning",
  poor: "bg-destructive",
  none: "bg-muted-foreground/40",
};

export const toneStroke: Record<Tone, string> = {
  good: "stroke-success",
  fair: "stroke-warning",
  poor: "stroke-destructive",
  none: "stroke-muted-foreground",
};

export const toneSoft: Record<Tone, string> = {
  good: "bg-success/12 text-success",
  fair: "bg-warning/14 text-warning",
  poor: "bg-destructive/10 text-destructive",
  none: "bg-fill-default text-muted-foreground",
};

export const toneWord: Record<Tone, string> = {
  good: "Good",
  fair: "Fair",
  poor: "Poor",
  none: "Not rated",
};

export const percent = (score: number) => `${Math.round(score)}%`;

const severityRank: Record<string, number> = { critical: 0, high: 1, medium: 2, low: 3, informational: 4 };

export const bySeverity = <T extends { severity: string }>(rows: T[]) =>
  [...rows].sort(
    (a, b) => (severityRank[a.severity.toLowerCase()] ?? 5) - (severityRank[b.severity.toLowerCase()] ?? 5),
  );

export const severityTone = (severity: string): string => {
  switch (severity.toLowerCase()) {
    case "critical":
    case "high":
      return toneSoft.poor;
    case "medium":
      return toneSoft.fair;
    case "low":
      return "bg-fill-default text-foreground";
    default:
      return toneSoft.none;
  }
};

export const isFixed = (finding: Finding) => (finding.status ?? "").toLowerCase() === "fixed";

export const shortFolder = (folder: string) => {
  const home = folder.match(/^\/(Users|home)\/[^/]+/);
  const path = home ? `~${folder.slice(home[0].length)}` : folder;
  const parts = path.split("/").filter(Boolean);
  if (parts.length <= 3) return path;
  return `${parts[0] === "~" ? "~" : ""}/…/${parts.slice(-2).join("/")}`;
};

export const folderName = (folder: string) => folder.split("/").filter(Boolean).pop() ?? folder;

export const when = (stamp: string, now = Date.now()) => {
  const at = Date.parse(stamp);
  if (Number.isNaN(at)) return stamp;
  const seconds = Math.max(0, Math.round((now - at) / 1000));
  if (seconds < 60) return "just now";
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.round(hours / 24);
  if (days < 30) return `${days}d ago`;
  return new Date(at).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
};

export const dated = (stamp: string) => {
  const at = Date.parse(stamp);
  if (Number.isNaN(at)) return stamp;
  return new Date(at).toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
};
