import { cn } from "cn";
import { CircleAlert, CircleCheck, KeyRound, Plus, Trash2, Zap } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Code, Dot } from "@/components/page";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Sheet, SheetContent, SheetDescription, SheetFooter, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Spinner } from "@/components/ui/spinner";
import { present } from "@/hooks/use-vault";
import { host } from "@/lib/vault";
import {
  API_LABELS,
  type Api,
  listProviders,
  type Preset,
  PRESETS,
  type Provider,
  presetOf,
  providerProblem,
  type Tested,
  testProvider,
} from "@/lib/settings";

type Form = { preset: string; name: string; api: Api; base_url: string; key: string };

const blank = (preset: Preset): Form => ({
  preset: preset.id,
  name: preset.name,
  api: preset.api,
  base_url: preset.base_url,
  key: "",
});

const fromProvider = (provider: Provider): Form => ({
  preset: presetOf(provider).id,
  name: provider.name,
  api: provider.api,
  base_url: provider.base_url,
  key: "",
});

export function Outcome({ tested }: { tested: Tested | null }) {
  if (!tested) return null;
  return (
    <p className={cn("flex items-start gap-1.5 text-xs", tested.ok ? "text-success" : "text-destructive")}>
      {tested.ok ? <CircleCheck className="mt-px size-3.5 shrink-0" /> : <CircleAlert className="mt-px size-3.5 shrink-0" />}
      <span className="min-w-0 break-words">{tested.message}</span>
    </p>
  );
}

