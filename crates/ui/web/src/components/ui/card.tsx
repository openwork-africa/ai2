"use client";

import { wire } from "@wirekyt/react";
import { cn } from "cn";
import * as React from "react";

const Card = ({
  className,
  size = "default",
  ...props
}: React.ComponentProps<typeof wire.div> & { size?: "default" | "sm" }) => (
  <wire.div
    data-slot="card"
    data-size={size}
    className={cn("flex flex-col gap-(--card-spacing) overflow-hidden rounded-3xl bg-card py-(--card-spacing) text-sm text-card-foreground shadow-(--shadow-card) ring-1 ring-border [--card-spacing:--spacing(5)] has-[>img:first-child]:pt-0 data-[size=sm]:[--card-spacing:--spacing(4)] *:[img:first-child]:rounded-t-3xl *:[img:last-child]:rounded-b-3xl group/card", className)}
    {...props}
  />
);

const CardHeader = ({
  className,
  ...props
}: React.ComponentProps<typeof wire.div>) => (
  <wire.div
    data-slot="card-header"
    className={cn(
      "grid auto-rows-min items-start gap-x-3 gap-y-1.5 rounded-t-3xl px-(--card-spacing) has-data-[slot=card-action]:grid-cols-[1fr_auto] has-data-[slot=card-description]:grid-rows-[auto_auto] [.border-b]:pb-(--card-spacing) group/card-header @container/card-header",
      className
    )}
    {...props}
  />
);

const CardTitle = ({
  className,
  ...props
}: React.ComponentProps<typeof wire.div>) => (
  <wire.div
    data-slot="card-title"
    className={cn("font-heading text-base font-medium", className)}
    {...props}
  />
);

const CardDescription = ({
  className,
  ...props
}: React.ComponentProps<typeof wire.div>) => (
  <wire.div
    data-slot="card-description"
    className={cn("text-sm text-muted-foreground", className)}
    {...props}
  />
);

const CardAction = ({
  className,
  ...props
}: React.ComponentProps<typeof wire.div>) => (
  <wire.div
    data-slot="card-action"
    className={cn("col-start-2 row-span-2 row-start-1 self-start justify-self-end", className)}
    {...props}
  />
);

const CardContent = ({
  className,
  ...props
}: React.ComponentProps<typeof wire.div>) => (
  <wire.div
    data-slot="card-content"
    className={cn("px-(--card-spacing)", className)}
    {...props}
  />
);

const CardFooter = ({
  className,
  ...props
}: React.ComponentProps<typeof wire.div>) => (
  <wire.div
    data-slot="card-footer"
    className={cn("flex items-center gap-2 rounded-b-3xl px-(--card-spacing) [.border-t]:pt-(--card-spacing)", className)}
    {...props}
  />
);

export {
  Card,
  CardHeader,
  CardFooter,
  CardTitle,
  CardAction,
  CardDescription,
  CardContent,
};
