"use client";

import { wire } from "@wirekyt/react";
import { cva } from "class-variance-authority";
import type { VariantProps } from "class-variance-authority";
import { cn } from "cn";
import type * as React from "react";

const buttonVariants = cva(
  "inline-flex shrink-0 items-center justify-center whitespace-nowrap text-xs font-medium outline-1 -outline-offset-1 outline-transparent transition-all select-none outline-solid focus-visible:ring-2 focus-visible:ring-ring/30 focus-visible:outline-ring active:not-aria-[haspopup]:translate-y-px disabled:pointer-events-none disabled:opacity-50 aria-invalid:ring-2 aria-invalid:ring-destructive/20 aria-invalid:outline-destructive dark:aria-invalid:ring-destructive/40 dark:aria-invalid:outline-destructive/50 **:data-badge:inline-flex **:data-badge:h-5 **:data-badge:min-w-5 **:data-badge:shrink-0 **:data-badge:items-center **:data-badge:justify-center **:data-badge:rounded-xs **:data-badge:px-1 **:data-badge:text-2xs **:data-badge:font-medium [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
  {
    defaultVariants: {
      size: "default",
      variant: "default",
    },
    variants: {
      size: {
        default: "h-8 gap-1.5 rounded-lg px-2.5 has-data-[icon=inline-end]:pr-2 has-data-[icon=inline-start]:pl-2",
        icon: "size-8 rounded-lg",
        "icon-lg": "size-9 rounded-lg",
        "icon-sm": "size-7 rounded-md",
        "icon-xl": "size-10 rounded-lg",
        "icon-xs": "size-6 rounded-md [&_svg:not([class*='size-'])]:size-3.5",
        lg: "h-9 gap-1.5 rounded-lg px-3 has-data-[icon=inline-end]:pr-2.5 has-data-[icon=inline-start]:pl-2.5",
        sm: "h-7 gap-1.5 rounded-md px-2.5 has-data-[icon=inline-end]:pr-2 has-data-[icon=inline-start]:pl-2",
        xl: "h-10 gap-2 rounded-lg px-3.5 has-data-[icon=inline-end]:pr-3 has-data-[icon=inline-start]:pl-3",
        xs: "h-6 gap-1 rounded-md px-2 has-data-[icon=inline-end]:pr-1.5 has-data-[icon=inline-start]:pl-1.5 **:data-badge:h-4 **:data-badge:min-w-4 **:data-badge:px-0.5 [&_svg:not([class*='size-'])]:size-3.5",
      },
      variant: {
        default: "bg-primary bg-linear-to-b from-white/20 to-transparent to-[67.7%] text-primary-foreground shadow-(--shadow-primary-control) outline-primary-border hover:from-white/35 disabled:bg-muted-foreground disabled:from-white/35 disabled:shadow-(--shadow-primary-control-disabled) disabled:outline-primary-border-disabled **:data-badge:bg-primary-foreground/20",
        destructive: "text-destructive-foreground outline-destructive/50 hover:bg-fill-subtle focus-visible:ring-destructive/20 focus-visible:outline-destructive disabled:bg-fill-subtle aria-expanded:bg-fill-subtle **:data-badge:bg-destructive/10",
        ghost: "text-foreground hover:bg-fill-default aria-expanded:bg-fill-default **:data-badge:bg-fill-default",
        link: "text-primary underline-offset-4 hover:underline **:data-badge:bg-primary/10",
        outline: "bg-control bg-linear-to-b from-white/10 to-transparent to-[67.7%] text-foreground shadow-(--shadow-secondary-control) outline-control-border hover:bg-control-hover hover:from-white/8 disabled:bg-control-hover disabled:from-white/8 aria-expanded:bg-control-hover aria-expanded:from-white/8 **:data-badge:bg-fill-default",
        secondary: "bg-fill-default text-foreground hover:bg-fill-hover disabled:bg-fill-default aria-expanded:bg-fill-hover **:data-badge:bg-fill-default",
      },
    },
  }
);

const Button = ({
  className,
  variant = "default",
  size = "default",
  ...props
}: React.ComponentProps<typeof wire.button> &
  VariantProps<typeof buttonVariants>) => (
  <wire.button
    data-slot="button"
    data-variant={variant}
    data-size={size}
    className={cn(buttonVariants({ className, size, variant }))}
    {...props}
  />
);

export { Button, buttonVariants };
