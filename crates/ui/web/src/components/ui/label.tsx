"use client";

import { Field } from "@wirekyt/react/field";
import { cn } from "cn";
import type * as React from "react";

const Label = ({
  className,
  ...props
}: React.ComponentProps<typeof Field.Label>) => (
  <Field.Label
    data-slot="label"
    className={cn("gap-2 text-sm leading-none font-medium group-data-[disabled=true]:pointer-events-none group-data-[disabled=true]:opacity-50 group-data-disabled/field:pointer-events-none group-data-disabled/field:opacity-50 peer-disabled:cursor-not-allowed peer-disabled:opacity-50 peer-data-disabled:cursor-not-allowed peer-data-disabled:opacity-50 flex items-center select-none", className)}
    {...props}
  />
);

export { Label };
