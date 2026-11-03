import { cn } from "cn";
import { Check, ChevronRight, Plug, Sparkles, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Code, Dot } from "@/components/page";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Spinner } from "@/components/ui/spinner";
import { addSkill, type Harness, listHarnesses } from "@/lib/harnesses";
import { type Connections, connectedNames, listConnections, makePairingCode, minutesLeft, pairLine, pasteLine, revokeToken, showCard } from "@/lib/mcp";
import { CopyButton } from "@/pages/vault/common";

const POLL = 5000;

function Http({ connections, onChange }: { connections: Connections; onChange: () => void }) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const origin = window.location.origin;
  const pairing = connections.pairing;

  const make = async () => {
    setBusy(true);
    setFailure(null);
    try {
      await makePairingCode();
      onChange();
    } catch (error) {
      setFailure((error as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const revoke = async (id: string) => {
    setFailure(null);
    try {
      await revokeToken(id);
      onChange();
    } catch (error) {
      setFailure((error as Error).message);
    }
  };

  return (
    <Collapsible open={open} onOpenChange={(details) => setOpen(details.open)}>
      <CollapsibleTrigger className="group flex items-center gap-1 rounded-md py-0.5 text-xs text-muted-foreground transition-colors hover:text-foreground">
        <ChevronRight className={cn("size-3.5 transition-transform duration-200", open && "rotate-90")} />
        Connect over HTTP
      </CollapsibleTrigger>
      <CollapsibleContent>
        <div className="mt-3 flex flex-col gap-3" data-pairing>
          {pairing ? (
            <>
              <div className="flex flex-wrap items-center gap-3">
                <Code className="px-2.5 py-1 font-mono text-base tracking-[0.2em]" data-code>
                  {pairing.code}
                </Code>
                <span className="text-xs text-muted-foreground">Good once, for {minutesLeft(pairing.expires, Date.now())} more minutes.</span>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <Code className="min-w-0 flex-1 overflow-x-auto py-1.5 whitespace-nowrap">{pairLine(origin, pairing.code)}</Code>
                <CopyButton value={pairLine(origin, pairing.code)} />
              </div>
            </>
          ) : (
            <div className="flex flex-wrap items-center gap-3">
              <span className="text-xs text-muted-foreground">A pairing code gives one HTTP client its own key.</span>
              <Button type="button" size="sm" variant="outline" onClick={make} disabled={busy}>
                {busy && <Spinner />}
                Make a pairing code
              </Button>
            </div>
          )}
          {connections.tokens.length > 0 && (
            <ul className="flex flex-col divide-y">
              {connections.tokens.map((token) => (
                <li key={token.id} className="flex items-center gap-3 py-2 text-xs">
                  <span className="font-medium">{token.name}</span>
                  <span className="text-muted-foreground">
                    {token.last_used ? `last used ${new Date(token.last_used * 1000).toLocaleString()}` : "not used yet"}
                  </span>
                  <Button type="button" size="xs" variant="ghost" className="ml-auto" onClick={() => void revoke(token.id)} aria-label={`Revoke ${token.name}`}>
                    <X data-icon="inline-start" />
                    Revoke
                  </Button>
                </li>
              ))}
            </ul>
          )}
          {failure && <p className="text-xs text-destructive">{failure}</p>}
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}

export function SkillButton({ harness, onAdded }: { harness: Harness; onAdded: () => void }) {
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  if (harness.skill) {
    return (
      <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
        <Check className="size-3.5" />
        The ai2 skill is in {harness.label}.
      </span>
    );
  }
  const add = async () => {
    setBusy(true);
    setFailure(null);
    try {
      await addSkill(harness.id);
      onAdded();
    } catch (error) {
      setFailure((error as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <span className="flex flex-wrap items-center gap-2">
      <Button type="button" size="sm" variant="outline" onClick={add} disabled={busy}>
        {busy ? <Spinner /> : <Sparkles />}
        Add the ai2 skill to {harness.label}
      </Button>
      {failure && <span className="text-xs text-destructive">{failure}</span>}
    </span>
  );
}

function useConnections() {
  const [connections, setConnections] = useState<Connections | null>(null);

  const load = useCallback(() => {
    listConnections()
      .then(setConnections)
      .catch(() => setConnections(null));
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const waiting = showCard(connections);
  useEffect(() => {
    if (!waiting) return;
    const timer = window.setInterval(load, POLL);
    return () => window.clearInterval(timer);
  }, [waiting, load]);

  return { connections, waiting, load };
}

export function McpConnect() {
  const { connections, waiting, load } = useConnections();
  const [claude, setClaude] = useState<Harness | null>(null);

  const loadHarness = useCallback(() => {
    listHarnesses()
      .then(({ harnesses }) => setClaude(harnesses.find((harness) => harness.id === "claude" && harness.installed && harness.skills) ?? null))
      .catch(() => setClaude(null));
  }, []);

  useEffect(() => {
    loadHarness();
  }, [loadHarness]);

  if (!connections) return null;

  if (!waiting) {
    return (
      <Card size="sm" className="gap-4" data-mcp="connected">
        <p className="flex items-center gap-2 px-(--card-spacing) text-xs text-muted-foreground">
          <Dot tone="success" className="ring-0" />
          Connected: {connectedNames(connections).join(", ")}
        </p>
        <div className="border-t px-(--card-spacing) pt-3">
          <Http connections={connections} onChange={load} />
        </div>
      </Card>
    );
  }

  const line = pasteLine(window.location.origin);
  return (
    <Card size="sm" className="gap-4" data-mcp="card">
      <div className="flex items-center gap-3 px-(--card-spacing)">
        <span className="flex size-9 shrink-0 items-center justify-center rounded-xl bg-fill-default">
          <Plug className="size-4" />
        </span>
        <p className="text-sm">Lets your coding agent work with ai squared: your apps, keys and reviews.</p>
      </div>
      <div className="flex flex-wrap items-center gap-2 px-(--card-spacing)">
        <Code className="min-w-0 flex-1 overflow-x-auto py-1.5 whitespace-nowrap" data-paste>
          {line}
        </Code>
        <CopyButton value={line} />
      </div>
      <div className="flex flex-col gap-3 border-t px-(--card-spacing) pt-3">
        {claude && <SkillButton harness={claude} onAdded={loadHarness} />}
        <Http connections={connections} onChange={load} />
      </div>
    </Card>
  );
}
