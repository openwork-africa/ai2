import { cn } from "cn";
import { Check, Copy, type LucideIcon, TriangleAlert } from "lucide-react";
import { type ReactNode, useState } from "react";
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogMedia,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { toast } from "@/components/ui/toast";
import { ApiError } from "@/lib/api";
import { absolute, Cancelled, host, relative } from "@/lib/vault";

export const problem = (failure: unknown, mac: boolean) => {
  if (failure instanceof ApiError && failure.code === "presence_required")
    return mac ? "Touch ID was not confirmed, so nothing changed." : "The vault did not accept the passphrase, so nothing changed.";
  return (failure as Error).message;
};

export async function run<T>(
  work: () => Promise<T>,
  { done, mac, after }: { done: string; mac: boolean; after?: () => unknown },
): Promise<T | undefined> {
  try {
    const out = await work();
    toast.create({ title: done, type: "success" });
    await after?.();
    return out;
  } catch (failure) {
    if (failure instanceof Cancelled) toast.create({ title: failure.message, type: "info" });
    else toast.create({ title: "The vault said no", description: problem(failure, mac), type: "error" });
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

export function Confirm({
  open,
  onOpenChange,
  icon: Icon,
  title,
  children,
  action,
  onConfirm,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  icon: LucideIcon;
  title: string;
  children: ReactNode;
  action: string;
  onConfirm: () => void;
}) {
  return (
    <AlertDialog open={open} onOpenChange={(details) => onOpenChange(details.open)}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogMedia className="bg-destructive/10 text-destructive">
            <Icon />
          </AlertDialogMedia>
          <AlertDialogTitle>{title}</AlertDialogTitle>
          <AlertDialogDescription>{children}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <Button
            className="bg-destructive text-white outline-destructive hover:bg-destructive/90"
            onClick={() => {
              onOpenChange(false);
              onConfirm();
            }}
          >
            {action}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
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
