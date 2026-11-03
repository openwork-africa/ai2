import { cn } from "cn";
import { Check, ChevronRight, CircleCheck, CircleX, Copy, ExternalLink, KeyRound, RotateCcw, X } from "lucide-react";
import { type FormEvent, useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import type { JobView } from "@/hooks/use-harness-jobs";
import type { Harness } from "@/lib/harnesses";

const clock = (ms: number) => {
  const seconds = Math.max(0, Math.floor(ms / 1000));
  if (seconds < 60) return `0:${String(seconds).padStart(2, "0")}`;
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
};

const took = (ms: number) => (ms < 10000 ? `${(ms / 1000).toFixed(1)}s` : clock(ms));

function Elapsed({ from, to }: { from: number; to?: number }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (to) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [to]);
  return <span className="font-mono text-xs text-muted-foreground tabular-nums">{to ? took(to - from) : clock(now - from)}</span>;
}

const headline = (job: JobView, label: string) => {
  const install = job.action === "install";
  if (!job.done) return install ? `Installing ${label}` : `Signing in to ${label}`;
  if (job.done.ok) return install ? `Installed ${label}` : `Signed in to ${label}`;
  return install ? "The install didn't finish" : "The sign-in didn't finish";
};

const lastLine = (lines: string[]) => {
  for (let at = lines.length - 1; at >= 0; at -= 1) {
    if (!lines[at].startsWith("$ ")) return lines[at];
  }
  return null;
};

function CopyButton({ text, label }: { text: string; label: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      onClick={() => {
        void navigator.clipboard?.writeText(text).then(() => {
          setCopied(true);
          window.setTimeout(() => setCopied(false), 1500);
        });
      }}
    >
      {copied ? <Check /> : <Copy />}
      {copied ? "Copied" : label}
    </Button>
  );
}

function Log({ lines, open }: { lines: string[]; open: boolean }) {
  const box = useRef<HTMLPreElement>(null);
  const [shown, setShown] = useState(open);
  useEffect(() => {
    if (open) setShown(true);
  }, [open]);
  useEffect(() => {
    const element = box.current;
    if (shown && element) element.scrollTop = element.scrollHeight;
  }, [lines, shown]);
  return (
    <details
      open={shown}
      onToggle={(event) => setShown((event.currentTarget as HTMLDetailsElement).open)}
      className="group/log"
    >
      <summary className="flex w-fit cursor-pointer list-none items-center gap-1 rounded-md py-0.5 text-xs text-muted-foreground transition-colors select-none hover:text-foreground [&::-webkit-details-marker]:hidden">
        <ChevronRight className="size-3.5 transition-transform group-open/log:rotate-90" />
        {shown ? "Hide the log" : "Show the log"}
        <span className="text-muted-foreground/70 tabular-nums">· {lines.length} {lines.length === 1 ? "line" : "lines"}</span>
      </summary>
      <pre
        ref={box}
        className="mt-2 max-h-72 overflow-auto rounded-xl bg-fill-subtle p-3 font-mono text-2xs leading-relaxed whitespace-pre-wrap text-foreground/85 ring-1 ring-border [font-variant-ligatures:none]"
      >
        {lines.map((line, at) => (
          <span key={at} className={cn("block", line.startsWith("$ ") && "text-muted-foreground")}>
            {line}
          </span>
        ))}
      </pre>
    </details>
  );
}

function Answer({ harness, job, onAnswer }: { harness: Harness; job: JobView; onAnswer: (text: string) => Promise<void> }) {
  const [text, setText] = useState("");
  const [sending, setSending] = useState(false);
  const [failed, setFailed] = useState<string | null>(null);
  const key = harness.asks === "key";
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const value = text.trim();
    if (!value) {
      setFailed(key ? "The key goes here first." : "The code goes here first.");
      return;
    }
    setSending(true);
    setFailed(null);
    try {
      await onAnswer(value);
      setText("");
    } catch (error) {
      setFailed((error as Error).message);
    } finally {
      setSending(false);
    }
  };
  const problem = failed ?? job.error ?? null;
  const field = `answer-${harness.id}`;
  return (
    <form onSubmit={submit} className="flex flex-col gap-2">
      <label htmlFor={field} className="text-xs font-medium">
        {key ? "API key" : "The code from the sign-in page"}
      </label>
      <div className="flex gap-2">
        <Input
          id={field}
          type={key ? "password" : "text"}
          autoComplete="off"
          spellCheck={false}
          data-size="lg"
          placeholder={key ? "Paste the key" : "Paste the code"}
          value={text}
          onChange={(event) => setText(event.currentTarget.value)}
          aria-invalid={problem ? true : undefined}
          className="font-mono"
        />
        <Button type="submit" size="lg" disabled={sending}>
          {sending ? <Spinner /> : null}
          Continue
        </Button>
      </div>
      {problem ? (
        <p className="text-xs text-destructive">{problem}</p>
      ) : job.sent > 0 ? (
        <p className="flex items-center gap-1 text-xs text-muted-foreground">
          <Check className="size-3.5 text-success" />
          Sent. It is typed in for you and never shown back.
        </p>
      ) : (
        <p className="text-xs text-muted-foreground">It's typed into the sign-in for you and never shown in the log.</p>
      )}
    </form>
  );
}

