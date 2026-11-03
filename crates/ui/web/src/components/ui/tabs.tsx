"use client";

import { Tabs as TabsPrimitive } from "@wirekyt/react/tabs";
import { cva } from "class-variance-authority";
import type { VariantProps } from "class-variance-authority";
import { cn } from "cn";
import type { ComponentProps } from "react";

const Tabs = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof TabsPrimitive.Root>) => (
  <TabsPrimitive.Root
    ref={ref}
    data-slot="tabs"
    className={cn("flex gap-2 data-[orientation=horizontal]:flex-col group/tabs", className)}
    {...props}
  />
);

const tabsListVariants = cva(
  "text-muted-foreground relative isolate inline-flex w-fit items-center justify-center rounded-lg p-0.5 group-data-[orientation=horizontal]/tabs:h-8 group-data-[orientation=horizontal]/tabs:has-data-[layout=stacked]:h-auto group-data-[orientation=vertical]/tabs:h-fit group-data-[orientation=vertical]/tabs:flex-col data-[size=lg]:group-data-[orientation=horizontal]/tabs:h-9 data-[variant=line]:rounded-none group/tabs-list",
  {
    defaultVariants: {
      size: "default",
      variant: "default",
    },
    variants: {
      size: {
        default: "",
        lg: "",
      },
      variant: {
        default: "bg-fill-default",
        line: "bg-transparent",
        primary: "bg-transparent",
        secondary: "bg-transparent",
      },
    },
  }
);

const TabsList = ({
  className,
  variant = "default",
  size = "default",
  ref,
  ...props
}: ComponentProps<typeof TabsPrimitive.List> &
  VariantProps<typeof tabsListVariants>) => (
  <TabsPrimitive.List
    ref={ref}
    data-slot="tabs-list"
    data-variant={variant}
    data-size={size}
    className={cn(tabsListVariants({ size, variant }), className)}
    {...props}
  />
);

const tabsTriggerVariants = cva(
  "text-muted-foreground hover:text-foreground focus-visible:ring-ring/30 focus-visible:outline-ring relative inline-flex rounded-md border border-transparent text-xs font-medium whitespace-nowrap transition-all focus-visible:ring-3 focus-visible:outline-1 disabled:pointer-events-none disabled:opacity-50 data-disabled:pointer-events-none data-disabled:opacity-50 data-selected:text-foreground group-data-[orientation=vertical]/tabs:w-full group-data-[variant=default]/tabs-list:data-selected:bg-raised group-data-[variant=default]/tabs-list:data-selected:bg-linear-to-b group-data-[variant=default]/tabs-list:data-selected:from-white/10 group-data-[variant=default]/tabs-list:data-selected:to-transparent group-data-[variant=default]/tabs-list:data-selected:shadow-xs group-data-[variant=secondary]/tabs-list:data-selected:bg-fill-default group-data-[variant=primary]/tabs-list:data-selected:bg-primary group-data-[variant=primary]/tabs-list:data-selected:text-primary-foreground group-data-[variant=line]/tabs-list:rounded-none [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4 after:bg-foreground after:absolute after:opacity-0 after:transition-opacity group-data-[orientation=horizontal]/tabs:after:inset-x-2 group-data-[orientation=horizontal]/tabs:after:bottom-0 group-data-[orientation=horizontal]/tabs:after:h-0.5 group-data-[orientation=vertical]/tabs:after:inset-y-1 group-data-[orientation=vertical]/tabs:after:right-0 group-data-[orientation=vertical]/tabs:after:w-0.5 group-data-[variant=line]/tabs-list:data-selected:after:opacity-100 group/tabs-trigger",
  {
    defaultVariants: {
      align: "start",
      layout: "inline",
    },
    variants: {
      align: {
        center: "items-center text-center",
        start: "",
      },
      layout: {
        inline: "h-7 items-center justify-center gap-1 px-1.75 group-data-[size=lg]/tabs-list:h-8 group-data-[size=lg]/tabs-list:gap-1.5 group-data-[size=lg]/tabs-list:px-2.25 group-data-[orientation=vertical]/tabs:justify-start",
        stacked: "flex-col items-start justify-start gap-2 px-2.25 py-1.75 text-left",
      },
    },
  }
);

const TabsTrigger = ({
  align = "start",
  className,
  layout = "inline",
  ref,
  ...props
}: ComponentProps<typeof TabsPrimitive.Trigger> &
  VariantProps<typeof tabsTriggerVariants>) => (
  <TabsPrimitive.Trigger
    ref={ref}
    data-slot="tabs-trigger"
    data-layout={layout}
    className={cn(tabsTriggerVariants({ align, layout }), className)}
    {...props}
  />
);

const TabsContent = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof TabsPrimitive.Content>) => (
  <TabsPrimitive.Content
    ref={ref}
    data-slot="tabs-content"
    className={cn("flex-1 text-sm outline-none", className)}
    {...props}
  />
);

const TabsTriggerDescription = ({
  className,
  ...props
}: ComponentProps<"span">) => (
  <span
    data-slot="tabs-trigger-description"
    className={cn("text-muted-foreground text-2xs font-normal group-data-[variant=primary]/tabs-list:group-data-selected/tabs-trigger:text-primary-foreground/70", className)}
    {...props}
  />
);

const TabsTriggerCount = ({
  className,
  ...props
}: ComponentProps<"span">) => (
  <span
    data-slot="tabs-trigger-count"
    className={cn("text-muted-foreground font-normal tabular-nums group-data-[variant=primary]/tabs-list:group-data-selected/tabs-trigger:text-primary-foreground/70", className)}
    {...props}
  />
);

const TabsIndicator = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof TabsPrimitive.Indicator>) => (
  <TabsPrimitive.Indicator
    ref={ref}
    data-slot="tabs-indicator"
    className={cn("absolute -z-10 h-(--height) w-(--width) rounded-md bg-raised bg-linear-to-b from-white/10 to-transparent shadow-xs transition-all duration-200 ease-out motion-reduce:transition-none group-data-[variant=secondary]/tabs-list:bg-fill-default group-data-[variant=primary]/tabs-list:bg-primary group-data-[variant=line]/tabs-list:hidden", className)}
    {...props}
  />
);

const TabsContext = TabsPrimitive.Context;
const TabsRootProvider = TabsPrimitive.RootProvider;

export {
  Tabs,
  TabsContent,
  TabsIndicator,
  TabsList,
  TabsTrigger,
  TabsTriggerCount,
  TabsTriggerDescription,
  tabsTriggerVariants,
  tabsListVariants,
  TabsContext,
  TabsRootProvider,
};

export {
  useTabs,
  useTabsContext,
  type TabsValueChangeDetails,
} from "@wirekyt/react/tabs";
