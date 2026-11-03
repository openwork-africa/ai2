import { cn } from "cn";
import {
  ChevronRight,
  Circle,
  CircleCheck,
  CircleDashed,
  CircleOff,
  CircleX,
  Copy,
  MessageCircleQuestion,
  RotateCcw,
  Sparkles,
  TriangleAlert,
} from "lucide-react";
import { useState } from "react";
import { LogLineText, LogView, logText } from "@/components/setup/log-view";
import { clock, duration, Rich } from "@/components/setup/text";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Spinner } from "@/components/ui/spinner";
import { failureHint, states, stepText, type StepCopy } from "@/copy/setup";
import type { StepView } from "@/lib/setup";

export type Look = "pending" | "running" | "input" | "done" | "warned" | "unsupported" | "failed" | "skipped" | "missing" | "unknown";

export const lookOf = (step: StepView, waiting: boolean): Look => {
  if (waiting) return "input";
  switch (step.status) {
    case "ready":
      return step.warnings.length ? "warned" : "done";
    case "running":
    case "unsupported":
    case "failed":
    case "skipped":
    case "missing":
    case "unknown":
    case "pending":
      return step.status;
  }
};

export function StepIcon({ look, className }: { look: Look; className?: string }) {
  const base = cn("size-[1.125rem]", className);
  switch (look) {
    case "running":
      return <Spinner className={cn(base, "text-foreground")} />;
    case "input":
      return <MessageCircleQuestion className={cn(base, "text-primary")} />;
    case "done":
      return <CircleCheck className={cn(base, "text-success")} />;
    case "warned":
      return <TriangleAlert className={cn(base, "text-warning")} />;
    case "unsupported":
      return <CircleOff className={cn(base, "text-muted-foreground")} />;
    case "failed":
      return <CircleX className={cn(base, "text-destructive")} />;
    case "skipped":
      return <CircleDashed className={cn(base, "text-muted-foreground/60")} />;
    case "missing":
      return <Circle className={cn(base, "text-warning")} />;
    default:
      return <Circle className={cn(base, "text-muted-foreground/45")} />;
  }
}

function Right({ look, step, now, since }: { look: Look; step: StepView; now: number; since?: number }) {
  const word = {
    warned: states.warned,
    unsupported: states.unsupported,
    failed: states.failed,
    skipped: states.skipped,
    missing: states.missing,
    unknown: states.unknown,
    input: states.input,
  } as Partial<Record<Look, string>>;
  if (look === "running") {
    return <span className="text-xs text-foreground tabular-nums">{clock(since ? now - since : 0)}</span>;
  }
  if (look === "done") return <span className="text-xs text-muted-foreground tabular-nums">{duration(step.ms)}</span>;
  const text = word[look];
  if (!text) return null;
  const tone = look === "failed" ? "text-destructive" : look === "warned" || look === "missing" ? "text-warning" : "text-muted-foreground";
  return <span className={cn("text-xs whitespace-nowrap", tone)}>{text}</span>;
}

function StepLog({ step, open: initial }: { step: StepView; open: boolean }) {
  const [open, setOpen] = useState(initial);
  if (step.log.length === 0 && step.status !== "running") return null;
  return (
    <Collapsible open={open} onOpenChange={(details) => setOpen(details.open)}>
      <CollapsibleTrigger className="group -ml-1 flex items-center gap-1 rounded-md px-1 py-0.5 text-xs text-muted-foreground transition-colors hover:text-foreground">
        <ChevronRight className={cn("size-3.5 transition-transform duration-200", open && "rotate-90")} />
        {open ? stepText.hideLog : stepText.showLog}
        {step.log.length > 0 && <span className="text-muted-foreground/60 tabular-nums">· {step.log.length}</span>}
      </CollapsibleTrigger>
      <CollapsibleContent>
        <LogView lines={step.log} className="mt-2" />
      </CollapsibleContent>
    </Collapsible>
  );
}

