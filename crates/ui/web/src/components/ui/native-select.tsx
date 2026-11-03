import { wire } from "@wirekyt/react";
import { Field } from "@wirekyt/react/field";
import { cva } from "class-variance-authority";
import type { VariantProps } from "class-variance-authority";
import { cn } from "cn";
import { ChevronDownIcon } from "lucide-react";
import type * as React from "react";

const nativeSelectVariants = cva(
  "rounded-lg bg-field text-xs outline-1 -outline-offset-1 outline-field-border transition-[color,box-shadow,outline-color] duration-200 outline-solid selection:bg-primary selection:text-primary-foreground placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring/30 focus-visible:outline-ring aria-invalid:ring-2 aria-invalid:ring-destructive/20 aria-invalid:outline-destructive dark:aria-invalid:ring-destructive/40 dark:aria-invalid:outline-destructive/50 w-full min-w-0 appearance-none select-none disabled:pointer-events-none disabled:cursor-not-allowed",
  {
    defaultVariants: {
      size: "default",
    },
    variants: {
      size: {
        default: "h-8 pr-8.5 pl-2.5",
        lg: "h-9 pr-9.5 pl-3",
        sm: "h-7 pr-8 pl-2.5",
        xl: "h-10 pr-10 pl-3",
      },
    },
  }
);

type NativeSelectProps = Omit<
  React.ComponentProps<typeof Field.Select>,
  "size"
> &
  VariantProps<typeof nativeSelectVariants>;

const NativeSelect = ({
  className,
  size = "default",
  ...props
}: NativeSelectProps) => (
  <wire.div
    className={cn(
      "has-[select:disabled]:opacity-50 group/native-select relative w-fit",
      className
    )}
    data-slot="native-select-wrapper"
    data-size={size}
  >
    <Field.Select
      data-slot="native-select"
      data-size={size}
      className={cn(nativeSelectVariants({ size }))}
      {...props}
    />
    <ChevronDownIcon
      className="top-1/2 right-2 size-4 -translate-y-1/2 text-muted-foreground group-data-[size=sm]/native-select:right-1.5 group-data-[size=lg]/native-select:right-2.5 group-data-[size=xl]/native-select:right-3 pointer-events-none absolute select-none"
      aria-hidden="true"
      data-slot="native-select-icon"
    />
  </wire.div>
);

const NativeSelectOption = ({
  className,
  ...props
}: React.ComponentProps<typeof wire.option>) => (
  <wire.option
    data-slot="native-select-option"
    className={cn("bg-[Canvas] text-[CanvasText]", className)}
    {...props}
  />
);

const NativeSelectOptGroup = ({
  className,
  ...props
}: React.ComponentProps<typeof wire.optgroup>) => (
  <wire.optgroup
    data-slot="native-select-optgroup"
    className={cn("bg-[Canvas] text-[CanvasText]", className)}
    {...props}
  />
);

export { NativeSelect, NativeSelectOptGroup, NativeSelectOption };
