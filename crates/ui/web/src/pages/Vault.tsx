import { KeyRound } from "lucide-react";
import { Code, Dot, Page, PageHeader } from "@/components/page";
import { Badge } from "@/components/ui/badge";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { useStatus } from "@/hooks/use-status";

export default function Vault() {
  const { status } = useStatus();
  const vault = status?.vault;
  return (
    <Page>
      <PageHeader
        title="Vault"
        description="Keys, tokens, sandboxes, approvals and the audit log. Secrets stay in the vault service; this page never sees them."
        actions={
          vault && (
            <Badge variant="outline" size="lg">
              <Dot tone={vault.reachable ? "success" : "warning"} className="ring-0" />
              {vault.reachable ? `daemon v${vault.version ?? "?"}` : "daemon not running"}
              {vault.reachable && vault.master_key !== null && ` · ${vault.master_key ? "key loaded" : "no key"}`}
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
            <EmptyTitle>The vault is not running</EmptyTitle>
            <EmptyDescription>Setup installs it and starts it with you.</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Code>ai2 setup</Code>
          </EmptyContent>
        </Empty>
      )}
    </Page>
  );
}
