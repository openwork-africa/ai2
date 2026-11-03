"use client";

import { Portal, wire } from "@wirekyt/react";
import { Select as SelectPrimitive } from "@wirekyt/react/select";
import { cn } from "cn";
import { CheckIcon, ChevronDownIcon, XIcon } from "lucide-react";
import type { ComponentProps } from "react";

const Select = ({
  className,
  height = "default",
  ref,
  ...props
}: ComponentProps<typeof SelectPrimitive.Root> & {
  height?: "default" | "lg" | "sm" | "xl";
}) => (
  <SelectPrimitive.Root
    ref={ref}
    data-slot="select-root"
    data-height={height}
    className={cn("data-[height=default]:h-8 data-[height=lg]:h-9 data-[height=sm]:h-7 data-[height=xl]:h-10 group/select", className)}
    {...props}
  />
);

const SelectControl = ({
  className,
  children,
  ref,
  ...props
}: ComponentProps<typeof SelectPrimitive.Control>) => (
  <SelectPrimitive.Control
    ref={ref}
    data-slot="select-control"
    className={cn(
      "relative flex h-full w-full items-center",
      "has-[>[data-slot=select-indicator]]:[&>[data-slot=select-trigger]]:pr-7",
      "has-[>[data-slot=select-indicator-group]]:[&>[data-slot=select-trigger]]:pr-14",
      className
    )}
    {...props}
  >
    {children}
  </SelectPrimitive.Control>
);

const SelectTrigger = ({
  className,
  children,
  ref,
  ...props
}: ComponentProps<typeof SelectPrimitive.Trigger>) => (
  <SelectPrimitive.Trigger
    ref={ref}
    data-slot="select-trigger"
    className={cn("flex h-full w-full items-center justify-between gap-(--sel-gap) rounded-lg bg-field pl-(--sel-gap) text-xs whitespace-nowrap outline-1 -outline-offset-1 outline-field-border transition-[color,box-shadow,outline-color] duration-200 outline-solid [--sel-gap:--spacing(3)] focus-visible:ring-2 focus-visible:ring-ring/30 focus-visible:outline-ring data-disabled:cursor-not-allowed data-disabled:opacity-50 aria-invalid:ring-2 aria-invalid:ring-destructive/20 aria-invalid:outline-destructive data-placeholder:text-muted-foreground data-placeholder-shown:text-muted-foreground group-data-[height=default]/select:pr-2 group-data-[height=default]/select:[--sel-gap:--spacing(2.5)] group-data-[height=default]/select:has-[>:first-child:not([data-slot=select-value])]:pl-2 group-data-[height=lg]/select:pr-2.5 group-data-[height=lg]/select:has-[>:first-child:not([data-slot=select-value])]:pl-2.5 group-data-[height=sm]/select:rounded-md group-data-[height=sm]/select:pr-1.5 group-data-[height=sm]/select:[--sel-gap:--spacing(2.5)] group-data-[height=sm]/select:has-[>:first-child:not([data-slot=select-value])]:pl-1.5 group-data-[height=xl]/select:pr-3 group-data-[height=xl]/select:has-[>:first-child:not([data-slot=select-value])]:pl-3 *:data-[slot=select-value]:line-clamp-1 *:data-[slot=select-value]:flex *:data-[slot=select-value]:items-center *:data-[slot=select-value]:gap-2 dark:aria-invalid:ring-destructive/40 dark:aria-invalid:outline-destructive/50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4", className)}
    {...props}
  >
    {children}
  </SelectPrimitive.Trigger>
);

const SelectIndicatorGroup = ({
  className,
  children,
  ref,
  ...props
}: ComponentProps<typeof wire.div>) => (
  <wire.div
    ref={ref}
    data-slot="select-indicator-group"
    className={cn(
      "pointer-events-none absolute inset-y-0 right-0 flex items-center gap-1 px-2.5",
      "[&>[data-slot=select-indicator]]:static [&>[data-slot=select-indicator]]:translate-y-0 [&>[data-slot=select-indicator]]:px-0",
      className
    )}
    {...props}
  >
    {children}
  </wire.div>
);

const SelectIndicator = ({
  className,
  children,
  ref,
  ...props
}: ComponentProps<typeof SelectPrimitive.Indicator>) => (
  <SelectPrimitive.Indicator
    ref={ref}
    data-slot="select-indicator"
    className={cn("text-muted-foreground pointer-events-none absolute top-1/2 right-2 flex size-4 -translate-y-1/2 items-center justify-center [&_svg]:size-full", className)}
    {...props}
  >
    {children ?? <ChevronDownIcon />}
  </SelectPrimitive.Indicator>
);

const SelectClearTrigger = ({
  className,
  children,
  ref,
  ...props
}: ComponentProps<typeof SelectPrimitive.ClearTrigger>) => (
  <SelectPrimitive.ClearTrigger
    ref={ref}
    data-slot="select-clear-trigger"
    className={cn(
      "text-muted-foreground/60 hover:text-foreground pointer-events-auto flex shrink-0 items-center justify-center rounded-sm transition-colors [&_svg]:size-3.5",
      className
    )}
    {...props}
  >
    {children ?? <XIcon />}
  </SelectPrimitive.ClearTrigger>
);