function ProviderSheet({
  open,
  editing,
  onClose,
  onSaved,
}: {
  open: boolean;
  editing: Provider | null;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [form, setForm] = useState<Form>(() => (editing ? fromProvider(editing) : blank(PRESETS[0])));
  const [saving, setSaving] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const [kept, setKept] = useState<Provider | null>(editing);
  const [tested, setTested] = useState<Tested | null>(null);
  const [testing, setTesting] = useState(false);
  const custom = form.preset === "custom";
  const adding = !kept?.stored;
  const problem = providerProblem(form, adding);
  const preset = PRESETS.find((entry) => entry.id === form.preset) ?? PRESETS[0];

  const pick = (next: Preset) => {
    setForm((now) => ({ ...blank(next), key: now.key, name: editing ? now.name : next.name }));
    setFailure(null);
  };

  const save = async () => {
    setSaving(true);
    setFailure(null);
    setTested(null);
    try {
      const body = { api: form.api, base_url: form.base_url.trim(), ...(form.key.trim() ? { key: form.key } : {}) };
      const saved = await present<Provider>("PUT", `/api/providers/${encodeURIComponent(form.name)}`, body, "keep this key");
      setKept(saved);
      setForm((now) => ({ ...now, key: "" }));
      onSaved();
    } catch (error) {
      setFailure((error as Error).message);
    } finally {
      setSaving(false);
    }
  };

  const test = async () => {
    if (!kept) return;
    setTesting(true);
    try {
      setTested(await testProvider(kept.name));
    } catch (error) {
      setTested({ ok: false, message: (error as Error).message });
    } finally {
      setTesting(false);
    }
  };

  return (
    <Sheet open={open} onOpenChange={(details) => !details.open && onClose()}>
      <SheetContent className="w-full sm:max-w-md">
        <SheetHeader className="pr-12">
          <SheetTitle>{editing ? editing.name : "Add a model provider"}</SheetTitle>
          <SheetDescription>Its key goes in the vault, and ai squared only ever holds a pass to it.</SheetDescription>
        </SheetHeader>
        <form
          className="flex flex-1 flex-col gap-5 overflow-y-auto px-6"
          autoComplete="off"
          onSubmit={(event) => {
            event.preventDefault();
            if (!problem && !saving) void save();
          }}
        >
          <Field>
            <FieldLabel>Provider</FieldLabel>
            <div className="grid grid-cols-4 gap-1 rounded-xl bg-fill-default p-1">
              {PRESETS.map((entry) => (
                <button
                  key={entry.id}
                  type="button"
                  aria-pressed={form.preset === entry.id}
                  className={cn(
                    "rounded-lg px-2 py-1.5 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground",
                    form.preset === entry.id && "bg-background text-foreground shadow-(--shadow-secondary-control)",
                  )}
                  onClick={() => pick(entry)}
                >
                  {entry.label}
                </button>
              ))}
            </div>
            <FieldDescription>{preset.hint}</FieldDescription>
          </Field>
          <Field>
            <FieldLabel>Name</FieldLabel>
            <Input
              value={form.name}
              onChange={(event) => setForm({ ...form, name: event.target.value })}
              placeholder="my-models"
              className="font-mono text-xs"
              disabled={Boolean(editing)}
            />
            <FieldDescription>The key is kept in the vault under this name.</FieldDescription>
          </Field>
          {custom && (
            <Field>
              <FieldLabel>API style</FieldLabel>
              <NativeSelect className="w-full" value={form.api} onChange={(event) => setForm({ ...form, api: event.target.value as Api })}>
                {(Object.keys(API_LABELS) as Api[]).map((api) => (
                  <NativeSelectOption key={api} value={api}>
                    {API_LABELS[api]}
                  </NativeSelectOption>
                ))}
              </NativeSelect>
            </Field>
          )}
          <Field>
            <FieldLabel>Base URL</FieldLabel>
            <Input
              value={form.base_url}
              onChange={(event) => setForm({ ...form, base_url: event.target.value })}
              placeholder="https://api.example.com/v1"
              className="font-mono text-xs"
            />
            <FieldDescription>{API_LABELS[form.api]}, sent only to this address.</FieldDescription>
          </Field>
          <Field>
            <FieldLabel>API key</FieldLabel>
            <Input
              type="password"
              value={form.key}
              onChange={(event) => setForm({ ...form, key: event.target.value })}
              placeholder={kept?.stored ? "••••••••  kept in the vault" : "sk-…"}
              className="font-mono text-xs"
              autoComplete="new-password"
            />
            {kept?.stored ? (
              <p className="flex items-center gap-1.5 text-xs text-success">
                <KeyRound className="size-3.5" />
                Key stored in the vault
              </p>
            ) : (
              <FieldDescription>It is never shown again or written to a file.</FieldDescription>
            )}
          </Field>
          {failure && <p className="text-sm text-destructive">{failure}</p>}
          <Outcome tested={tested} />
          <button type="submit" hidden />
        </form>
        <SheetFooter className="flex-row justify-end">
          <Button type="button" variant="outline" disabled={!kept?.stored || testing} onClick={() => void test()}>
            {testing ? <Spinner data-icon="inline-start" /> : <Zap data-icon="inline-start" />}
            Test
          </Button>
          <Button type="button" disabled={Boolean(problem) || saving} onClick={() => void save()} title={problem ?? undefined}>
            {saving && <Spinner data-icon="inline-start" />}
            {saving ? "Saving" : "Save"}
          </Button>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
}

function ProviderCard({
  provider,
  onEdit,
  onRemoved,
}: {
  provider: Provider;
  onEdit: () => void;
  onRemoved: () => void;
}) {
  const [busy, setBusy] = useState<"test" | "remove" | null>(null);
  const [tested, setTested] = useState<Tested | null>(null);
  const kept = provider.key === "vault" && provider.stored;

  const test = async () => {
    setBusy("test");
    try {
      setTested(await testProvider(provider.name));
    } catch (error) {
      setTested({ ok: false, message: (error as Error).message });
    } finally {
      setBusy(null);
    }
  };

  const remove = async () => {
    setBusy("remove");
    try {
      await present("DELETE", `/api/providers/${encodeURIComponent(provider.name)}`, undefined, "take this key out");
      onRemoved();
    } catch (error) {
      setTested({ ok: false, message: (error as Error).message });
      setBusy(null);
    }
  };

  return (
    <Card size="sm" className="gap-4" data-provider={provider.name}>
      <button type="button" className="flex min-w-0 items-start gap-3 px-(--card-spacing) text-left" onClick={onEdit}>
        <span className="flex size-11 shrink-0 items-center justify-center rounded-xl bg-fill-default">
          <KeyRound className="size-5 text-muted-foreground" />
        </span>
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="truncate font-heading text-sm font-semibold tracking-tight">{provider.name}</span>
          <span className="truncate text-xs text-muted-foreground">
            <Code className="bg-transparent px-0 text-muted-foreground">{host(provider.base_url)}</Code>
            <span className="px-1.5 text-muted-foreground/50">·</span>
            {API_LABELS[provider.api] ?? provider.api}
          </span>
        </span>
      </button>
      <div className="flex flex-col gap-3 px-(--card-spacing)">
        <Outcome tested={tested} />
        <div className="-mx-(--card-spacing) flex items-center justify-between gap-2 border-t px-(--card-spacing) pt-3">
          <span className="flex min-w-0 items-center gap-1.5 truncate text-2xs text-muted-foreground">
            <Dot tone={kept ? "success" : "warning"} className="ring-0" />
            {kept ? "Key in the vault" : provider.key === "vault" ? "No key in the vault" : "Key kept outside the vault"}
          </span>
          <div className="flex items-center gap-1">
            <Button type="button" size="sm" variant="ghost" disabled={busy !== null || !provider.pass} onClick={() => void test()}>
              {busy === "test" ? <Spinner /> : <Zap />}
              Test
            </Button>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={busy !== null}
              aria-label={`Remove ${provider.name}`}
              onClick={() => void remove()}
            >
              {busy === "remove" ? <Spinner /> : <Trash2 />}
            </Button>
          </div>
        </div>
      </div>
    </Card>
  );
}

export function ModelProviders() {
  const [providers, setProviders] = useState<Provider[] | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [sheet, setSheet] = useState<{ editing: Provider | null; at: number } | null>(null);

  const load = useCallback(
    () =>
      listProviders()
        .then(({ providers }) => {
          setProviders(providers);
          setFailed(null);
        })
        .catch((error: Error) => setFailed(error.message)),
    [],
  );

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <section className="flex flex-col gap-3" aria-labelledby="model-providers">
      <div className="flex items-center justify-between gap-2">
        <h2 id="model-providers" className="flex items-baseline gap-2 text-xs font-medium tracking-wide text-muted-foreground uppercase">
          Model providers
          <span className="text-muted-foreground/60 tabular-nums">{providers?.length ?? 0}</span>
        </h2>
        <Button type="button" size="sm" variant="outline" onClick={() => setSheet({ editing: null, at: Date.now() })}>
          <Plus />
          Add a provider
        </Button>
      </div>
      {failed && <p className="text-sm text-destructive">{failed}</p>}
      {providers && providers.length === 0 && (
        <Card size="sm" className="items-center border-dashed py-8 text-center shadow-none">
          <p className="px-6 text-sm text-muted-foreground">No model provider yet. A key added here stays in the vault.</p>
        </Card>
      )}
      {providers && providers.length > 0 && (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          {providers.map((provider) => (
            <ProviderCard
              key={provider.name}
              provider={provider}
              onEdit={() => setSheet({ editing: provider, at: Date.now() })}
              onRemoved={() => void load()}
            />
          ))}
        </div>
      )}
      {sheet && (
        <ProviderSheet
          key={sheet.at}
          open
          editing={sheet.editing}
          onClose={() => setSheet(null)}
          onSaved={() => void load()}
        />
      )}
    </section>
  );
}
