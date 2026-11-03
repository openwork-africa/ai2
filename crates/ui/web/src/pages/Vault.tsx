import { Box, Inbox, KeyRound, ScrollText, Ticket } from "lucide-react";
import { useSearchParams } from "react-router";
import { Code, Dot, Page, PageHeader } from "@/components/page";
import { Badge } from "@/components/ui/badge";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Tabs, TabsContent, TabsList, TabsTrigger, TabsTriggerCount } from "@/components/ui/tabs";
import { Toaster } from "@/components/ui/toast";
import { useStatus } from "@/hooks/use-status";
import { useApprovals, useVaultStatus } from "@/hooks/use-vault";
import { isMac } from "@/lib/vault";
import Approvals from "@/pages/vault/Approvals";
import Audit from "@/pages/vault/Audit";
import Keys from "@/pages/vault/Keys";
import { Pending } from "@/pages/vault/Pending";
import { Presence } from "@/pages/vault/Presence";
import Sandboxes from "@/pages/vault/Sandboxes";
import Tokens from "@/pages/vault/Tokens";

const TABS = [
  { value: "keys", label: "Keys", icon: KeyRound },
  { value: "tokens", label: "Tokens", icon: Ticket },
  { value: "sandboxes", label: "Sandboxes", icon: Box },
  { value: "approvals", label: "Approvals", icon: Inbox },
  { value: "audit", label: "Audit", icon: ScrollText },
] as const;

type Tab = (typeof TABS)[number]["value"];

const isTab = (value: string | null): value is Tab => TABS.some((tab) => tab.value === value);

export default function Vault() {
  const { status } = useStatus();
  const vault = status?.vault;
  return (
    <Page>
      <PageHeader
        title="Vault"
        description="Your keys stay locked away here, and agents get only a small pass to use them."
        actions={
          vault && (
            <Badge variant="outline" size="lg">
              <Dot tone={vault.reachable ? "success" : "warning"} className="ring-0" />
              {vault.reachable ? `vault v${vault.version ?? "?"}` : "vault down"}
              {vault.reachable && vault.master_key !== null && ` · ${vault.master_key ? "master key loaded" : "no master key"}`}
            </Badge>
          )
        }
      />
      {vault && !vault.reachable && (
        <Empty className="rounded-3xl border border-dashed py-16">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <KeyRound />
            </EmptyMedia>
            <EmptyTitle>The vault is down</EmptyTitle>
            <EmptyDescription>The vault keeps your keys safe, and setup starts it each time you log in.</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Code>ai2 setup</Code>
          </EmptyContent>
        </Empty>
      )}
      {vault?.reachable && <Live />}
    </Page>
  );
}

function Live() {
  const [params, setParams] = useSearchParams();
  const asked = params.get("tab");
  const tab: Tab = isTab(asked) ? asked : "keys";
  const { data } = useVaultStatus();
  const mac = isMac(data?.platform);
  const waiting = useApprovals().approvals?.length ?? 0;

  return (
    <>
      <Tabs
        value={tab}
        onValueChange={(details) =>
          setParams(
            (prev) => {
              const next = new URLSearchParams(prev);
              next.set("tab", details.value);
              return next;
            },
            { replace: true },
          )
        }
        className="gap-6"
      >
        <div className="-mx-5 overflow-x-auto px-5 md:mx-0 md:px-0">
          <TabsList>
            {TABS.map((entry) => (
              <TabsTrigger key={entry.value} value={entry.value} className="gap-1.5 px-3">
                <entry.icon />
                {entry.label}
                {entry.value === "approvals" && waiting > 0 && (
                  <TabsTriggerCount className="rounded-full bg-primary px-1.5 text-2xs text-primary-foreground">
                    {waiting}
                  </TabsTriggerCount>
                )}
              </TabsTrigger>
            ))}
          </TabsList>
        </div>
        <TabsContent value="keys">{tab === "keys" && <Keys />}</TabsContent>
        <TabsContent value="tokens">{tab === "tokens" && <Tokens mac={mac} />}</TabsContent>
        <TabsContent value="sandboxes">{tab === "sandboxes" && <Sandboxes />}</TabsContent>
        <TabsContent value="approvals">{tab === "approvals" && <Approvals />}</TabsContent>
        <TabsContent value="audit">{tab === "audit" && <Audit />}</TabsContent>
      </Tabs>
      <Pending />
      <Presence />
      <Toaster />
    </>
  );
}
