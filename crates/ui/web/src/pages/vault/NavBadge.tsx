import { SidebarMenuBadge } from "@/components/ui/sidebar";
import { useApprovals } from "@/hooks/use-vault";

function Count() {
  const count = useApprovals().approvals?.length ?? 0;
  if (!count) return null;
  return (
    <SidebarMenuBadge
      aria-label={`${count} waiting for your word`}
      className="rounded-full bg-primary px-1.5 text-primary-foreground [.cn-sidebar-menu-button:hover_~_&]:text-primary-foreground [.cn-sidebar-menu-button[data-active=true]_~_&]:text-primary-foreground"
    >
      {count}
    </SidebarMenuBadge>
  );
}

export function VaultNavBadge({ reachable }: { reachable: boolean }) {
  return reachable ? <Count /> : null;
}
