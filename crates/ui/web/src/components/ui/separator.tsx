"use client";

import { wire } from "@wirekyt/react";
import { cn } from "cn";
import * as React from "react";

function Separator({
  className,
  orientation = "horizontal",
  decorative = true,
  ...props
}: React.ComponentProps<typeof wire.div> & {
  orientation?: "horizontal" | "vertical";
  decorative?: boolean;
}) {
  return (
    <wire.div
      data-slot="separator"
      role={decorative ? "none" : "separator"}
      aria-orientation={decorative ? undefined : orientation}
      data-orientation={orientation}
      className={cn(
        "bg-border shrink-0",
        orientation === "vertical"
          ? "-mx-[0.5px] w-px self-stretch"
          : "-my-[0.5px] h-px w-full",
        className
      )}
      {...props}
    />
  );
}

export { Separator };
