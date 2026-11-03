import { cn } from "cn";
import { KeyRound, ListPlus, Plus, RefreshCcw, Sparkles, Trash2 } from "lucide-react";
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
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Sheet, SheetContent, SheetDescription, SheetFooter, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Spinner } from "@/components/ui/spinner";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Textarea } from "@/components/ui/textarea";
import { enqueue, useVaultQuery, vault } from "@/hooks/use-vault";
import {
  INJECT_PRESETS,
  type InjectPreset,
  injectLabel,
  injectValue,
  lines,
  segment,
  validName,
  type VaultKey,
} from "@/lib/vault";
import { Failed, Mono, Nothing, Panel, problem, QueuedTag, Rows, Toolbar, Undo, Urls, useQueued, When } from "@/pages/vault/common";

type KeyList = { keys: VaultKey[] };

export default function Keys() {
  const { data, error } = useVaultQuery<KeyList>("/keys");
  const queued = useQueued();
  const [adding, setAdding] = useState(false);
  const [rotating, setRotating] = useState<VaultKey | null>(null);
  const keys = data?.keys ?? [];

  return (
    <div className="flex flex-col gap-4">
      <Toolbar
        actions={
          <Button onClick={() => setAdding(true)}>
            <Plus data-icon="inline-start" />
            Add key
          </Button>
        }
      >
        Each key is locked in the vault and goes only where you said it may.
      </Toolbar>
      {error && !data ? (
        <Failed message={error} />
      ) : !data ? (
        <Rows />
      ) : keys.length === 0 ? (
        <Nothing icon={KeyRound} title="The vault is empty">
          Keys you add are kept safe by the vault and used for your apps and agents.
        </Nothing>
      ) : (
        <Panel>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="pl-4">Key</TableHead>
                <TableHead>Approved URLs</TableHead>
                <TableHead>Inject</TableHead>
                <TableHead>Changed</TableHead>
                <TableHead className="pr-4 text-right">
                  <span className="sr-only">Actions</span>
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {keys.map((key) => {
                const deleting = queued("delete_key", key.name);
                const rotation = queued("rotate_key", key.name);
                return (
                  <TableRow key={key.name} className={cn(deleting && "bg-destructive/5 hover:bg-destructive/5")}>
                    <TableCell className={cn("max-w-72 min-w-44 py-3 pl-4 align-top", deleting && "opacity-50")}>
                      <div className="flex flex-col gap-0.5">
                        <Mono className={cn("text-sm font-medium", deleting && "line-through")}>{key.name}</Mono>
                        {key.description && (
                          <span className="line-clamp-2 text-xs whitespace-normal text-muted-foreground">{key.description}</span>
                        )}
                      </div>
                    </TableCell>
                    <TableCell className={cn("py-3 align-top", deleting && "opacity-50")}>
                      <Urls urls={key.approved_urls} />
                    </TableCell>
                    <TableCell className={cn("py-3 align-top", deleting && "opacity-50")}>
                      <Badge variant="outline" className="font-mono" title={key.inject}>
                        {injectLabel(key.inject)}
                      </Badge>
                    </TableCell>
                    <TableCell className="py-3 align-top text-xs text-muted-foreground">
                      {rotation ? (
                        <QueuedTag>new key queued</QueuedTag>
                      ) : (
                        <span className="flex flex-col">
                          <When value={key.rotated ?? key.created} />
                          <span className="text-2xs text-muted-foreground/80">{key.rotated ? "rotated" : "added"}</span>
                        </span>
                      )}
                    </TableCell>
                    <TableCell className="py-3 pr-4 text-right align-top">
                      {deleting ? (
                        <Undo entry={deleting} label="Queued to delete" />
                      ) : rotation ? (
                        <Undo entry={rotation} label="Rotation queued" />
                      ) : (
                        <div className="flex justify-end gap-1">
                          <Button variant="outline" size="sm" onClick={() => setRotating(key)}>
                            <RefreshCcw data-icon="inline-start" />
                            Rotate
                          </Button>
                          <Button
                            variant="ghost"
                            className="text-muted-foreground hover:text-destructive"
                            size="icon-sm"
                            aria-label={`Delete ${key.name}`}
                            onClick={() => enqueue({ action: "delete_key", name: key.name })}
                          >
                            <Trash2 />
                          </Button>
                        </div>
                      )}
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </Panel>
      )}
      <AddKey open={adding} onOpenChange={setAdding} existing={keys} />
      <Rotate keyRecord={rotating} onClose={() => setRotating(null)} />
    </div>
  );
}

function AddKey({
  open,
  onOpenChange,
  existing,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  existing: VaultKey[];
}) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [urls, setUrls] = useState("");
  const [preset, setPreset] = useState<InjectPreset>("bearer");
  const [arg, setArg] = useState("");
  const [secret, setSecret] = useState("");
  const [proposing, setProposing] = useState(false);
  const [problemText, setProblem] = useState("");
  const [proposed, setProposed] = useState(false);
  const chosen = INJECT_PRESETS.find((entry) => entry.value === preset)!;
  const trimmed = name.trim();
  const replaces = existing.some((key) => key.name === trimmed);

  const reset = () => {
    setName("");
    setDescription("");
    setUrls("");
    setPreset("bearer");
    setArg("");
    setSecret("");
    setProblem("");
    setProposed(false);
  };

  const close = (next: boolean) => {
    if (!next) setSecret("");
    onOpenChange(next);
  };

  const propose = async () => {
    if (!validName(trimmed) || !description.trim()) {
      setProblem("A name and a description come first; the vetting model reads the description.");
      return;
    }
    setProblem("");
    setProposing(true);
    try {
      const found = await vault<{ approved_urls: string[] }>(
        "POST",
        `/keys/${segment(trimmed)}/propose`,
        { description: description.trim() },
        null,
      );
      if (found.approved_urls.length) {
        setUrls(found.approved_urls.join("\n"));
        setProposed(true);
      } else setProblem("The vetting model had no suggestions this time. The URLs can be typed in below.");
    } catch (failure) {
      setProblem(problem(failure));
    } finally {
      setProposing(false);
    }
  };

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const approved = lines(urls);
    if (!validName(trimmed)) return setProblem("A name is letters, digits, - _ . and at most 64.");
    if (!approved.length) return setProblem("The key needs at least one approved URL. Propose URLs will offer some.");
    if (chosen.arg && !arg.trim()) return setProblem(`Fill in the ${chosen.label.toLowerCase()}.`);
    if (!secret) return setProblem("The key itself goes here.");
    enqueue({
      action: "put_key",
      name: trimmed,
      description: description.trim(),
      approved_urls: approved,
      inject: injectValue(preset, arg),
      secret,
    });
    reset();
    onOpenChange(false);
  };

  return (
    <Sheet open={open} onOpenChange={(details) => close(details.open)}>
      <SheetContent className="w-full sm:max-w-md">
        <form onSubmit={submit} autoComplete="off" className="flex min-h-0 flex-1 flex-col">
          <SheetHeader className="pb-2">
            <SheetTitle>Add a key</SheetTitle>
            <SheetDescription>It joins your pending changes. On Apply the key goes into the vault and is never shown again.</SheetDescription>
          </SheetHeader>
          <div className="flex min-h-0 flex-1 flex-col overflow-y-auto px-6 py-2">
            <FieldGroup className="gap-5">
              <Field>
                <FieldLabel>Name</FieldLabel>
                <Input
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                  placeholder="stripe"
                  spellCheck={false}
                  className="font-mono"
                  autoFocus
                />
                {replaces && <FieldDescription>A key already answers to this name; adding replaces it.</FieldDescription>}
              </Field>
              <Field>
                <FieldLabel>Description</FieldLabel>
                <Textarea
                  value={description}
                  onChange={(event) => setDescription(event.target.value)}
                  rows={2}
                  placeholder="Payments for the shop: charges and refunds"
                />
                <FieldDescription>What the key is for. The vetting model reads this when it proposes URLs.</FieldDescription>
              </Field>
              <Field>
                <div className="flex items-center justify-between gap-2">
                  <FieldLabel>Approved URLs</FieldLabel>
                  <Button type="button" variant="ghost" size="xs" onClick={propose} disabled={proposing}>
                    {proposing ? <Spinner data-icon="inline-start" /> : <Sparkles data-icon="inline-start" />}
                    Propose URLs
                  </Button>
                </div>
                <Textarea
                  value={urls}
                  onChange={(event) => {
                    setUrls(event.target.value);
                    setProposed(false);
                  }}
                  rows={3}
                  spellCheck={false}
                  className="font-mono"
                  placeholder={"https://api.stripe.com/v1/"}
                />
                <FieldDescription>
                  {proposed
                    ? "Proposed by the vetting model from the description. Worth a look before the key goes in."
                    : "One URL prefix per line. The key only ever travels to these."}
                </FieldDescription>
              </Field>
              <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                <Field>
                  <FieldLabel>Inject</FieldLabel>
                  <NativeSelect
                    className="w-full"
                    value={preset}
                    onChange={(event) => {
                      setPreset(event.target.value as InjectPreset);
                      setArg("");
                    }}
                  >
                    {INJECT_PRESETS.map((entry) => (
                      <NativeSelectOption key={entry.value} value={entry.value}>
                        {entry.label}
                      </NativeSelectOption>
                    ))}
                  </NativeSelect>
                </Field>
                <Field disabled={!chosen.arg}>
                  <FieldLabel>{preset === "query" ? "Parameter" : preset === "template" ? "Template" : "Header"}</FieldLabel>
                  <Input
                    value={arg}
                    onChange={(event) => setArg(event.target.value)}
                    placeholder={chosen.arg ?? "Not needed"}
                    spellCheck={false}
                    className="font-mono"
                  />
                </Field>
              </div>
              <p className="-mt-3 font-mono text-2xs text-muted-foreground">{chosen.hint}</p>
              <Field>
                <FieldLabel>Secret</FieldLabel>
                <Input
                  type="password"
                  value={secret}
                  onChange={(event) => setSecret(event.target.value)}
                  autoComplete="new-password"
                  spellCheck={false}
                  placeholder="sk_live_…"
                />
              </Field>
            </FieldGroup>
          </div>
          <SheetFooter className="border-t pt-4">
            {problemText && <p className="text-xs text-destructive">{problemText}</p>}
            <div className="flex justify-end gap-2">
              <Button type="button" variant="outline" onClick={() => close(false)}>
                Cancel
              </Button>
              <Button type="submit">
                <ListPlus data-icon="inline-start" />
                {replaces ? "Queue replacement" : "Queue key"}
              </Button>
            </div>
          </SheetFooter>
        </form>
      </SheetContent>
    </Sheet>
  );
}

function Rotate({
  keyRecord,
  onClose,
}: {
  keyRecord: VaultKey | null;
  onClose: () => void;
}) {
  const [secret, setSecret] = useState("");
  const close = () => {
    setSecret("");
    onClose();
  };
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!keyRecord || !secret) return;
    enqueue({ action: "rotate_key", name: keyRecord.name, secret });
    close();
  };
  return (
    <AlertDialog open={keyRecord !== null} onOpenChange={(details) => !details.open && close()}>
      <AlertDialogContent>
        <form onSubmit={submit} autoComplete="off" className="grid gap-6">
          <AlertDialogHeader>
            <AlertDialogMedia>
              <RefreshCcw />
            </AlertDialogMedia>
            <AlertDialogTitle>Rotate {keyRecord?.name}</AlertDialogTitle>
            <AlertDialogDescription>
              On Apply the new key replaces the old one and is never shown again. Approved URLs stay as they are.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <Field>
            <FieldLabel>New key</FieldLabel>
            <Input
              type="password"
              value={secret}
              onChange={(event) => setSecret(event.target.value)}
              autoComplete="new-password"
              spellCheck={false}
              autoFocus
            />
          </Field>
          <AlertDialogFooter>
            <AlertDialogCancel type="button">Cancel</AlertDialogCancel>
            <Button type="submit" disabled={!secret}>
              Queue rotation
            </Button>
          </AlertDialogFooter>
        </form>
      </AlertDialogContent>
    </AlertDialog>
  );
}
