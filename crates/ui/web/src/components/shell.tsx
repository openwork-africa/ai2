import { Link, Outlet, useLocation } from "react-router";
import { Mark } from "@/components/mark";
import { Dot } from "@/components/page";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarTrigger,
} from "@/components/ui/sidebar";
import { StatusContext, usePolledStatus } from "@/hooks/use-status";
import { NAV } from "@/nav";

const machine = () => (/Mac/i.test(navigator.userAgent) ? "this Mac" : "this machine");

export function Shell() {
  const { pathname } = useLocation();
  const polled = usePolledStatus();
  const { status, error } = polled;
  const current = NAV.find((entry) => entry.match(pathname));
  const vault = status?.vault;
  const tone = !status ? (error ? "destructive" : "muted") : vault?.reachable ? "success" : "warning";
  const vaultLine = !status
    ? error
      ? "ai squared isn't answering"
      : "Checking on the vault"
    : vault?.reachable
      ? vault.master_key === false
        ? "Vault up, no master key"
        : "Vault up"
      : "Vault down";

  return (
    <SidebarProvider>
      <Sidebar variant="inset" className="border-r-0">
        <SidebarHeader className="border-b-0 px-3 pt-4 pb-2">
          <Link to="/" className="flex items-center gap-2.5 rounded-lg px-1 py-1">
            <Mark />
            <span className="flex min-w-0 flex-col leading-tight">
              <span className="font-heading text-sm font-semibold tracking-tight">ai squared</span>
              <span className="truncate text-2xs text-muted-foreground">on {machine()}</span>
            </span>
          </Link>
        </SidebarHeader>
        <SidebarContent>
          <SidebarGroup className="px-2">
            <SidebarGroupContent>
              <SidebarMenu className="gap-0.5">
                {NAV.map((entry) => (
                  <SidebarMenuItem key={entry.to}>
                    <SidebarMenuButton asChild isActive={entry === current}>
                      <Link to={entry.to}>
                        <entry.icon />
                        <span>{entry.label}</span>
                      </Link>
                    </SidebarMenuButton>
                    {entry.badge && <entry.badge reachable={Boolean(vault?.reachable)} />}
                  </SidebarMenuItem>
                ))}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </SidebarContent>
        <SidebarFooter className="border-t-0 px-3 pb-4">
          <Link
            to="/vault"
            className="flex items-center gap-2 rounded-lg px-2 py-1.5 text-xs text-muted-foreground transition-colors hover:bg-fill-default hover:text-foreground"
          >
            <Dot tone={tone} />
            <span className="truncate">{vaultLine}</span>
          </Link>
          <span className="px-2 font-mono text-2xs text-muted-foreground/70">
            {status ? `v${status.version}` : " "}
          </span>
        </SidebarFooter>
      </Sidebar>
      <SidebarInset className="min-w-0">
        <div className="sticky top-0 z-10 flex h-12 items-center gap-2 border-b bg-background/80 px-3 backdrop-blur md:hidden">
          <SidebarTrigger />
          <Mark className="size-6" />
          <span className="text-sm font-medium">{current?.label ?? "ai2"}</span>
        </div>
        <StatusContext.Provider value={polled}>
          <Outlet />
        </StatusContext.Provider>
      </SidebarInset>
    </SidebarProvider>
  );
}
