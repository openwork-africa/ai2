import { cn } from "cn";
import { useEffect, useState } from "react";
import { Dot } from "@/components/page";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { type App, type AppApi, apiStanding, iconUrl, standing } from "@/lib/apps";

const letter = (name: string) => (name.match(/[\p{L}\p{N}]/u)?.[0] ?? "?").toUpperCase();

export function AppIcon({ app, size = "md", className }: { app: Pick<App, "id" | "name" | "updated">; size?: "md" | "lg"; className?: string }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    iconUrl(app.id, app.updated)
      .then((url) => alive && setSrc(url))
      .catch(() => alive && setSrc(null));
    return () => {
      alive = false;
    };
  }, [app.id, app.updated]);
  return (
    <span
      className={cn(
        "flex shrink-0 items-center justify-center overflow-hidden bg-white shadow-(--shadow-secondary-control) ring-1 ring-black/8 dark:ring-white/10",
        size === "lg" ? "size-14 rounded-2xl" : "size-11 rounded-xl",
        className,
      )}
    >
      {src ? (
        <img src={src} alt="" className={size === "lg" ? "size-9" : "size-7"} />
      ) : (
        <span className="font-heading text-base font-semibold tracking-tight text-neutral-800">{letter(app.name)}</span>
      )}
    </span>
  );
}

export function StandingBadge({ app, className }: { app: App; className?: string }) {
  const { tone, label } = standing(app);
  return (
    <Badge variant="outline" className={cn("gap-1.5 pl-1.5", className)}>
      <Dot tone={tone} className={cn("ring-0", tone === "success" && "animate-pulse [animation-duration:2.4s]")} />
      {label}
    </Badge>
  );
}

export function ApiBadge({ api, className }: { api: AppApi; className?: string }) {
  const { tone, label } = apiStanding(api);
  return (
    <Badge variant="outline" className={cn("gap-1.5 pl-1.5 font-normal", className)} title={api.error}>
      <Dot tone={tone} className="ring-0" />
      <span className="font-medium">OpenAPI</span>
      <span className="text-muted-foreground">{label}</span>
    </Badge>
  );
}

export function RemoveDialog({
  app,
  open,
  onOpenChange,
  onRemove,
}: {
  app: App;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onRemove: () => void;
}) {
  return (
    <AlertDialog open={open} onOpenChange={({ open }) => onOpenChange(open)}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Remove {app.name}?</AlertDialogTitle>
          <AlertDialogDescription>
            The app, its icon and its OpenAPI copy leave the shelf. {app.running ? "The app itself keeps running until you stop it." : "The folder isn't touched."}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction variant="destructive" onClick={onRemove}>
            Remove
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
