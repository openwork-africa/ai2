"use client";

import { wire } from "@wirekyt/react";
import { cva } from "class-variance-authority";
import type { VariantProps } from "class-variance-authority";
import { cn } from "cn";
import * as React from "react";

const badgeVariants = cva(
  "inline-flex w-fit shrink-0 items-center justify-center gap-1 overflow-hidden whitespace-nowrap rounded-full py-0.5 text-2xs font-medium outline-1 -outline-offset-1 outline-transparent transition-all outline-solid focus-visible:ring-3 focus-visible:ring-ring/30 focus-visible:outline-ring aria-invalid:ring-destructive/20 aria-invalid:outline-destructive dark:aria-invalid:ring-destructive/40 [&>svg]:pointer-events-none [&>svg]:size-3!",
  {
    defaultVariants: {
      size: "default",
      variant: "default",
    },
    variants: {
      size: {
        default: "h-5 min-w-5 px-2 has-data-[icon=inline-end]:pr-1.5 has-data-[icon=inline-start]:pl-1.5",
        lg: "h-6 px-2.5 has-data-[icon=inline-end]:pr-2 has-data-[icon=inline-start]:pl-2 [&>svg]:size-4!",
      },
      variant: {
        default: "bg-primary text-primary-foreground [a]:hover:bg-primary/80",
        destructive: "bg-destructive/10 text-destructive-foreground focus-visible:ring-destructive/20 focus-visible:outline-destructive/50 [a]:hover:bg-destructive/15",
        ghost: "text-foreground hover:bg-fill-default",
        link: "text-primary underline underline-offset-4",
        outline: "bg-control text-foreground outline-1 -outline-offset-1 outline-border [a]:hover:bg-fill-default",
        secondary: "bg-fill-default text-foreground",
      },
    },
  }
);

const Badge = ({
  className,
  variant = "default",
  size = "default",
  ...props
}: React.ComponentProps<typeof wire.span> &
  VariantProps<typeof badgeVariants>) => (
  <wire.span
    data-slot="badge"
    data-variant={variant}
    data-size={size}
    className={cn(badgeVariants({ size, variant }), className)}
    {...props}
  />
);

export { Badge, badgeVariants };
