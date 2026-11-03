import { cn } from "cn";
import { Check, Inbox, X } from "lucide-react";
import { useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Item, ItemActions, ItemContent, ItemDescription, ItemGroup, ItemTitle } from "@/components/ui/item";
import { refreshApprovals, useApprovals, vault } from "@/hooks/use-vault";
import { type Approval, segment } from "@/lib/vault";
import { Chip, Failed, Mono, Nothing, Rows, run, Toolbar, When } from "@/pages/vault/common";

const WAIT_SECONDS = 120;

const left = (created: string, now: number) => {
  const start = new Date(created).getTime();
  if (Number.isNaN(start)) return null;
  return Math.max(0, WAIT_SECONDS - Math.floor((now - start) / 1000));
};

const clock = (seconds: number) => `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;

export default function Approvals() {
  const { approvals, error } = useApprovals();
  const [now, setNow] = useState(() => Date.now());
  const [deciding, setDeciding] = useState<string | null>(null);

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);

  const decide = async (approval: Approval, approve: boolean) => {
    setDeciding(approval.id);
    await run(
      () => vault("POST", `/approvals/${segment(approval.id)}`, { approve }, `${approve ? "allow" : "refuse"} this call`),
      { done: approve ? "Allowed the call" : "Refused the call", after: refreshApprovals },
    );
    setDeciding(null);
  };

  return (
    <div className="flex flex-col gap-4">
      <Toolbar>
        Write calls on agent tokens wait here for you. Undecided after two minutes, they are refused.
      </Toolbar>
      {error && !approvals ? (
        <Failed message={error} />
      ) : !approvals ? (
        <Rows count={2} />
      ) : approvals.length === 0 ? (
        <Nothing icon={Inbox} title="Nothing is waiting">
          When an agent asks to change something, the call shows up here. This page checks every few seconds.
        </Nothing>
      ) : (
        <ItemGroup className="gap-3">
          {approvals.map((approval) => {
            const remaining = left(approval.created, now);
            return (
              <Item key={approval.id} variant="outline" className="flex-col items-stretch gap-4 rounded-2xl bg-card p-4 sm:flex-row sm:items-start">
                <ItemContent className="w-full min-w-0 gap-2 sm:flex-1">
                  <ItemTitle className="flex w-full min-w-0 flex-wrap items-center gap-2">
                    <Badge variant="secondary" className="font-mono">
                      {approval.method}
                    </Badge>
                    <Mono className="min-w-0 text-sm break-all sm:truncate" title={approval.url}>
                      {approval.url}
                    </Mono>
                  </ItemTitle>
                  <ItemDescription className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs">
                    <span className="flex items-center gap-1">
                      key <Chip>{approval.key}</Chip>
                    </span>
                    <span className="flex items-center gap-1">
                      sandbox <Chip>{approval.sandbox}</Chip>
                    </span>
                    <span className="flex items-center gap-1">
                      token <Mono className="text-2xs">{approval.token}</Mono>
                    </span>
                    <span>
                      asked <When value={approval.created} />
                    </span>
                  </ItemDescription>
                  {approval.intent && (
                    <div className="flex flex-col gap-0.5 text-sm">
                      <span className="text-2xs font-medium tracking-wide text-muted-foreground uppercase">Intent</span>
                      <span>{approval.intent}</span>
                    </div>
                  )}
                  {approval.request && (
                    <div className="flex flex-col gap-0.5 rounded-lg bg-fill-subtle px-3 py-2 text-sm">
                      <span className="text-2xs font-medium tracking-wide text-muted-foreground uppercase">
                        The agent says, unverified
                      </span>
                      <span className="whitespace-pre-wrap break-words">{approval.request}</span>
                    </div>
                  )}
                </ItemContent>
                <ItemActions className="flex-row-reverse items-center justify-between gap-2 sm:flex-col sm:items-end">
                  {remaining !== null && (
                    <span
                      className={cn("font-mono text-2xs tabular-nums", remaining < 30 ? "text-warning" : "text-muted-foreground")}
                    >
                      refused in {clock(remaining)}
                    </span>
                  )}
                  <div className="flex gap-2">
                    <Button
                      variant="outline"
                      size="sm"
                      disabled={deciding === approval.id}
                      onClick={() => decide(approval, false)}
                    >
                      <X data-icon="inline-start" />
                      Deny
                    </Button>
                    <Button size="sm" disabled={deciding === approval.id} onClick={() => decide(approval, true)}>
                      <Check data-icon="inline-start" />
                      Approve
                    </Button>
                  </div>
                </ItemActions>
              </Item>
            );
          })}
        </ItemGroup>
      )}
    </div>
  );
}