const SelectValue = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof SelectPrimitive.ValueText>) => (
  <SelectPrimitive.ValueText
    ref={ref}
    data-slot="select-value"
    className={cn("flex flex-1 text-left", className)}
    {...props}
  />
);

const SelectPositioner = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof SelectPrimitive.Positioner>) => (
  <SelectPrimitive.Positioner
    ref={ref}
    data-slot="select-positioner"
    className={cn("outline-none", className)}
    {...props}
  />
);

const SelectContent = ({
  className,
  children,
  ref,
  ...props
}: ComponentProps<typeof SelectPrimitive.Content>) => (
  <Portal>
    <SelectPrimitive.Positioner
      data-slot="select-positioner"
      className="outline-none"
    >
      <SelectPrimitive.Content
        ref={ref}
        data-slot="select-content"
        className={cn(
          "bg-popover text-popover-foreground data-open:animate-in data-closed:animate-out data-closed:fade-out-0 data-open:fade-in-0 data-closed:zoom-out-95 data-open:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 relative isolate z-50 flex max-h-(--available-height) w-(--reference-width) min-w-36 origin-(--transform-origin) flex-col gap-0.5 overflow-x-hidden overflow-y-auto rounded-14 p-1 shadow-(--shadow-popover) ring-1 ring-border outline-none duration-100 has-[>[data-slot=select-item-group]]:p-0 *:data-[slot=select-item]:scroll-my-1.5 data-[side=inline-start]:slide-in-from-right-2 data-[side=inline-end]:slide-in-from-left-2",
          className
        )}
        {...props}
      >
        {children}
      </SelectPrimitive.Content>
    </SelectPrimitive.Positioner>
  </Portal>
);

const SelectHiddenSelect = SelectPrimitive.HiddenSelect;

const SelectItem = ({
  className,
  children,
  ref,
  ...props
}: ComponentProps<typeof SelectPrimitive.Item>) => (
  <SelectPrimitive.Item
    ref={ref}
    data-slot="select-item"
    className={cn("focus:bg-accent data-highlighted:bg-accent focus:text-accent-foreground data-highlighted:text-accent-foreground not-data-[variant=destructive]:focus:**:text-accent-foreground not-data-[variant=destructive]:data-highlighted:**:text-accent-foreground relative flex w-full cursor-default items-center gap-2 rounded-lg py-1.5 pr-8 pl-2 text-sm outline-hidden select-none data-disabled:pointer-events-none data-disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4 *:[span]:last:flex *:[span]:last:items-center *:[span]:last:gap-2", className)}
    {...props}
  >
    {children}
  </SelectPrimitive.Item>
);

const SelectItemText = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof SelectPrimitive.ItemText>) => (
  <SelectPrimitive.ItemText
    ref={ref}
    data-slot="select-item-text"
    className={cn("flex flex-1 shrink-0 items-center gap-2 whitespace-nowrap", className)}
    {...props}
  />
);

const SelectItemIndicator = ({
  className,
  children,
  ref,
  ...props
}: ComponentProps<typeof SelectPrimitive.ItemIndicator>) => (
  <SelectPrimitive.ItemIndicator
    ref={ref}
    data-slot="select-item-indicator"
    className={cn("pointer-events-none absolute right-2 flex size-4 items-center justify-center", className)}
    {...props}
  >
    {children ?? <CheckIcon />}
  </SelectPrimitive.ItemIndicator>
);

const SelectItemGroup = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof SelectPrimitive.ItemGroup>) => (
  <SelectPrimitive.ItemGroup
    ref={ref}
    data-slot="select-item-group"
    className={cn("flex scroll-my-1.5 flex-col p-1", className)}
    {...props}
  />
);

const SelectItemGroupLabel = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof SelectPrimitive.ItemGroupLabel>) => (
  <SelectPrimitive.ItemGroupLabel
    ref={ref}
    data-slot="select-item-group-label"
    className={cn("px-2 py-1 text-2xs text-muted-foreground", className)}
    {...props}
  />
);

const SelectLabel = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof SelectPrimitive.Label>) => (
  <SelectPrimitive.Label
    ref={ref}
    data-slot="select-label"
    className={cn(
      "text-sm leading-none font-medium select-none data-disabled:opacity-50",
      className
    )}
    {...props}
  />
);

const SelectSeparator = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof wire.div>) => (
  <wire.div
    ref={ref}
    data-slot="select-separator"
    className={cn("pointer-events-none -mx-1 mt-1 mb-[3px] h-px bg-border [&:has(+[data-slot=select-item-group])]:-mb-px [[data-slot=select-item-group]+&]:mt-0", className)}
    {...props}
  />
);

const SelectContext = SelectPrimitive.Context;
const SelectRootProvider = SelectPrimitive.RootProvider;

export {
  Select,
  SelectControl,
  SelectTrigger,
  SelectIndicator,
  SelectClearTrigger,
  SelectValue,
  SelectPositioner,
  SelectContent,
  SelectHiddenSelect,
  SelectItem,
  SelectItemText,
  SelectItemIndicator,
  SelectItemGroup,
  SelectItemGroupLabel,
  SelectLabel,
  SelectSeparator,
  SelectIndicatorGroup,
  SelectContext,
  SelectRootProvider,
};

export {
  createListCollection,
  useSelect,
  useSelectContext,
  type SelectValueChangeDetails,
  type CollectionItem,
  type ListCollection,
} from "@wirekyt/react/select";
