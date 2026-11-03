"use client";

import { RadioGroup as RadioGroupPrimitive } from "@wirekyt/react/radio-group";
import { cn } from "cn";
import * as React from "react";

const RadioGroup = React.forwardRef<
  HTMLDivElement,
  React.ComponentProps<typeof RadioGroupPrimitive.Root>
>(({ className, ...props }, ref) => (
  <RadioGroupPrimitive.Root
    ref={ref}
    data-slot="radio-group"
    className={cn("grid w-full gap-3", className)}
    {...props}
  />
));
RadioGroup.displayName = "RadioGroup";

const RadioGroupLabel = React.forwardRef<
  HTMLLabelElement,
  React.ComponentProps<typeof RadioGroupPrimitive.Label>
>(({ className, ...props }, ref) => (
  <RadioGroupPrimitive.Label
    ref={ref}
    data-slot="radio-group-label"
    className={cn("text-sm leading-none font-medium select-none", className)}
    {...props}
  />
));
RadioGroupLabel.displayName = "RadioGroupLabel";

const RadioGroupItem = React.forwardRef<
  HTMLLabelElement,
  React.ComponentProps<typeof RadioGroupPrimitive.Item>
>(({ className, ...props }, ref) => (
  <RadioGroupPrimitive.Item
    ref={ref}
    data-slot="radio-group-item"
    className={cn("inline-flex items-center gap-2 data-disabled:opacity-50", className)}
    {...props}
  />
));
RadioGroupItem.displayName = "RadioGroupItem";

const RadioGroupItemControl = React.forwardRef<
  HTMLDivElement,
  React.ComponentProps<typeof RadioGroupPrimitive.ItemControl>
>(({ className, ...props }, ref) => (
  <RadioGroupPrimitive.ItemControl
    ref={ref}
    data-slot="radio-group-item-control"
    className={cn("relative flex aspect-square size-4 shrink-0 rounded-full border border-transparent bg-fill-hover outline-none group-has-[:focus-visible]/field-label:border-transparent group-has-[:focus-visible]/field-label:ring-0 after:absolute after:-inset-x-3 after:-inset-y-2 focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30 data-focus-visible:border-ring data-focus-visible:ring-3 data-focus-visible:ring-ring/30 data-disabled:cursor-not-allowed data-disabled:opacity-50 aria-invalid:border-destructive aria-invalid:ring-3 aria-invalid:ring-destructive/20 data-invalid:border-destructive data-invalid:ring-3 data-invalid:ring-destructive/20 dark:aria-invalid:border-destructive/50 dark:aria-invalid:ring-destructive/40 dark:data-invalid:border-destructive/50 dark:data-invalid:ring-destructive/40 data-checked:bg-primary data-checked:text-primary-foreground data-checked:focus-visible:border-primary data-checked:aria-invalid:border-primary dark:data-checked:aria-invalid:border-primary data-[state=checked]:bg-primary data-[state=checked]:text-primary-foreground data-[state=checked]:data-focus-visible:border-primary data-[state=checked]:data-invalid:border-primary dark:data-[state=checked]:data-invalid:border-primary", className)}
    {...props}
  >
    <span className="absolute top-1/2 left-1/2 size-2 -translate-x-1/2 -translate-y-1/2 rounded-full bg-primary-foreground opacity-0 transition-opacity [[data-state=checked]_&]:opacity-100" />
  </RadioGroupPrimitive.ItemControl>
));
RadioGroupItemControl.displayName = "RadioGroupItemControl";

const RadioGroupItemText = React.forwardRef<
  HTMLSpanElement,
  React.ComponentProps<typeof RadioGroupPrimitive.ItemText>
>(({ className, ...props }, ref) => (
  <RadioGroupPrimitive.ItemText
    ref={ref}
    data-slot="radio-group-item-text"
    className={cn("text-sm leading-none select-none", className)}
    {...props}
  />
));
RadioGroupItemText.displayName = "RadioGroupItemText";

const RadioGroupItemHiddenInput = RadioGroupPrimitive.ItemHiddenInput;

const RadioGroupIndicator = React.forwardRef<
  HTMLDivElement,
  React.ComponentProps<typeof RadioGroupPrimitive.Indicator>
>(({ className, ...props }, ref) => (
  <RadioGroupPrimitive.Indicator
    ref={ref}
    data-slot="radio-group-indicator"
    className={cn("flex size-4 items-center justify-center", className)}
    {...props}
  />
));
RadioGroupIndicator.displayName = "RadioGroupIndicator";

const RadioGroupContext = RadioGroupPrimitive.Context;
const RadioGroupRootProvider = RadioGroupPrimitive.RootProvider;

export {
  RadioGroup,
  RadioGroupLabel,
  RadioGroupItem,
  RadioGroupItemControl,
  RadioGroupItemText,
  RadioGroupItemHiddenInput,
  RadioGroupIndicator,
  RadioGroupContext,
  RadioGroupRootProvider,
};
export {
  useRadioGroup,
  useRadioGroupContext,
  type RadioGroupValueChangeDetails,
} from "@wirekyt/react/radio-group";
