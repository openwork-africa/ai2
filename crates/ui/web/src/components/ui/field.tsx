"use client";

import { wire } from "@wirekyt/react";
import { Field as FieldPrimitive } from "@wirekyt/react/field";
import { Fieldset as FieldsetPrimitive } from "@wirekyt/react/fieldset";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "cn";
import * as React from "react";
import { useMemo } from "react";

import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";

function FieldSet({
  className,
  ...props
}: React.ComponentProps<typeof FieldsetPrimitive.Root>) {
  return (
    <FieldsetPrimitive.Root
      data-slot="field-set"
      className={cn("cn-field-set flex flex-col gap-5", className)}
      {...props}
    />
  );
}

function FieldLegend({
  className,
  variant = "legend",
  ...props
}: React.ComponentProps<typeof FieldsetPrimitive.Legend> & {
  variant?: "legend" | "label";
}) {
  return (
    <FieldsetPrimitive.Legend
      data-slot="field-legend"
      data-variant={variant}
      className={cn("cn-field-legend mb-5 font-medium data-[variant=label]:text-sm data-[variant=legend]:text-base", className)}
      {...props}
    />
  );
}

function FieldGroup({
  className,
  ...props
}: React.ComponentProps<typeof wire.div>) {
  return (
    <wire.div
      data-slot="field-group"
      className={cn("cn-field-group flex w-full flex-col gap-5 data-[slot=checkbox-group]:gap-3 *:data-[slot=field-group]:gap-4", className)}
      {...props}
    />
  );
}

const fieldVariants = cva("cn-field flex w-full gap-2 data-[invalid=true]:text-destructive-foreground", {
  variants: {
    orientation: {
      vertical: "cn-field-orientation-vertical flex-col *:w-full [&>.sr-only]:w-auto",
      horizontal: "cn-field-orientation-horizontal flex-row items-center has-[>[data-slot=field-content]]:items-start *:data-[slot=field-label]:flex-auto has-[>[data-slot=field-content]]:[&>[role=checkbox],&>[role=radio]]:mt-0.5",
      responsive: "cn-field-orientation-responsive flex-col *:w-full [&>.sr-only]:w-auto [@container_field-group_(min-width:_28rem)]:flex-row [@container_field-group_(min-width:_28rem)]:items-center [@container_field-group_(min-width:_28rem)]:*:w-auto [@container_field-group_(min-width:_28rem)]:has-[>[data-slot=field-content]]:items-start [@container_field-group_(min-width:_28rem)]:*:data-[slot=field-label]:flex-auto [@container_field-group_(min-width:_28rem)]:has-[>[data-slot=field-content]]:[&>[role=checkbox],&>[role=radio]]:mt-0.5",
    },
  },
  defaultVariants: {
    orientation: "vertical",
  },
});

function Field({
  className,
  orientation = "vertical",
  ...props
}: React.ComponentProps<typeof FieldPrimitive.Root> &
  VariantProps<typeof fieldVariants>) {
  return (
    <FieldPrimitive.Root
      data-slot="field"
      data-orientation={orientation}
      className={cn(fieldVariants({ orientation }), className)}
      {...props}
    />
  );
}

function FieldContent({
  className,
  ...props
}: React.ComponentProps<typeof wire.div>) {
  return (
    <wire.div
      data-slot="field-content"
      className={cn("cn-field-content flex flex-1 flex-col gap-1 leading-5", className)}
      {...props}
    />
  );
}

function FieldLabel({
  className,
  ...props
}: React.ComponentProps<typeof Label>) {
  return (
    <Label
      data-slot="field-label"
      className={cn("cn-field-label flex w-fit gap-2 leading-5 has-data-checked:bg-fill-subtle has-[>[data-slot=field]]:w-full has-[>[data-slot=field]]:flex-col has-[>[data-slot=field]]:rounded-2xl has-[>[data-slot=field]]:border has-[>[data-slot=field]]:not-has-[:disabled,[data-disabled]]:hover:bg-fill-subtle has-[>[data-slot=field]]:has-[:focus-visible]:border-ring has-[>[data-slot=field]]:has-[:focus-visible]:ring-3 has-[>[data-slot=field]]:has-[:focus-visible]:ring-ring/30 *:data-[slot=field]:p-3.75 [.cn-field[data-disabled=true]_&]:opacity-50", className)}
      {...props}
    />
  );
}

