import { cn } from "cn";
import { Bot, Check, RotateCw, Settings, Sparkles, Wrench, Zap } from "lucide-react";
import { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router";
import { Outcome } from "@/components/providers";
import { Button } from "@/components/ui/button";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Sheet, SheetContent, SheetDescription, SheetFooter, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import { type Harness, listHarnesses } from "@/lib/harnesses";
import {
  type Choice,
  getSettings,
  listProviders,
  type Page,
  type Provider,
  providerModels,
  restartVault,
  runsHarness,
  saveSettings,
  type Tested,
  testSettings,
  usable,
  waiting,
} from "@/lib/settings";

const COPY: Record<Page, { title: string; description: string; harness: string; model: string }> = {
  security: {
    title: "Security settings",
    description: "What reads the code an agent changed and rates the risk.",
    harness: "A coding agent reads the folder with the four security skills.",
    model: "A model reads the changed files and lists what it finds.",
  },
  vault: {
    title: "Vault settings",
    description: "What checks each call an agent makes with one of your keys.",
    harness: "A coding agent weighs each call against what the agent was asked to do.",
    model: "A model weighs each call against what the agent was asked to do.",
  },
};

type Way = "harness" | "model";

type Loaded = { choice: Choice; harnesses: Harness[]; providers: Provider[] };

function Row({
  chosen,
  onPick,
  icon,
  title,
  line,
  disabled,
  after,
}: {
  chosen: boolean;
  onPick?: () => void;
  icon: React.ReactNode;
  title: string;
  line: string;
  disabled?: boolean;
  after?: React.ReactNode;
}) {
  return (
    <div
      role="radio"
      aria-checked={chosen}
      aria-disabled={disabled}
      tabIndex={disabled ? -1 : 0}
      onClick={disabled ? undefined : onPick}
      onKeyDown={(event) => {
        if (!disabled && (event.key === "Enter" || event.key === " ")) {
          event.preventDefault();
          onPick?.();
        }
      }}
      className={cn(
        "flex items-center gap-3 rounded-xl px-3 py-2.5 outline-1 -outline-offset-1 outline-transparent transition-colors outline-solid",
        disabled ? "opacity-55" : "cursor-pointer hover:bg-fill-default focus-visible:outline-ring",
        chosen && "bg-fill-default outline-border",
      )}
    >
      <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-background ring-1 ring-border">{icon}</span>
      <span className="flex min-w-0 flex-1 flex-col">
        <span className="truncate text-sm font-medium">{title}</span>
        <span className="truncate text-xs text-muted-foreground">{line}</span>
      </span>
      {after}
      {chosen && <Check className="size-4 shrink-0 text-foreground" />}
    </div>
  );
}

function Logo({ harness }: { harness: Harness }) {
  const [broken, setBroken] = useState(false);
  return harness.logo && !broken ? (
    <img src={`/harnesses/${harness.logo}`} alt="" className="size-4" onError={() => setBroken(true)} />
  ) : (
    <Bot className="size-4 text-muted-foreground" />
  );
}

function OpenTools({ line }: { line: string }) {
  const navigate = useNavigate();
  return (
    <div className="flex flex-col items-start gap-3 rounded-xl border border-dashed px-4 py-4">
      <p className="text-sm text-muted-foreground">{line}</p>
      <Button type="button" size="sm" variant="outline" onClick={() => navigate("/harnesses")}>
        <Wrench />
        Open Tools
      </Button>
    </div>
  );
}

function Harnesses({
  page,
  loaded,
  choice,
  setChoice,
}: {
  page: Page;
  loaded: Loaded;
  choice: Choice;
  setChoice: (choice: Choice) => void;
}) {
  const ready = usable(loaded.harnesses);
  const later = waiting(loaded.harnesses);
  const picked = ready.find((harness) => harness.id === choice.runs);
  return (
    <div className="flex flex-col gap-4">
      <div role="radiogroup" aria-label="Harness" className="flex flex-col gap-1">
        {page === "security" && (
          <>
            <Row
              chosen={choice.runs === "auto"}
              onPick={() => setChoice({ runs: "auto", provider: null, model: null })}
              icon={<Sparkles className="size-4 text-muted-foreground" />}
              title="The first one set up"
              line="Muse Code, then Claude Code, Codex and pi."
            />
            <Row
              chosen={choice.runs === "none"}
              onPick={() => setChoice({ runs: "none", provider: null, model: null })}
              icon={<span className="size-2 rounded-full bg-muted-foreground/50" />}
              title="None"
              line="No review, so no risk rating."
            />
          </>
        )}
        {ready.map((harness) => (
          <Row
            key={harness.id}
            chosen={choice.runs === harness.id}
            onPick={() => setChoice({ runs: harness.id, provider: null, model: null })}
            icon={<Logo harness={harness} />}
            title={harness.label}
            line="Set up and signed in."
          />
        ))}
        {later.map((harness) => (
          <Row
            key={harness.id}
            chosen={false}
            disabled
            icon={<Logo harness={harness} />}
            title={harness.label}
            line={harness.installed ? "Not signed in yet." : "Not set up yet."}
            after={
              <Link to="/harnesses" className="shrink-0 text-xs font-medium text-foreground underline-offset-2 hover:underline">
                Tools
              </Link>
            }
          />
        ))}
      </div>
      {ready.length === 0 && <OpenTools line="No coding agent is set up and signed in yet." />}
      {picked && picked.models.length > 0 && (
        <Field>
          <FieldLabel>Model</FieldLabel>
          <NativeSelect
            className="w-full"
            value={choice.model ?? ""}
            onChange={(event) => setChoice({ ...choice, model: event.target.value || null })}
          >
            <NativeSelectOption value="">Its own default</NativeSelectOption>
            {picked.models.map((model) => (
              <NativeSelectOption key={model.id} value={model.id}>
                {model.label}
              </NativeSelectOption>
            ))}
          </NativeSelect>
          <FieldDescription>It runs on your own {picked.label} account.</FieldDescription>
        </Field>
      )}
    </div>
  );
}

function Models({
  loaded,
  choice,
  setChoice,
}: {
  loaded: Loaded;
  choice: Choice;
  setChoice: (choice: Choice) => void;
}) {
  const kept = loaded.providers.filter((provider) => provider.key === "vault" && provider.stored);
  const provider = kept.find((entry) => entry.name === choice.provider) ?? null;
  const [listed, setListed] = useState<{ name: string; models: string[] | null } | null>(null);
  const name = provider?.name ?? null;

  useEffect(() => {
    if (!name) return;
    let alive = true;
    providerModels(name)
      .then(({ models }) => alive && setListed({ name, models }))
      .catch(() => alive && setListed({ name, models: null }));
    return () => {
      alive = false;
    };
  }, [name]);

  if (kept.length === 0) return <OpenTools line="No model provider with a key is added yet." />;
  const models = listed?.name === name ? listed.models : undefined;
  const typed = models === null || (models !== undefined && models.length === 0);
  return (
    <div className="flex flex-col gap-5">
      <Field>
        <FieldLabel>Provider</FieldLabel>
        <NativeSelect
          className="w-full"
          value={choice.provider ?? ""}
          onChange={(event) => setChoice({ runs: "model", provider: event.target.value || null, model: null })}
        >
          <NativeSelectOption value="">Pick one</NativeSelectOption>
          {kept.map((entry) => (
            <NativeSelectOption key={entry.name} value={entry.name}>
              {entry.name}
            </NativeSelectOption>
          ))}
        </NativeSelect>
        <FieldDescription>Its key stays in the vault; calls go out through it.</FieldDescription>
      </Field>
      {provider && (
        <Field>
          <FieldLabel>Model</FieldLabel>
          {models === undefined ? (
            <Skeleton className="h-8 w-full rounded-lg" />
          ) : typed ? (
            <Input
              value={choice.model ?? ""}
              onChange={(event) => setChoice({ ...choice, model: event.target.value })}
              placeholder={provider.models[0] ?? "model-id"}
              className="font-mono text-xs"
            />
          ) : (
            <NativeSelect
              className="w-full"
              value={choice.model ?? ""}
              onChange={(event) => setChoice({ ...choice, model: event.target.value || null })}
            >
              <NativeSelectOption value="">Pick one</NativeSelectOption>
              {models.map((model) => (
                <NativeSelectOption key={model} value={model}>
                  {model}
                </NativeSelectOption>
              ))}
            </NativeSelect>
          )}
          <FieldDescription>
            {typed ? "The provider gave no list, so the model is typed in." : `${models?.length ?? 0} models from ${provider.name}.`}
          </FieldDescription>
        </Field>
      )}
    </div>
  );
}

function Body({ page }: { page: Page }) {
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [choice, setChoice] = useState<Choice>({ runs: "auto", provider: null, model: null });
  const [way, setWay] = useState<Way>("harness");
  const [busy, setBusy] = useState<"test" | "save" | "restart" | null>(null);
  const [tested, setTested] = useState<Tested | null>(null);
  const [restart, setRestart] = useState(false);

  useEffect(() => {
    Promise.all([getSettings(page), listHarnesses(), listProviders()])
      .then(([saved, { harnesses }, { providers }]) => {
        setLoaded({ choice: saved.choice, harnesses, providers });
        setChoice(saved.choice);
        setWay(saved.choice.runs === "model" ? "model" : "harness");
      })
      .catch((error: Error) => setFailed(error.message));
  }, [page]);

  const pick = (next: Choice) => {
    setChoice(next);
    setTested(null);
  };

  const switchTo = (next: Way) => {
    setWay(next);
    setTested(null);
    if (next === "model" && choice.runs !== "model") setChoice({ runs: "model", provider: null, model: null });
    if (next === "harness" && choice.runs === "model") {
      const first = page === "security" ? "auto" : (usable(loaded?.harnesses ?? [])[0]?.id ?? "");
      setChoice({ runs: first, provider: null, model: null });
    }
  };

  const complete =
    choice.runs === "model" ? Boolean(choice.provider && choice.model?.trim()) : choice.runs !== "" && (!runsHarness(choice) || usable(loaded?.harnesses ?? []).some((harness) => harness.id === choice.runs));

  const run = async (what: "test" | "save" | "restart") => {
    setBusy(what);
    try {
      if (what === "test") setTested(await testSettings(page, choice));
      if (what === "save") {
        const saved = await saveSettings(page, choice);
        setRestart(saved.restart);
        setTested({ ok: true, message: saved.restart ? "Saved. The vault reads this when it starts." : "Saved." });
      }
      if (what === "restart") {
        setTested(await restartVault());
        setRestart(false);
      }
    } catch (error) {
      setTested({ ok: false, message: (error as Error).message });
    } finally {
      setBusy(null);
    }
  };

  const copy = COPY[page];

  return (
    <>
      <SheetHeader className="pr-12">
        <SheetTitle>{copy.title}</SheetTitle>
        <SheetDescription>{copy.description}</SheetDescription>
      </SheetHeader>
      <div className="flex flex-1 flex-col gap-5 overflow-y-auto px-6">
        {failed && <p className="text-sm text-destructive">{failed}</p>}
        {!loaded && !failed && (
          <div className="flex flex-col gap-2">
            <Skeleton className="h-9 w-full rounded-xl" />
            <Skeleton className="h-14 w-full rounded-xl" />
            <Skeleton className="h-14 w-full rounded-xl" />
          </div>
        )}
        {loaded && (
          <>
            <div role="tablist" aria-label="What runs" className="grid grid-cols-2 gap-1 rounded-xl bg-fill-default p-1">
              {(["harness", "model"] as const).map((entry) => (
                <button
                  key={entry}
                  type="button"
                  role="tab"
                  aria-selected={way === entry}
                  className={cn(
                    "rounded-lg px-2 py-1.5 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground",
                    way === entry && "bg-background text-foreground shadow-(--shadow-secondary-control)",
                  )}
                  onClick={() => switchTo(entry)}
                >
                  {entry === "harness" ? "Coding agent" : "Model and key"}
                </button>
              ))}
            </div>
            <p className="-mt-2 text-xs text-muted-foreground">{way === "harness" ? copy.harness : copy.model}</p>
            {way === "harness" ? (
              <Harnesses page={page} loaded={loaded} choice={choice} setChoice={pick} />
            ) : (
              <Models loaded={loaded} choice={choice} setChoice={pick} />
            )}
          </>
        )}
      </div>
      <SheetFooter className="flex-row items-center justify-end">
        <div className="mr-auto min-w-0">
          <Outcome tested={tested} />
        </div>
        {restart ? (
          <Button type="button" disabled={busy !== null} onClick={() => void run("restart")}>
            {busy === "restart" ? <Spinner data-icon="inline-start" /> : <RotateCw data-icon="inline-start" />}
            Restart the vault
          </Button>
        ) : (
          <>
            <Button type="button" variant="outline" disabled={!loaded || !complete || busy !== null} onClick={() => void run("test")}>
              {busy === "test" ? <Spinner data-icon="inline-start" /> : <Zap data-icon="inline-start" />}
              Test
            </Button>
            <Button type="button" disabled={!loaded || !complete || busy !== null} onClick={() => void run("save")}>
              {busy === "save" && <Spinner data-icon="inline-start" />}
              Save
            </Button>
          </>
        )}
      </SheetFooter>
    </>
  );
}

export function SettingsCog({ page }: { page: Page }) {
  const [open, setOpen] = useState(false);
  const [at, setAt] = useState(0);
  return (
    <Sheet open={open} onOpenChange={(details) => setOpen(details.open)}>
      <Button
        type="button"
        variant="outline"
        size="icon"
        aria-label={COPY[page].title}
        title={COPY[page].title}
        onClick={() => {
          setAt(Date.now());
          setOpen(true);
        }}
      >
        <Settings />
      </Button>
      <SheetContent className="w-full sm:max-w-md">{open && <Body key={at} page={page} />}</SheetContent>
    </Sheet>
  );
}
