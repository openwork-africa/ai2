"use client";

import { Field } from "@wirekyt/react/field";
import { cva } from "class-variance-authority";
import type { VariantProps } from "class-variance-authority";
import { cn } from "cn";
import type * as React from "react";

const inputVariants = cva(
  "rounded-lg bg-field py-1 text-xs outline-1 -outline-offset-1 outline-field-border transition-[color,box-shadow,outline-color] duration-200 outline-solid file:h-6 file:text-xs file:font-medium file:text-foreground placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring/30 focus-visible:outline-ring aria-invalid:text-destructive-foreground aria-invalid:ring-2 aria-invalid:ring-destructive/20 aria-invalid:outline-destructive dark:aria-invalid:ring-destructive/40 dark:aria-invalid:outline-destructive/50 w-full min-w-0 file:inline-flex file:border-0 file:bg-transparent disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50",
  {
    defaultVariants: {
      size: "default",
    },
    variants: {
      size: {
        default: "h-8 px-2.5",
        lg: "h-9 px-3",
        xl: "h-10 px-3",
      },
    },
  }
);

const Input = ({
  className,
  type,
  "data-size": dataSize = "default",
  ...props
}: React.ComponentProps<typeof Field.Input> & {
  "data-size"?: VariantProps<typeof inputVariants>["size"]
}) => {
  return (
    <Field.Input
      type={type}
      data-slot="input"
      data-size={dataSize}
      className={cn(inputVariants({ size: dataSize }), className)}
      {...props}
    />
  );
};

export { Input };