export function HarnessJob({
  harness,
  job,
  onAnswer,
  onRetry,
  onDismiss,
}: {
  harness: Harness;
  job: JobView;
  onAnswer: (text: string) => Promise<void>;
  onRetry: () => void;
  onDismiss: () => void;
}) {
  const running = !job.done;
  const failed = job.done && !job.done.ok;
  const signIn = job.action === "sign-in";
  const latest = lastLine(job.lines);
  return (
    <section
      aria-live="polite"
      className={cn(
        "flex flex-col gap-4 rounded-2xl p-4 ring-1",
        running && "bg-fill-subtle ring-border",
        job.done?.ok && "bg-success/5 ring-success/25",
        failed && "bg-destructive/5 ring-destructive/25",
      )}
    >
      <div className="flex items-center gap-2.5">
        {running ? (
          <Spinner className="text-foreground" />
        ) : job.done?.ok ? (
          <CircleCheck className="size-4 text-success" />
        ) : (
          <CircleX className="size-4 text-destructive" />
        )}
        <span className="text-sm font-medium">{headline(job, harness.label)}</span>
        <span className="ml-auto flex items-center gap-2">
          <Elapsed from={job.started} to={job.ended} />
          {job.done && (
            <Button type="button" variant="ghost" size="icon-xs" aria-label="Close" onClick={onDismiss}>
              <X />
            </Button>
          )}
        </span>
      </div>

      {running && latest && (
        <p className="-mt-2 truncate pl-6.5 font-mono text-2xs text-muted-foreground [font-variant-ligatures:none]">{latest}</p>
      )}

      {failed && (
        <div className="-mt-2 flex flex-col gap-3 pl-6.5">
          <p className="text-xs text-foreground/80">{job.done?.message ?? "It stopped."} Fix that, then press Try again.</p>
          <span className="flex flex-wrap items-center gap-1.5">
            <Button type="button" variant="outline" size="sm" onClick={onRetry}>
              <RotateCcw />
              Try again
            </Button>
            <CopyButton text={job.lines.join("\n")} label="Copy the log" />
          </span>
        </div>
      )}

      {signIn && running && (
        <div className="flex flex-col gap-4 rounded-xl bg-background p-4 ring-1 ring-border">
          {harness.sign_in_hint && <p className="text-sm text-muted-foreground">{harness.sign_in_hint}</p>}
          {(job.link || job.code) && (
            <div className="flex flex-wrap items-stretch gap-3">
              {job.link && (
                <Button asChild size="xl">
                  <a href={job.link} target="_blank" rel="noreferrer noopener">
                    <ExternalLink />
                    Open the sign-in page
                  </a>
                </Button>
              )}
              {job.code && (
                <div className="flex items-center gap-1 rounded-lg bg-fill-subtle py-1 pr-1 pl-3 ring-1 ring-border ring-dashed">
                  <KeyRound className="size-3.5 text-muted-foreground" />
                  <span className="px-1.5 font-mono text-base font-semibold tracking-[0.2em] tabular-nums">{job.code}</span>
                  <CopyButton text={job.code} label="Copy" />
                </div>
              )}
            </div>
          )}
          {job.link && <p className="-mt-2 truncate font-mono text-2xs text-muted-foreground/80">{job.link}</p>}
          {!job.link && !job.code && !harness.asks && (
            <p className="flex items-center gap-2 text-xs text-muted-foreground">
              <Spinner className="size-3.5" />
              Waiting for the sign-in page to say where to go
            </p>
          )}
          {harness.asks && <Answer harness={harness} job={job} onAnswer={onAnswer} />}
        </div>
      )}

      <Log lines={job.lines} open={Boolean(failed)} />
    </section>
  );
}
