"use client";

import { Portal, wire } from "@wirekyt/react";
import { Dialog as AlertDialogPrimitive } from "@wirekyt/react/dialog";
import { cn } from "cn";
import * as React from "react";

import { Button } from "@/components/ui/button";

const AlertDialog = ({
  ...props
}: React.ComponentProps<typeof AlertDialogPrimitive.Root>) => (
  <AlertDialogPrimitive.Root
    data-slot="alert-dialog"
    role="alertdialog"
    {...props}
  />
);

const AlertDialogTrigger = ({
  ...props
}: React.ComponentProps<typeof AlertDialogPrimitive.Trigger>) => (
  <AlertDialogPrimitive.Trigger data-slot="alert-dialog-trigger" {...props} />
);

const AlertDialogOverlay = ({
  className,
  ...props
}: React.ComponentProps<typeof AlertDialogPrimitive.Backdrop>) => (
  <AlertDialogPrimitive.Backdrop
    data-slot="alert-dialog-overlay"
    className={cn(
      "cn-alert-dialog-overlay data-open:animate-in data-closed:animate-out data-closed:fade-out-0 data-open:fade-in-0 bg-black/30 duration-100 supports-backdrop-filter:backdrop-blur-sm fixed inset-0 isolate z-50",
      className
    )}
    {...props}
  />
);

const AlertDialogContent = ({
  className,
  size = "default",
  ...props
}: React.ComponentProps<typeof AlertDialogPrimitive.Content> & {
  size?: "default" | "sm";
}) => (
  <Portal>
    <AlertDialogOverlay />
    <AlertDialogPrimitive.Positioner className="cn-alert-dialog-positioner fixed inset-0 z-50 flex items-center justify-center">
      <AlertDialogPrimitive.Content
        data-slot="alert-dialog-content"
        data-size={size}
        role="alertdialog"
        className={cn(
          "cn-alert-dialog-content data-open:animate-in data-closed:animate-out data-closed:fade-out-0 data-open:fade-in-0 data-closed:zoom-out-95 data-open:zoom-in-95 bg-popover text-popover-foreground z-50 grid w-full gap-6 rounded-4xl p-6 shadow-(--shadow-dialog) ring-1 ring-border duration-100 outline-none data-[size=default]:max-w-xs data-[size=sm]:max-w-xs data-[size=default]:sm:max-w-md",
          className
        )}
        {...props}
      />
    </AlertDialogPrimitive.Positioner>
  </Portal>
);

const AlertDialogHeader = ({
  className,
  ...props
}: React.ComponentProps<typeof wire.div>) => (
  <wire.div
    data-slot="alert-dialog-header"
    className={cn("cn-alert-dialog-header grid grid-rows-[auto_1fr] place-items-center gap-1.5 text-center has-data-[slot=alert-dialog-media]:grid-rows-[auto_auto_1fr] has-data-[slot=alert-dialog-media]:gap-x-6 [@media(min-width:_40rem)]:[.cn-alert-dialog-content[data-size="default"]_&]:place-items-start [@media(min-width:_40rem)]:[.cn-alert-dialog-content[data-size="default"]_&]:text-left [@media(min-width:_40rem)]:[.cn-alert-dialog-content[data-size="default"]_&:has([data-slot="alert-dialog-media"])]:grid-rows-[auto_1fr]", className)}
    {...props}
  />
);

const AlertDialogFooter = ({
  className,
  ...props
}: React.ComponentProps<typeof wire.div>) => (
  <wire.div
    data-slot="alert-dialog-footer"
    className={cn("cn-alert-dialog-footer flex flex-col-reverse gap-2 sm:flex-row sm:justify-end [.cn-alert-dialog-content[data-size="sm"]_&]:grid [.cn-alert-dialog-content[data-size="sm"]_&]:grid-cols-2", className)}
    {...props}
  />
);

const AlertDialogMedia = ({
  className,
  ...props
}: React.ComponentProps<typeof wire.div>) => (
  <wire.div
    data-slot="alert-dialog-media"
    className={cn("cn-alert-dialog-media mb-2.5 inline-flex size-12 items-center justify-center rounded-lg bg-fill-default *:[svg:not([class*='size-'])]:size-5 [@media(min-width:_40rem)]:[.cn-alert-dialog-content[data-size="default"]_&]:row-span-2 [@media(min-width:_40rem)]:[.cn-alert-dialog-content[data-size="default"]_&]:mb-0", className)}
    {...props}
  />
);

const AlertDialogTitle = ({
  className,
  ...props
}: React.ComponentProps<typeof AlertDialogPrimitive.Title>) => (
  <AlertDialogPrimitive.Title
    data-slot="alert-dialog-title"
    className={cn("cn-alert-dialog-title font-heading text-base font-medium [@media(min-width:_40rem)]:[.cn-alert-dialog-content[data-size="default"]:has([data-slot="alert-dialog-media"])_&]:col-start-2", className)}
    {...props}
  />
);

const AlertDialogDescription = ({
  className,
  ...props
}: React.ComponentProps<typeof AlertDialogPrimitive.Description>) => (
  <AlertDialogPrimitive.Description
    data-slot="alert-dialog-description"
    className={cn("cn-alert-dialog-description text-sm text-balance text-muted-foreground md:text-pretty *:[a]:underline *:[a]:underline-offset-3 *:[a]:hover:text-foreground", className)}
    {...props}
  />
);

const AlertDialogAction = ({
  className,
  variant = "default",
  size = "default",
  ...props
}: React.ComponentProps<typeof AlertDialogPrimitive.CloseTrigger> &
  Pick<React.ComponentProps<typeof Button>, "variant" | "size">) => (
  <Button variant={variant} size={size} asChild>
    <AlertDialogPrimitive.CloseTrigger
      data-slot="alert-dialog-action"
      className={cn("cn-alert-dialog-action", className)}
      {...props}
    />
  </Button>
);

const AlertDialogCancel = ({
  className,
  variant = "outline",
  size = "default",
  ...props
}: React.ComponentProps<typeof AlertDialogPrimitive.CloseTrigger> &
  Pick<React.ComponentProps<typeof Button>, "variant" | "size">) => (
  <Button variant={variant} size={size} asChild>
    <AlertDialogPrimitive.CloseTrigger
      data-slot="alert-dialog-cancel"
      className={cn("cn-alert-dialog-cancel", className)}
      {...props}
    />
  </Button>
);

const AlertDialogContext = AlertDialogPrimitive.Context;
const AlertDialogRootProvider = AlertDialogPrimitive.RootProvider;

export {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogContext,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogMedia,
  AlertDialogOverlay,
  AlertDialogRootProvider,
  AlertDialogTitle,
  AlertDialogTrigger,
};

export {
  useDialog as useAlertDialog,
  useDialogContext as useAlertDialogContext,
  type DialogOpenChangeDetails as AlertDialogOpenChangeDetails,
} from "@wirekyt/react/dialog";
