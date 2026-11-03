import { wire } from "@wirekyt/react";
import { cva } from "class-variance-authority";
import type { VariantProps } from "class-variance-authority";
import { cn } from "cn";
import type * as React from "react";

const kbdVariants = cva(
  "gap-1 rounded-xs bg-fill-default font-sans text-2xs font-medium text-muted-foreground in-data-[slot=badge]:h-4 in-data-[slot=badge]:min-w-4 in-data-[slot=badge]:px-0.5 in-data-[slot=tooltip-content]:h-4 in-data-[slot=tooltip-content]:min-w-4 in-data-[slot=tooltip-content]:bg-background/20 in-data-[slot=tooltip-content]:px-0.5 in-data-[slot=tooltip-content]:text-background dark:in-data-[slot=tooltip-content]:bg-background/10 [&_svg:not([class*='size-'])]:size-3 pointer-events-none inline-flex w-fit items-center justify-center select-none",
  {
    defaultVariants: {
      size: "default",
    },
    variants: {
      size: {
        default: "h-5 min-w-5 px-1",
        sm: "h-4 min-w-4 px-0.5",
      },
    },
  }
);

const Kbd = ({
  className,
  size = "default",
  ...props
}: React.ComponentProps<typeof wire.kbd> &
  VariantProps<typeof kbdVariants>) => (
  <wire.kbd
    data-slot="kbd"
    data-size={size}
    className={cn(kbdVariants({ className, size }))}
    {...props}
  />
);

const KbdGroup = ({
  className,
  ...props
}: React.ComponentProps<typeof wire.kbd>) => (
  <wire.kbd
    data-slot="kbd-group"
    className={cn("gap-0.5 inline-flex items-center", className)}
    {...props}
  />
);

export { Kbd, KbdGroup };
