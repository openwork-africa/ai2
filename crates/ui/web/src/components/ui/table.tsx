"use client";

import { wire } from "@wirekyt/react";
import { cn } from "cn";
import * as React from "react";

function Table({
  className,
  ...props
}: React.ComponentProps<typeof wire.table>) {
  return (
    <wire.div data-slot="table-container" className="cn-table-container relative w-full overflow-x-auto">
      <wire.table
        data-slot="table"
        className={cn("cn-table w-full caption-bottom text-sm", className)}
        {...props}
      />
    </wire.div>
  );
}

function TableHeader({
  className,
  ...props
}: React.ComponentProps<typeof wire.thead>) {
  return (
    <wire.thead
      data-slot="table-header"
      className={cn("cn-table-header [&_tr]:border-b", className)}
      {...props}
    />
  );
}

function TableBody({
  className,
  ...props
}: React.ComponentProps<typeof wire.tbody>) {
  return (
    <wire.tbody
      data-slot="table-body"
      className={cn("cn-table-body [&_tr:last-child]:border-0", className)}
      {...props}
    />
  );
}

function TableFooter({
  className,
  ...props
}: React.ComponentProps<typeof wire.tfoot>) {
  return (
    <wire.tfoot
      data-slot="table-footer"
      className={cn("cn-table-footer border-t bg-fill-default font-medium [&_>_tr:last-child]:border-b-0", className)}
      {...props}
    />
  );
}

function TableRow({
  className,
  ...props
}: React.ComponentProps<typeof wire.tr>) {
  return (
    <wire.tr
      data-slot="table-row"
      className={cn("cn-table-row border-b transition-colors hover:bg-fill-default data-[state=selected]:bg-fill-hover [&:has([aria-expanded=true])]:bg-fill-default", className)}
      {...props}
    />
  );
}

function TableHead({
  className,
  ...props
}: React.ComponentProps<typeof wire.th>) {
  return (
    <wire.th
      data-slot="table-head"
      className={cn("cn-table-head h-[calc(--spacing(10)-var(--table-divider,0px))] px-3 pt-2.5 pb-[calc(--spacing(2.5)-var(--table-divider,0px))] text-left align-middle font-medium whitespace-nowrap text-foreground [&:has([role=checkbox])]:pr-0", className)}
      {...props}
    />
  );
}

function TableCell({
  className,
  ...props
}: React.ComponentProps<typeof wire.td>) {
  return (
    <wire.td
      data-slot="table-cell"
      className={cn("cn-table-cell px-3 pt-[calc(--spacing(2)-var(--table-divider-top,0px))] pb-[calc(--spacing(2)-var(--table-divider,0px))] align-middle whitespace-nowrap [&:has([role=checkbox])]:pr-0", className)}
      {...props}
    />
  );
}

function TableCaption({
  className,
  ...props
}: React.ComponentProps<typeof wire.caption>) {
  return (
    <wire.caption
      data-slot="table-caption"
      className={cn("cn-table-caption mt-4 px-3 text-left text-sm text-muted-foreground", className)}
      {...props}
    />
  );
}

export {
  Table,
  TableHeader,
  TableBody,
  TableFooter,
  TableHead,
  TableRow,
  TableCell,
  TableCaption,
};