export function StepRow({
  step,
  copy,
  now,
  since,
  last,
  onRetry,
}: {
  step: StepView;
  copy: StepCopy;
  now: number;
  since?: number;
  last: boolean;
  onRetry?: () => void;
}) {
  const look = lookOf(step, false);
  const [copied, setCopied] = useState(false);
  const quiet = look === "pending" || look === "skipped";
  const active = look === "running" || look === "failed";
  const lastLine = step.log.at(-1);
  const copyLog = () =>
    navigator.clipboard?.writeText(logText(step.log)).then(() => {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    });

  return (
    <li className="relative flex gap-3.5" data-step={step.id} data-look={look}>
      <div className="flex w-6 shrink-0 flex-col items-center pt-[0.6875rem]">
        <span className="flex size-6 items-center justify-center rounded-full bg-background">
          <StepIcon look={look} />
        </span>
        {!last && (
          <span
            aria-hidden
            className={cn("mt-1.5 w-px flex-1", look === "done" || look === "warned" ? "bg-success/35" : "bg-border")}
          />
        )}
      </div>
      <div
        className={cn(
          "mb-2 flex min-w-0 flex-1 flex-col gap-1 rounded-2xl px-3.5 py-2.5 transition-colors duration-300",
          active && "bg-card shadow-(--shadow-card) ring-1 ring-border",
          look === "failed" && "ring-destructive/30",
        )}
      >
        <div className="flex items-baseline justify-between gap-3">
          <h3 className={cn("text-sm font-medium tracking-tight", quiet ? "text-muted-foreground" : "text-foreground")}>
            {copy.title}
          </h3>
          <Right look={look} step={step} now={now} since={since} />
        </div>
        <p className={cn("text-[0.8125rem] leading-5 text-pretty", quiet ? "text-muted-foreground/75" : "text-muted-foreground")}>
          {copy.why}
        </p>

        {look === "warned" &&
          step.warnings.map((warning) => (
            <p key={warning} className="text-[0.8125rem] leading-5 text-warning">
              {warning}
            </p>
          ))}

        {look === "unsupported" && step.warnings.length > 0 && (
          <Alert className="mt-2 rounded-2xl bg-fill-subtle ring-0">
            <TriangleAlert className="text-warning" />
            <AlertDescription className="text-foreground/85">{step.warnings.join(" ")}</AlertDescription>
          </Alert>
        )}

        {look === "running" && (
          <div className="mt-1.5 flex flex-col gap-2.5">
            <p className="flex gap-2 text-[0.8125rem] leading-5 text-muted-foreground">
              <Sparkles className="mt-0.5 size-3.5 shrink-0 text-warning" />
              <span className="text-pretty">
                <Rich text={copy.fact} />
              </span>
            </p>
            {lastLine && (
              <div className="rounded-lg bg-fill-subtle px-2.5 py-1">
                <LogLineText line={lastLine} />
              </div>
            )}
            <StepLog step={step} open={false} />
          </div>
        )}

        {look === "failed" && (
          <div className="mt-1.5 flex flex-col gap-3">
            {step.error && <p className="text-[0.8125rem] leading-5 font-medium text-destructive">{step.error}</p>}
            <p className="text-xs text-muted-foreground">{failureHint}</p>
            <StepLog step={step} open />
            <div className="flex flex-wrap gap-2">
              {onRetry && (
                <Button onClick={onRetry}>
                  <RotateCcw />
                  {stepText.tryAgain}
                </Button>
              )}
              {step.log.length > 0 && (
                <Button variant="outline" onClick={copyLog}>
                  <Copy />
                  {copied ? stepText.copied : stepText.copyLog}
                </Button>
              )}
            </div>
          </div>
        )}

        {(look === "done" || look === "warned" || look === "unsupported") && step.log.length > 0 && (
          <div className="mt-0.5">
            <StepLog step={step} open={false} />
          </div>
        )}
      </div>
    </li>
  );
}
