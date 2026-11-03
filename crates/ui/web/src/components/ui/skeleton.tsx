import { wire } from "@wirekyt/react";
import { cn } from "cn";
import * as React from "react";

function Skeleton({
  className,
  ...props
}: React.ComponentProps<typeof wire.div>) {
  return (
    <wire.div
      data-slot="skeleton"
      className={cn("animate-pulse rounded-2xl bg-fill-hover", className)}
      {...props}
    />
  );
}

export { Skeleton };
