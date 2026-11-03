import { Box, Check, KeyRound, Layers, type LucideIcon, RefreshCcw, Ticket, Trash2, TriangleAlert, X } from "lucide-react";
import { useState } from "react";
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogMedia,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { applyQueue, dequeue, discardQueue, useQueue } from "@/hooks/use-vault";
import { type Change, describeChange, type NewToken } from "@/lib/vault";
import { CopyButton, Mono, run, When } from "@/pages/vault/common";

const ICONS: Record<Change["action"], LucideIcon> = {
  put_key: KeyRound,
  rotate_key: RefreshCcw,
  delete_key: Trash2,
  create_token: Ticket,
  revoke_token: Trash2,
  add_sandbox: Box,
  remove_sandbox: Trash2,
};

const removes = (change: Change) =>
  change.action === "delete_key" || change.action === "revoke_token" || change.action === "remove_sandbox";

export function Pending() {
  const queue = useQueue();
  const [busy, setBusy] = useState(false);
  const [made, setMade] = useState<NewToken[]>([]);
  const count = queue.length;

  const apply = async () => {
    setBusy(true);
    const tokens = await run(applyQueue, { done: count === 1 ? "Applied 1 change" : `Applied ${count} changes` });
    setBusy(false);
    if (tokens?.length) setMade(tokens);
  };

  return (
    <>
      {count > 0 && (
        <section
          aria-label="Pending changes"
          className="sticky bottom-4 z-30 overflow-hidden rounded-2xl border bg-popover text-popover-foreground shadow-(--shadow-dialog) animate-in fade-in-0 slide-in-from-bottom-2"
        >
          <ul className="max-h-56 divide-y overflow-y-auto">
            {queue.map((entry) => {
              const Icon = ICONS[entry.change.action];
              return (
                <li key={entry.id} className="flex items-center gap-3 px-4 py-2 text-xs">
                  <Icon className={removes(entry.change) ? "size-3.5 shrink-0 text-destructive" : "size-3.5 shrink-0 text-muted-foreground"} />
                  <span className="min-w-0 flex-1 truncate">{describeChange(entry.change)}</span>
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    aria-label="Drop this change"
                    disabled={busy}
                    onClick={() => dequeue(entry.id)}
                  >
                    <X />
                  </Button>
                </li>
              );
            })}
          </ul>
          <div className="flex flex-wrap items-center gap-3 border-t bg-fill-subtle px-4 py-3">
            <span className="flex size-7 items-center justify-center rounded-lg bg-primary text-primary-foreground">
              <Layers className="size-3.5" />
            </span>
            <span className="flex min-w-0 flex-1 flex-col leading-tight">
              <span className="text-sm font-medium">
                {count === 1 ? "1 change waiting" : `${count} changes waiting`}
              </span>
              <span className="hidden text-xs text-muted-foreground sm:inline">Checked together and applied all or none, after one confirmation.</span>
            </span>
            <div className="flex gap-2">
              <Button variant="outline" disabled={busy} onClick={discardQueue}>
                Discard
              </Button>
              <Button disabled={busy} onClick={apply}>
                {busy ? <Spinner data-icon="inline-start" /> : <Check data-icon="inline-start" />}
                Apply
              </Button>
            </div>
          </div>
        </section>
      )}
      <ShownOnce tokens={made} onDone={() => setMade([])} />
    </>
  );
}

function ShownOnce({ tokens, onDone }: { tokens: NewToken[]; onDone: () => void }) {
  const one = tokens.length === 1;
  return (
    <AlertDialog open={tokens.length > 0} onOpenChange={(details) => !details.open && onDone()} closeOnInteractOutside={false}>
      <AlertDialogContent className="data-[size=default]:sm:max-w-lg">
        <AlertDialogHeader>
          <AlertDialogMedia className="bg-success/10 text-success">
            <Ticket />
          </AlertDialogMedia>
          <AlertDialogTitle>{one ? "Copy the new token now" : "Copy the new tokens now"}</AlertDialogTitle>
          <AlertDialogDescription>
            This is the only time {one ? "it appears" : "they appear"}. The vault keeps only a hash, so it can't show {one ? "it" : "them"} again.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <div className="flex max-h-[50vh] min-w-0 flex-col gap-5 overflow-y-auto">
          {tokens.map((token) => (
            <div key={token.id} className="flex min-w-0 flex-col gap-2">
              <div className="flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground">
                <Mono>{token.id}</Mono>
                <span>
                  {token.mode} · {token.sandbox} · expires <When value={token.expires} />
                </span>
              </div>
              <div className="flex gap-2">
                <Input
                  readOnly
                  value={token.token}
                  aria-label={`Token ${token.id}`}
                  spellCheck={false}
                  className="font-mono"
                  onFocus={(event) => event.currentTarget.select()}
                />
                <CopyButton value={token.token} />
              </div>
              {token.warnings?.map((warning) => (
                <p key={warning} className="flex gap-2 rounded-lg bg-warning/10 px-3 py-2 text-xs">
                  <TriangleAlert className="mt-0.5 size-3.5 shrink-0 text-warning" />
                  {warning}
                </p>
              ))}
            </div>
          ))}
        </div>
        <AlertDialogFooter>
          <Button onClick={onDone}>Done</Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
