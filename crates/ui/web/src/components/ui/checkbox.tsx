"use client";

import { Checkbox as CheckboxPrimitive } from "@wirekyt/react/checkbox";
import { cn } from "cn";
import { CheckIcon, MinusIcon } from "lucide-react";
import type { ComponentProps } from "react";

const Checkbox = ({
  className,
  children,
  ref,
  ...props
}: ComponentProps<typeof CheckboxPrimitive.Root>) => (
  <CheckboxPrimitive.Root
    ref={ref}
    data-slot="checkbox"
    className={cn("inline-flex items-center gap-2 data-[disabled]:opacity-50", className)}
    {...props}
  >
    <CheckboxPrimitive.Control
      data-slot="checkbox-control"
      className="relative flex size-4 shrink-0 items-center justify-center rounded-[5px] border border-transparent bg-fill-hover outline-none transition-shadow group-has-disabled/field:opacity-50 after:absolute after:-inset-x-3 after:-inset-y-2 focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30 data-focus-visible:border-ring data-focus-visible:ring-3 data-focus-visible:ring-ring/30 data-invalid:border-destructive data-invalid:ring-3 data-invalid:ring-destructive/20 data-invalid:aria-checked:border-primary dark:data-invalid:border-destructive/50 dark:data-invalid:ring-destructive/40 dark:data-invalid:aria-checked:border-primary data-checked:border-primary data-checked:bg-primary data-checked:text-primary-foreground data-indeterminate:text-muted-foreground group-has-[:focus-visible]/field-label:ring-0 group-has-[:focus-visible]/field-label:not-data-checked:border-transparent group-has-[:focus-visible]/field-label:data-checked:border-primary data-checked:focus-visible:border-primary data-checked:data-focus-visible:border-primary data-disabled:cursor-not-allowed data-disabled:opacity-50"
    >
      <CheckboxPrimitive.Indicator
        data-slot="checkbox-indicator"
        className="grid place-content-center text-current transition-none [&>svg]:size-3.5"
      >
        <CheckIcon />
      </CheckboxPrimitive.Indicator>
      <CheckboxPrimitive.Indicator
        data-slot="checkbox-indicator"
        className="grid place-content-center text-current transition-none [&>svg]:size-3.5"
        indeterminate
      >
        <MinusIcon />
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Control>
    {children && (
      <CheckboxPrimitive.Label
        data-slot="checkbox-label"
        className="text-sm leading-none font-medium select-none data-invalid:text-destructive-foreground"
      >
        {children}
      </CheckboxPrimitive.Label>
    )}
    <CheckboxPrimitive.HiddenInput />
  </CheckboxPrimitive.Root>
);

const CheckboxRoot = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof CheckboxPrimitive.Root>) => (
  <CheckboxPrimitive.Root
    ref={ref}
    data-slot="checkbox"
    className={cn("inline-flex items-center gap-2 data-[disabled]:opacity-50", className)}
    {...props}
  />
);

const CheckboxControl = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof CheckboxPrimitive.Control>) => (
  <CheckboxPrimitive.Control
    ref={ref}
    data-slot="checkbox-control"
    className={cn("relative flex size-4 shrink-0 items-center justify-center rounded-[5px] border border-transparent bg-fill-hover outline-none transition-shadow group-has-disabled/field:opacity-50 after:absolute after:-inset-x-3 after:-inset-y-2 focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30 data-focus-visible:border-ring data-focus-visible:ring-3 data-focus-visible:ring-ring/30 data-invalid:border-destructive data-invalid:ring-3 data-invalid:ring-destructive/20 data-invalid:aria-checked:border-primary dark:data-invalid:border-destructive/50 dark:data-invalid:ring-destructive/40 dark:data-invalid:aria-checked:border-primary data-checked:border-primary data-checked:bg-primary data-checked:text-primary-foreground data-indeterminate:text-muted-foreground group-has-[:focus-visible]/field-label:ring-0 group-has-[:focus-visible]/field-label:not-data-checked:border-transparent group-has-[:focus-visible]/field-label:data-checked:border-primary data-checked:focus-visible:border-primary data-checked:data-focus-visible:border-primary data-disabled:cursor-not-allowed data-disabled:opacity-50", className)}
    {...props}
  />
);

const CheckboxIndicator = ({
  className,
  children,
  ref,
  ...props
}: ComponentProps<typeof CheckboxPrimitive.Indicator>) => (
  <CheckboxPrimitive.Indicator
    ref={ref}
    data-slot="checkbox-indicator"
    className={cn("grid place-content-center text-current transition-none [&>svg]:size-3.5", className)}
    {...props}
  >
    {children ?? <CheckIcon />}
  </CheckboxPrimitive.Indicator>
);

const CheckboxLabel = ({
  className,
  ref,
  ...props
}: ComponentProps<typeof CheckboxPrimitive.Label>) => (
  <CheckboxPrimitive.Label
    ref={ref}
    data-slot="checkbox-label"
    className={cn("text-sm leading-none font-medium select-none data-invalid:text-destructive-foreground", className)}
    {...props}
  />
);

const CheckboxHiddenInput = CheckboxPrimitive.HiddenInput;

const CheckboxGroup = CheckboxPrimitive.Group;

const CheckboxContext = CheckboxPrimitive.Context;
const CheckboxRootProvider = CheckboxPrimitive.RootProvider;

export {
  Checkbox,
  CheckboxRoot,
  CheckboxControl,
  CheckboxIndicator,
  CheckboxLabel,
  CheckboxHiddenInput,
  CheckboxGroup,
  CheckboxContext,
  CheckboxRootProvider,
};
export {
  useCheckbox,
  useCheckboxContext,
  type CheckboxCheckedChangeDetails,
} from "@wirekyt/react/checkbox";
