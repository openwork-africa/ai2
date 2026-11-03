import { cn } from "cn";
import { Check, Copy, type LucideIcon, TriangleAlert, Undo2 } from "lucide-react";
import { type ReactNode, useState } from "react";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { toast } from "@/components/ui/toast";
import { ApiError } from "@/lib/api";
import { dequeue, presenceNow, type Queued, useQueue } from "@/hooks/use-vault";
import { absolute, Cancelled, type Change, changeTarget, host, type PresenceKind, relative } from "@/lib/vault";

const UNCONFIRMED: Record<PresenceKind, string> = {
  touch_id: "Touch ID was not confirmed, so nothing changed.",
  polkit: "The system prompt was not confirmed, so nothing changed.",
  passphrase: "The vault did not accept the passphrase, so nothing changed.",
};

export const problem = (failure: unknown) => {
  if (failure instanceof ApiError && failure.code === "presence_required") return UNCONFIRMED[presenceNow()];
  if (failure instanceof ApiError && failure.code === "busy") return "Another vault prompt is open. Answer it first, then try again.";
  return (failure as Error).message;
};

export async function run<T>(
  work: () => Promise<T>,
  { done, after }: { done: string; after?: () => unknown },
): Promise<T | undefined> {
  try {
    const out = await work();
    toast.create({ title: done, type: "success" });
    await after?.();
    return out;
  } catch (failure) {
    if (failure instanceof Cancelled) toast.create({ title: failure.message, type: "info" });
    else toast.create({ title: "The vault said no", description: problem(failure), type: "error" });
    return undefined;
  }
}

export function Toolbar({ children, actions }: { children: ReactNode; actions?: ReactNode }) {
  return (
    <div className="flex flex-wrap items-end justify-between gap-3">
      <p className="max-w-prose text-sm text-muted-foreground">{children}</p>
      {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </div>
  );
}

export function Panel({ className, children }: { className?: string; children: ReactNode }) {
  return (
    <div
      className={cn(
        "overflow-hidden rounded-2xl border bg-card [&_thead]:bg-fill-subtle [&_th]:h-9 [&_th]:text-2xs [&_th]:font-medium [&_th]:tracking-wide [&_th]:text-muted-foreground [&_th]:uppercase",
        className,
      )}
    >
      {children}
    </div>
  );
}

export function Mono({ className, children, title }: { className?: string; children: ReactNode; title?: string }) {
  return (
    <span title={title} className={cn("font-mono text-xs [font-variant-ligatures:none]", className)}>
      {children}
    </span>
  );
}

export function Chip({ children, title, className }: { children: ReactNode; title?: string; className?: string }) {
  return (
    <span
      title={title}
      className={cn(
        "inline-flex max-w-full items-center truncate rounded-md bg-fill-default px-1.5 py-0.5 font-mono text-2xs text-foreground [font-variant-ligatures:none]",
        className,
      )}
    >
      {children}
    </span>
  );
}

export function Segmented({
  value,
  onChange,
  options,
}: {
  value: string;
  onChange: (value: string) => void;
  options: { value: string; label: string }[];
}) {
  return (
    <Tabs value={value} onValueChange={(details) => onChange(details.value)}>
      <TabsList className="w-full">
        {options.map((option) => (
          <TabsTrigger key={option.value} value={option.value} className="flex-1">
            {option.label}
          </TabsTrigger>
        ))}
      </TabsList>
    </Tabs>
  );
}

export function Urls({ urls, empty = "—" }: { urls: string[]; empty?: string }) {
  if (!urls.length) return <span className="text-xs text-muted-foreground">{empty}</span>;
  return (
    <span className="flex max-w-80 flex-wrap gap-1">
      {urls.map((url) => (
        <Chip key={url} title={url}>
          {host(url)}
        </Chip>
      ))}
    </span>
  );
}

export function When({ value, className }: { value: string | null | undefined; className?: string }) {
  if (!value) return <span className="text-muted-foreground">—</span>;
  return (
    <time dateTime={value} title={absolute(value)} className={cn("whitespace-nowrap tabular-nums", className)}>
      {relative(value)}
    </time>
  );
}

export function Rows({ count = 3 }: { count?: number }) {
  return (
    <Panel className="flex flex-col gap-3 p-4">
      {Array.from({ length: count }, (_, index) => (
        <Skeleton key={index} className="h-9 w-full rounded-lg" />
      ))}
    </Panel>
  );
}

export function Nothing({ icon: Icon, title, children }: { icon: LucideIcon; title: string; children: ReactNode }) {
  return (
    <Empty className="rounded-2xl border border-dashed py-14">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <Icon />
        </EmptyMedia>
        <EmptyTitle>{title}</EmptyTitle>
        <EmptyDescription>{children}</EmptyDescription>
      </EmptyHeader>
    </Empty>
  );
}

export function Failed({ message }: { message: string }) {
  return (
    <Panel className="flex items-start gap-3 p-4 text-sm">
      <TriangleAlert className="mt-0.5 size-4 shrink-0 text-warning" />
      <span className="text-muted-foreground">{message}</span>
    </Panel>
  );
}

export const useQueued = () => {
  const queue = useQueue();
  return (action: Change["action"], target: string) =>
    queue.find((entry) => entry.change.action === action && changeTarget(entry.change) === target);
};

export function QueuedTag({ children }: { children: ReactNode }) {
  return (
    <span className="inline-flex items-center gap-1.5 rounded-md bg-warning/10 px-1.5 py-0.5 text-2xs font-medium text-foreground">
      <span className="size-1.5 rounded-full bg-warning" />
      {children}
    </span>
  );
}

export function Undo({ entry, label }: { entry: Queued; label: string }) {
  return (
    <div className="flex flex-col items-end gap-1">
      <QueuedTag>{label}</QueuedTag>
      <Button variant="ghost" size="xs" onClick={() => dequeue(entry.id)}>
        <Undo2 data-icon="inline-start" />
        Undo
      </Button>
    </div>
  );
}

export function CopyButton({ value, label = "Copy" }: { value: string; label?: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <Button
      type="button"
      onClick={async () => {
        try {
          await navigator.clipboard.writeText(value);
          setCopied(true);
          window.setTimeout(() => setCopied(false), 1600);
        } catch {
          toast.create({ title: "Could not copy", description: "Select the text and copy it yourself.", type: "warning" });
        }
      }}
    >
      {copied ? <Check data-icon="inline-start" /> : <Copy data-icon="inline-start" />}
      {copied ? "Copied" : label}
    </Button>
  );
}
