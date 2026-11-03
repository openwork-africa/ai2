"use client";

import { wire } from "@wirekyt/react";
import { Avatar as AvatarPrimitive } from "@wirekyt/react/avatar";
import { cn } from "cn";
import * as React from "react";

function Avatar({
  className,
  size = "default",
  shape = "round",
  ...props
}: React.ComponentProps<typeof AvatarPrimitive.Root> & {
  size?: "default" | "sm" | "lg" | "xs" | "xl";
  shape?: "round" | "square";
}) {
  return (
    <AvatarPrimitive.Root
      data-slot="avatar"
      data-size={size}
      data-shape={shape}
      className={cn(
        "cn-avatar relative flex size-8 shrink-0 select-none rounded-full data-[size=lg]:size-10 data-[size=sm]:size-6 data-[size=xl]:size-12 data-[size=xs]:size-5 data-[shape=square]:rounded-md data-[shape=square]:data-[size=lg]:rounded-lg data-[shape=square]:data-[size=sm]:rounded-sm data-[shape=square]:data-[size=xl]:rounded-lg data-[shape=square]:data-[size=xs]:rounded-sm",
        className
      )}
      {...props}
    />
  );
}

function AvatarImage({
  className,
  ...props
}: React.ComponentProps<typeof AvatarPrimitive.Image>) {
  return (
    <AvatarPrimitive.Image
      data-slot="avatar-image"
      className={cn(
        "cn-avatar-image aspect-square size-full rounded-[inherit] object-cover",
        className
      )}
      {...props}
    />
  );
}

function AvatarFallback({
  className,
  ...props
}: React.ComponentProps<typeof AvatarPrimitive.Fallback>) {
  return (
    <AvatarPrimitive.Fallback
      data-slot="avatar-fallback"
      className={cn(
        "cn-avatar-fallback flex size-full items-center justify-center bg-fill-default text-muted-foreground rounded-[inherit] text-sm [&>svg]:size-4 [.cn-avatar[data-size="sm"]_&]:text-2xs [.cn-avatar[data-size="xs"]_&]:text-2xs [.cn-avatar[data-size="lg"]_&_>_svg]:size-5 [.cn-avatar[data-size="xl"]_&_>_svg]:size-5 [.cn-avatar[data-size="xs"]_&_>_svg]:size-3",
        className
      )}
      {...props}
    />
  );
}

function AvatarBadge({
  className,
  ...props
}: React.ComponentProps<typeof wire.span>) {
  return (
    <wire.span
      data-slot="avatar-badge"
      className={cn(
        "cn-avatar-badge absolute right-0 bottom-0 z-10 inline-flex items-center justify-center select-none bg-primary text-primary-foreground ring-background rounded-full bg-blend-color ring-2 [.cn-avatar[data-size="sm"]_&]:size-2 [.cn-avatar[data-size="sm"]_&]:[&>svg]:hidden [.cn-avatar[data-size="default"]_&]:size-2.5 [.cn-avatar[data-size="default"]_&]:[&>svg]:size-2 [.cn-avatar[data-size="lg"]_&]:size-3 [.cn-avatar[data-size="lg"]_&]:[&>svg]:size-2 [.cn-avatar[data-size="xl"]_&]:size-3 [.cn-avatar[data-size="xl"]_&]:[&>svg]:size-2 [.cn-avatar[data-size="xs"]_&]:size-2 [.cn-avatar[data-size="xs"]_&]:ring-1 [.cn-avatar[data-size="xs"]_&]:[&>svg]:hidden",
        className
      )}
      {...props}
    />
  );
}

function AvatarGroup({
  className,
  ...props
}: React.ComponentProps<typeof wire.div>) {
  return (
    <wire.div
      data-slot="avatar-group"
      className={cn("cn-avatar-group flex -space-x-2 *:data-[slot=avatar]:ring-2 *:data-[slot=avatar]:ring-background", className)}
      {...props}
    />
  );
}

function AvatarGroupCount({
  className,
  ...props
}: React.ComponentProps<typeof wire.div>) {
  return (
    <wire.div
      data-slot="avatar-group-count"
      className={cn(
        "cn-avatar-group-count relative flex size-8 shrink-0 items-center justify-center bg-muted text-muted-foreground ring-background rounded-full text-sm ring-2 [&>svg]:size-4 [.cn-avatar-group:has([data-size="lg"])_&]:size-10 [.cn-avatar-group:has([data-size="lg"])_&]:[&>svg]:size-5 [.cn-avatar-group:has([data-size="sm"])_&]:size-6 [.cn-avatar-group:has([data-size="sm"])_&]:text-2xs [.cn-avatar-group:has([data-size="sm"])_&]:[&>svg]:size-3 [.cn-avatar-group:has([data-size="xl"])_&]:size-12 [.cn-avatar-group:has([data-size="xl"])_&]:[&>svg]:size-5 [.cn-avatar-group:has([data-size="xs"])_&]:size-5 [.cn-avatar-group:has([data-size="xs"])_&]:text-2xs [.cn-avatar-group:has([data-size="xs"])_&]:[&>svg]:size-3",
        className
      )}
      {...props}
    />
  );
}

const AvatarContext = AvatarPrimitive.Context;
const AvatarRootProvider = AvatarPrimitive.RootProvider;

export {
  Avatar,
  AvatarImage,
  AvatarFallback,
  AvatarGroup,
  AvatarGroupCount,
  AvatarBadge,
  AvatarContext,
  AvatarRootProvider,
};

export { useAvatar, useAvatarContext } from "@wirekyt/react/avatar";
