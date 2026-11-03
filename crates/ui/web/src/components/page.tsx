import { cn } from "cn";
import { ChevronLeft } from "lucide-react";
import type * as React from "react";
import { Link } from "react-router";

export function Page({ className, ...props }: React.ComponentProps<"div">) {
  return <div className={cn("mx-auto flex w-full max-w-5xl flex-col gap-8 px-5 py-8 md:px-10 md:py-12", className)} {...props} />;
}

export function PageHeader({
  title,
  description,
  back,
  actions,
  children,
}: {
  title: React.ReactNode;
  description?: React.ReactNode;
  back?: { to: string; label: string };
  actions?: React.ReactNode;
  children?: React.ReactNode;
}) {
  return (
    <header className="flex flex-col gap-3">
      {back && (
        <Link
          to={back.to}
          className="-ml-1 flex w-fit items-center gap-0.5 rounded-md px-1 text-sm text-muted-foreground transition-colors hover:text-foreground"
        >
          <ChevronLeft className="size-4" />
          {back.label}
        </Link>
      )}
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="flex min-w-0 flex-col gap-1">
          <h1 className="font-heading text-2xl font-semibold tracking-tight">{title}</h1>
          {description && <p className="max-w-prose text-sm text-muted-foreground">{description}</p>}
        </div>
        {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
      </div>
      {children}
    </header>
  );
}

export function Code({ className, ...props }: React.ComponentProps<"code">) {
  return (
    <code
      className={cn("rounded-md bg-fill-default px-1.5 py-0.5 font-mono text-[0.8125em] text-foreground [font-feature-settings:normal] [font-variant-ligatures:none]", className)}
      {...props}
    />
  );
}

export function Dot({ tone, className }: { tone: "success" | "warning" | "destructive" | "muted"; className?: string }) {
  const color = {
    success: "bg-success",
    warning: "bg-warning",
    destructive: "bg-destructive",
    muted: "bg-muted-foreground/50",
  }[tone];
  return (
    <span aria-hidden className={cn("size-2 shrink-0 rounded-full ring-2 ring-background", color, className)} />
  );
}
