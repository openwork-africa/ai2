import { Fingerprint, LockKeyhole } from "lucide-react";
import { type FormEvent, useState } from "react";
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogMedia,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { answerPassphrase, usePresence } from "@/hooks/use-vault";

export function Presence() {
  const { kind, waiting, action, ask } = usePresence();
  return (
    <>
      {waiting > 0 && (
        <div
          role="status"
          aria-live="polite"
          className="pointer-events-none fixed inset-x-0 bottom-6 z-[60] flex justify-center px-4 animate-in fade-in-0 slide-in-from-bottom-4"
        >
          <div className="flex items-center gap-3 rounded-2xl border bg-popover py-2.5 pr-5 pl-3 text-popover-foreground shadow-(--shadow-dialog)">
            <span className="relative flex size-9 items-center justify-center rounded-xl bg-fill-default">
              <span className="absolute inset-0 animate-ping rounded-xl bg-primary/10" />
              <Fingerprint className="size-5" />
            </span>
            <span className="flex flex-col leading-tight">
              <span className="text-sm font-medium">
                {kind === "polkit" ? "Confirm in the system prompt" : "Confirm with Touch ID on this Mac"}
              </span>
              <span className="text-xs text-muted-foreground">The vault asks before it will {action}.</span>
            </span>
          </div>
        </div>
      )}
      {ask && <Passphrase key={ask.action} action={ask.action} />}
    </>
  );
}

function Passphrase({ action }: { action: string }) {
  const [value, setValue] = useState("");
  const finish = (answer: string | null) => {
    setValue("");
    answerPassphrase(answer);
  };
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (value) finish(value);
  };
  return (
    <AlertDialog open onOpenChange={(details) => !details.open && finish(null)}>
      <AlertDialogContent>
        <form onSubmit={submit} autoComplete="off" className="grid gap-6">
          <AlertDialogHeader>
            <AlertDialogMedia>
              <LockKeyhole />
            </AlertDialogMedia>
            <AlertDialogTitle>Vault passphrase</AlertDialogTitle>
            <AlertDialogDescription>
              The vault needs you present to {action}. The passphrase is sent once, for this change only, and not kept.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <Field>
            <FieldLabel>Passphrase</FieldLabel>
            <Input
              type="password"
              value={value}
              onChange={(event) => setValue(event.target.value)}
              autoComplete="current-password"
              spellCheck={false}
              autoFocus
            />
          </Field>
          <AlertDialogFooter>
            <AlertDialogCancel type="button">Cancel</AlertDialogCancel>
            <Button type="submit" disabled={!value}>
              Confirm
            </Button>
          </AlertDialogFooter>
        </form>
      </AlertDialogContent>
    </AlertDialog>
  );
}
