import { cn } from "cn";
import { Bot, Plus, WandSparkles } from "lucide-react";
import { type FormEvent, useMemo, useState } from "react";
import { Link, useNavigate } from "react-router";
import { HarnessLogo } from "@/components/run-view";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Sheet, SheetContent, SheetDescription, SheetFooter, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Spinner } from "@/components/ui/spinner";
import { Textarea } from "@/components/ui/textarea";
import { type Agent, asName, enhanceAgent, NAME_RULE, type NewAgent, saveAgent } from "@/lib/agents";
import { ApiError } from "@/lib/api";
import type { Harness } from "@/lib/harnesses";

type FieldName = "name" | "harness" | "model" | "personality" | "folder";
type Errors = Partial<Record<FieldName, string>>;

const FIELD_OF: Record<string, FieldName> = {
  name_required: "name",
  invalid_name: "name",
  unknown_harness: "harness",
  unknown_model: "model",
  personality_too_long: "personality",
  folder_required: "folder",
  not_a_folder: "folder",
};

type Expansion = { state: "idle" } | { state: "busy" } | { state: "shown"; prompt: string } | { state: "note"; text: string };

function HarnessPicker({ harnesses, value, onChange }: { harnesses: Harness[]; value: string; onChange: (id: string) => void }) {
  return (
    <div role="radiogroup" aria-label="Harness" className="grid grid-cols-2 gap-2">
      {harnesses.map((harness) => {
        const chosen = harness.id === value;
        return (
          <button
            key={harness.id}
            type="button"
            role="radio"
            aria-checked={chosen}
            onClick={() => onChange(harness.id)}
            className={cn(
              "flex min-w-0 items-center gap-2.5 rounded-xl px-2.5 py-2 text-left text-sm outline-1 -outline-offset-1 outline-border transition outline-solid hover:bg-fill-subtle focus-visible:ring-2 focus-visible:ring-ring/30",
              chosen && "bg-fill-subtle outline-foreground/50",
            )}
          >
            <HarnessLogo id={harness.id} harness={harness} size="sm" />
            <span className="truncate font-medium">{harness.label}</span>
          </button>
        );
      })}
    </div>
  );
}

function NoHarness({ onOpen }: { onOpen: () => void }) {
  return (
    <div className="flex items-center justify-between gap-3 rounded-xl bg-fill-subtle px-3 py-2.5">
      <span className="flex items-center gap-2 text-xs text-muted-foreground">
        <Bot className="size-4 shrink-0" />
        No harness here is installed and signed in.
      </span>
      <Button size="sm" variant="outline" asChild onClick={onOpen}>
        <Link to="/harnesses">Open Tools</Link>
      </Button>
    </div>
  );
}