function FieldTitle({
  className,
  ...props
}: React.ComponentProps<typeof wire.div>) {
  return (
    <wire.div
      data-slot="field-label"
      className={cn("cn-field-title flex w-fit items-center gap-2 text-sm font-medium [.cn-field[data-disabled=true]_&]:opacity-50", className)}
      {...props}
    />
  );
}

function FieldDescription({
  className,
  ...props
}: React.ComponentProps<typeof FieldPrimitive.HelperText>) {
  return (
    <FieldPrimitive.HelperText
      data-slot="field-description"
      className={cn("cn-field-description text-start text-sm leading-5 font-normal text-muted-foreground [[data-variant=label]+&]:-mt-4 [[data-variant=legend]+&]:-mt-3.5 last:mt-0 nth-last-2:-mt-1 [&>a]:underline [&>a]:underline-offset-4 [&>a:hover]:text-primary [.cn-field[data-orientation=horizontal]_&]:text-balance [.cn-field:has([data-horizontal])_&]:text-balance", className)}
      {...props}
    />
  );
}

function FieldSeparator({
  children,
  className,
  ...props
}: React.ComponentProps<typeof wire.div> & {
  children?: React.ReactNode;
}) {
  return (
    <wire.div
      data-slot="field-separator"
      data-content={!!children}
      className={cn("cn-field-separator relative h-5 text-sm [.cn-field-group[data-variant=outline]_&]:-mb-2", className)}
      {...props}
    >
      <Separator className="cn-field-separator-line absolute inset-0 top-1/2" />
      {children && (
        <wire.span
          className="cn-field-separator-content relative mx-auto block w-fit bg-background px-2 text-muted-foreground empty:hidden"
          data-slot="field-separator-content"
        >
          {children}
        </wire.span>
      )}
    </wire.div>
  );
}

function FieldError({
  className,
  children,
  errors,
  ...props
}: React.ComponentProps<typeof FieldPrimitive.ErrorText> & {
  errors?: Array<{ message?: string } | undefined>;
}) {
  const content = useMemo(() => {
    if (children) {
      return children;
    }

    if (!errors?.length) {
      return null;
    }

    const uniqueErrors = [
      ...new Map(errors.map((error) => [error?.message, error])).values(),
    ];

    if (uniqueErrors?.length == 1) {
      return uniqueErrors[0]?.message;
    }

    return (
      <wire.ul className="cn-field-error-list ml-4 flex list-disc flex-col gap-1">
        {uniqueErrors.map(
          (error, index) =>
            error?.message && <wire.li key={index}>{error.message}</wire.li>
        )}
      </wire.ul>
    );
  }, [children, errors]);

  if (!content) {
    return null;
  }

  return (
    <FieldPrimitive.ErrorText
      data-slot="field-error"
      className={cn("cn-field-error text-sm leading-5 font-normal text-destructive-foreground [[data-variant=label]+&]:-mt-4 [[data-variant=legend]+&]:-mt-3.5", className)}
      {...props}
    >
      {content}
    </FieldPrimitive.ErrorText>
  );
}

function FieldRequiredIndicator({
  className,
  ...props
}: React.ComponentProps<typeof FieldPrimitive.RequiredIndicator>) {
  return (
    <FieldPrimitive.RequiredIndicator
      data-slot="field-required-indicator"
      className={cn("cn-field-required-indicator text-destructive", className)}
      {...props}
    />
  );
}

export {
  Field,
  FieldLabel,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLegend,
  FieldSeparator,
  FieldSet,
  FieldContent,
  FieldTitle,
  FieldRequiredIndicator,
};
