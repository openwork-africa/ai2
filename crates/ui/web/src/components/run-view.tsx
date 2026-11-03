import { cn } from "cn";
import { ChevronRight, CircleAlert, CircleCheck } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Link } from "react-router";
import { Dot } from "@/components/page";
import { ScorePill } from "@/components/review";
import { Spinner } from "@/components/ui/spinner";
import { useReviewCards } from "@/hooks/use-run";
import { endLine, phaseOf, type Run, type Transcript } from "@/lib/agents";
import type { Harness } from "@/lib/harnesses";
import { dated, when } from "@/lib/reviews";

const monogram = (id: string, label?: string) => (id.length <= 2 ? id : (label ?? id).charAt(0).toUpperCase());

export function HarnessLogo({ id, harness, size = "md", className }: { id: string; harness?: Harness; size?: "sm" | "md" | "lg"; className?: string }) {
  const [broken, setBroken] = useState(false);
  const logo = harness?.logo;
  return (
    <span
      className={cn(
        "flex shrink-0 items-center justify-center bg-white shadow-(--shadow-secondary-control) ring-1 ring-black/8 dark:ring-white/10",
        size === "sm" && "size-7 rounded-lg",
        size === "md" && "size-11 rounded-xl",
        size === "lg" && "size-14 rounded-2xl",
        className,
      )}
    >
      {logo && !broken ? (
        <img
          src={`/harnesses/${logo}`}
          alt=""
          className={size === "sm" ? "size-4" : size === "lg" ? "size-8" : "size-6"}
          onError={() => setBroken(true)}
        />
      ) : (
        <span className={cn("font-heading font-semibold tracking-tight text-neutral-800", size === "sm" ? "text-xs" : "text-base")}>
          {monogram(id, harness?.label)}
        </span>
      )}
    </span>
  );
}

export function Scores({ hash, chevron = true, className }: { hash: string; chevron?: boolean; className?: string }) {
  const cards = useReviewCards(hash);
  if (cards === undefined) return <Spinner className={cn("size-3.5 text-muted-foreground", className)} />;
  if (!cards || cards.length === 0) {
    return (
      <Link to={`/reviews/${hash}`} className={cn("text-xs text-muted-foreground hover:text-foreground", className)}>
        Checked
      </Link>
    );
  }
  return (
    <Link
      to={`/reviews/${hash}`}
      onClick={(event) => event.stopPropagation()}
      className={cn("group/scores inline-flex items-center gap-1.5 rounded-full outline-none focus-visible:ring-2 focus-visible:ring-ring/30", className)}
      title="Open the review"
    >
      {cards.map((card) => (
        <span key={card.key} className="inline-flex items-center gap-1" title={card.name}>
          <span className="text-2xs font-medium text-muted-foreground">{card.short}</span>
          <ScorePill score={card.rating?.score ?? null} riskier={card.riskier} className="h-5 min-w-10 text-2xs" />
        </span>
      ))}
      {chevron && <ChevronRight className="size-3.5 text-muted-foreground transition-transform group-hover/scores:translate-x-0.5" />}
    </Link>
  );
}

const PHASE = {
  working: { tone: "success", label: "Working" },
  checking: { tone: "warning", label: "Checking…" },
  done: { tone: "success", label: "Done" },
  failed: { tone: "destructive", label: "Ended" },
} as const;

export function RunStanding({ run }: { run: Run }) {
  const phase = phaseOf(run);
  const label = phase === "failed" ? endLine(run) : PHASE[phase].label;
  return (
    <span className="inline-flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
      {phase === "working" || phase === "checking" ? <Spinner className="size-3" /> : <Dot tone={PHASE[phase].tone} className="ring-0" />}
      <span className="truncate">{label}</span>
    </span>
  );
}

export function RunLog({ transcript, className }: { transcript: Transcript; className?: string }) {
  const box = useRef<HTMLDivElement>(null);
  const [follow, setFollow] = useState(true);
  const { lines, run } = transcript;
  const live = run ? run.state === "running" : true;

  useEffect(() => {
    if (follow && box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [lines, follow]);

  return (
    <div
      ref={box}
      data-testid="run-log"
      onScroll={(event) => {
        const el = event.currentTarget;
        setFollow(el.scrollHeight - el.scrollTop - el.clientHeight < 8);
      }}
      className={cn(
        "max-h-[26rem] overflow-auto rounded-2xl bg-neutral-950 px-4 py-3 font-mono text-[0.75rem] leading-5 text-neutral-200 shadow-(--shadow-card) ring-1 ring-black/10 [font-variant-ligatures:none] dark:bg-black/60 dark:ring-white/10",
        className,
      )}
    >
      {lines.length === 0 ? (
        <p className="text-neutral-500">{live ? "Waiting for the first word…" : "It said nothing."}</p>
      ) : (
        lines.map((line, at) => (
          <div key={at} className={cn("whitespace-pre-wrap break-words", line.stream === "err" && "text-rose-300")}>
            {line.text || " "}
          </div>
        ))
      )}
      {live && lines.length > 0 && <span className="mt-1 inline-block h-3.5 w-1.5 animate-pulse bg-neutral-400 align-middle" />}
    </div>
  );
}

export function RunFooter({ transcript }: { transcript: Transcript }) {
  const { run, checking, reviewed } = transcript;
  if (!run) return null;
  const phase = phaseOf(run);
  const hash = reviewed?.hash ?? run.review;
  const reviewError = reviewed?.error ?? run.review_error;
  return (
    <div className="flex min-h-6 flex-wrap items-center gap-x-4 gap-y-2 text-xs text-muted-foreground">
      {phase === "working" && (
        <span className="inline-flex items-center gap-1.5">
          <Spinner className="size-3.5" />
          Working
        </span>
      )}
      {phase === "checking" && (
        <span className="inline-flex min-w-0 items-center gap-1.5">
          <Spinner className="size-3.5" />
          <span className="font-medium text-foreground">Checking…</span>
          {checking && <span className="truncate">{checking}</span>}
        </span>
      )}
      {(phase === "done" || phase === "failed") && (
        <span className={cn("inline-flex min-w-0 items-center gap-1.5", phase === "failed" && "text-destructive")}>
          {phase === "done" ? <CircleCheck className="size-3.5 text-success" /> : <CircleAlert className="size-3.5" />}
          <span className="truncate">{endLine(run)}</span>
        </span>
      )}
      {phase !== "working" && phase !== "checking" && hash && <Scores hash={hash} />}
      {phase !== "working" && phase !== "checking" && !hash && reviewError && <span className="truncate">Not checked: {reviewError}</span>}
      {run.ended && (
        <span className="ml-auto tabular-nums" title={dated(run.ended)}>
          {when(run.ended)}
        </span>
      )}
    </div>
  );
}