export function NewAgentSheet({ harnesses, onSaved }: { harnesses: Harness[] | null; onSaved: (agent: Agent) => void }) {
  const navigate = useNavigate();
  const ready = useMemo(() => (harnesses ?? []).filter((harness) => harness.connected), [harnesses]);
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [harness, setHarness] = useState("");
  const [model, setModel] = useState("");
  const [words, setWords] = useState("");
  const [folder, setFolder] = useState("");
  const [errors, setErrors] = useState<Errors>({});
  const [failure, setFailure] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [expansion, setExpansion] = useState<Expansion>({ state: "idle" });
  const [kept, setKept] = useState<{ fields: string; agent: Agent } | null>(null);

  const chosen = ready.find((one) => one.id === harness) ?? ready[0];
  const fields: NewAgent = {
    name: name.trim(),
    harness: chosen?.id ?? "",
    model: model || undefined,
    personality: words.trim() || undefined,
    folder: folder.trim() || undefined,
  };
  const key = JSON.stringify(fields);

  const reset = () => {
    setName("");
    setHarness("");
    setModel("");
    setWords("");
    setFolder("");
    setErrors({});
    setFailure(null);
    setExpansion({ state: "idle" });
    setKept(null);
  };

  const check = (): boolean => {
    const found: Errors = {};
    if (!fields.name) found.name = "It needs a name.";
    else if (!NAME_RULE.test(fields.name)) found.name = "Lowercase letters, digits and dashes, like code-review.";
    if (!fields.harness) found.harness = "No harness is ready.";
    setErrors(found);
    return Object.keys(found).length === 0;
  };

  const caught = (error: unknown) => {
    const problem = error as ApiError;
    const field = FIELD_OF[problem.code];
    if (field) setErrors({ [field]: problem.message });
    else setFailure(problem.message);
  };

  const keep = async () => {
    const { agent } = await saveAgent(fields);
    setKept({ fields: key, agent });
    onSaved(agent);
    return agent;
  };

  const expand = async () => {
    setFailure(null);
    if (!check()) return;
    if (!fields.personality) {
      setErrors({ personality: "A few words first." });
      return;
    }
    setExpansion({ state: "busy" });
    try {
      const agent = await keep();
      if (agent.personality?.enhanced) {
        setExpansion({ state: "shown", prompt: agent.personality.content });
        return;
      }
      const result = await enhanceAgent(agent.name);
      setKept({ fields: key, agent: result.agent });
      const personality = result.agent.personality;
      if (personality?.enhanced) setExpansion({ state: "shown", prompt: personality.content });
      else if (result.status === "failed") setExpansion({ state: "note", text: result.reason ?? "It could not be expanded." });
      else setExpansion({ state: "note", text: "These words are kept as they are." });
    } catch (error) {
      setExpansion({ state: "idle" });
      caught(error);
    }
  };

  const submit = async (event?: FormEvent) => {
    event?.preventDefault();
    setFailure(null);
    if (!check()) return;
    setSaving(true);
    try {
      const agent = kept?.fields === key ? kept.agent : await keep();
      setOpen(false);
      reset();
      navigate(`/agents/${encodeURIComponent(agent.name)}`);
    } catch (error) {
      caught(error);
    } finally {
      setSaving(false);
    }
  };

  const busy = saving || expansion.state === "busy";

  return (
    <Sheet
      open={open}
      onOpenChange={(details) => {
        setOpen(details.open);
        if (!details.open) reset();
      }}
    >
      <Button onClick={() => setOpen(true)}>
        <Plus data-icon="inline-start" />
        New agent
      </Button>
      <SheetContent className="w-full sm:max-w-md">
        <SheetHeader className="pr-12">
          <SheetTitle>New agent</SheetTitle>
          <SheetDescription>A harness, a model, a personality and a folder, kept under one name.</SheetDescription>
        </SheetHeader>
        <form onSubmit={submit} noValidate className="flex flex-1 flex-col gap-6 overflow-y-auto px-6 pb-2">
          {failure && (
            <Alert variant="destructive">
              <AlertDescription>{failure}</AlertDescription>
            </Alert>
          )}
          <FieldGroup className="gap-5">
            <Field className="data-[invalid=true]:text-foreground" invalid={Boolean(errors.name)}>
              <FieldLabel>Name</FieldLabel>
              <Input
                value={name}
                onChange={(event) => setName(asName(event.target.value))}
                placeholder="code-review"
                className="font-mono"
                autoFocus
                disabled={busy}
              />
              <FieldError className="text-destructive">{errors.name}</FieldError>
            </Field>
            <Field className="data-[invalid=true]:text-foreground" invalid={Boolean(errors.harness)}>
              <FieldLabel>Harness</FieldLabel>
              {harnesses === null ? (
                <Spinner className="size-4 text-muted-foreground" />
              ) : ready.length === 0 ? (
                <NoHarness onOpen={() => setOpen(false)} />
              ) : (
                <HarnessPicker
                  harnesses={ready}
                  value={chosen?.id ?? ""}
                  onChange={(id) => {
                    setHarness(id);
                    setModel("");
                  }}
                />
              )}
              <FieldError className="text-destructive">{errors.harness}</FieldError>
            </Field>
            {chosen && (
              <Field className="data-[invalid=true]:text-foreground" invalid={Boolean(errors.model)}>
                <FieldLabel>Model</FieldLabel>
                {chosen.models.length > 0 ? (
                  <NativeSelect value={model} onChange={(event) => setModel(event.target.value)} className="w-full" disabled={busy}>
                    <NativeSelectOption value="">{chosen.label}'s own pick</NativeSelectOption>
                    {chosen.models.map((one) => (
                      <NativeSelectOption key={one.id} value={one.id}>
                        {one.label}
                      </NativeSelectOption>
                    ))}
                  </NativeSelect>
                ) : (
                  <Input value={model} onChange={(event) => setModel(event.target.value.trim())} placeholder="provider/model" className="font-mono" disabled={busy} />
                )}
                <FieldError className="text-destructive">{errors.model}</FieldError>
              </Field>
            )}
          </FieldGroup>
          <FieldGroup className="gap-5 border-t pt-6">
            <Field className="data-[invalid=true]:text-foreground" invalid={Boolean(errors.personality)}>
              <div className="flex items-center justify-between gap-2">
                <FieldLabel>Personality</FieldLabel>
                <Button type="button" size="xs" variant="ghost" onClick={() => void expand()} disabled={busy || !words.trim()}>
                  {expansion.state === "busy" ? <Spinner data-icon="inline-start" /> : <WandSparkles data-icon="inline-start" />}
                  {expansion.state === "busy" ? "Expanding" : "Expand"}
                </Button>
              </div>
              <Textarea
                value={words}
                onChange={(event) => {
                  setWords(event.target.value);
                  if (expansion.state !== "busy") setExpansion({ state: "idle" });
                }}
                placeholder="A careful reviewer who explains each change"
                className="min-h-14"
                disabled={busy}
              />
              <FieldDescription>A few words grow into a full prompt.</FieldDescription>
              <FieldError className="text-destructive">{errors.personality}</FieldError>
              {expansion.state === "shown" && (
                <pre className="max-h-56 overflow-auto rounded-xl bg-fill-subtle p-3 font-sans text-xs leading-5 whitespace-pre-wrap text-foreground">
                  {expansion.prompt}
                </pre>
              )}
              {expansion.state === "note" && <p className="text-xs text-muted-foreground">{expansion.text}</p>}
            </Field>
            <Field className="data-[invalid=true]:text-foreground" invalid={Boolean(errors.folder)}>
              <FieldLabel>Folder</FieldLabel>
              <Input value={folder} onChange={(event) => setFolder(event.target.value)} placeholder="/Users/you/Documents/project" className="font-mono" disabled={busy} />
              <FieldDescription>Where it works when a message names no other folder.</FieldDescription>
              <FieldError className="text-destructive">{errors.folder}</FieldError>
            </Field>
          </FieldGroup>
          <button type="submit" hidden />
        </form>
        <SheetFooter className="flex-row justify-end">
          <Button onClick={() => void submit()} disabled={busy || ready.length === 0}>
            {saving && <Spinner data-icon="inline-start" />}
            {saving ? "Saving" : "Save agent"}
          </Button>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
}
