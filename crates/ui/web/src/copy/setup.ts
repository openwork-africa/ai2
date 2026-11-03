export type Branch = "macos" | "linux" | "unsupported";
export type StepId = "containers" | "master-key" | "presence" | "daemon";
export type StepCopy = { title: string; why: string; fact: string };

export const STEPS: StepId[] = ["containers", "master-key", "presence", "daemon"];

const copy: Record<StepId, Partial<Record<Branch, StepCopy>>> = {
  containers: {
    macos: {
      title: "Containers",
      why: "Agents work inside their own tiny Linux VM, so they can only see what you give them.",
      fact: "Apple's `container` boots a whole Linux virtual machine for each sandbox in well under a second. The vault's socket is the only way out.",
    },
    linux: {
      title: "Sandboxes",
      why: "Each agent run becomes a throwaway systemd unit that can reach only its own vault socket.",
      fact: "`systemd-run` has been making one-off services since systemd 205, in 2013. Here each one gets a socket folder nobody else can open.",
    },
    unsupported: {
      title: "Containers",
      why: "This machine cannot keep agents in a container yet; setup carries on, with a warning.",
      fact: "Apple's `container` needs macOS 26 on Apple silicon; Docker support is coming. Until then an agent on the host can read what you can.",
    },
  },
  "master-key": {
    macos: {
      title: "Master key",
      why: "One random key, kept by the Mac, locks every secret in the vault.",
      fact: "The login keychain has guarded Mac passwords since Mac OS 8.6, in 1999. Only the ai2 binary may read this item.",
    },
    linux: {
      title: "Master key",
      why: "One random key, sealed by systemd-creds, locks every secret in the vault.",
      fact: "`systemd-creds` arrived in systemd 250, in December 2021. With a TPM2 chip the key can only be opened on this very board.",
    },
  },
  presence: {
    macos: {
      title: "Human check",
      why: "Every change to the vault needs Touch ID or your password, so a process cannot approve itself.",
      fact: "Your fingerprint never leaves the Secure Enclave; macOS, and ai2, only ever get a yes or a no.",
    },
    linux: {
      title: "Human check",
      why: "Every change to the vault needs this passphrase, so a process cannot approve itself.",
      fact: "Argon2id won the Password Hashing Competition in 2015. It is slow and hungry for memory on purpose, so guessing is expensive.",
    },
  },
  daemon: {
    macos: {
      title: "Vault service",
      why: "The one process that ever holds a secret; it starts with you and injects keys into requests on the agent's behalf.",
      fact: "`launchd` has been PID 1 on every Mac since Tiger, in 2005. It will start the vault at login and again if it ever stops.",
    },
    linux: {
      title: "Vault service",
      why: "The one process that ever holds a secret, under its own user, with the rest of the system read-only to it.",
      fact: "`ProtectSystem=strict` makes the whole filesystem read-only for the service except the two folders it is handed.",
    },
  },
};

export const stepCopy = (id: StepId, branch: Branch): StepCopy => {
  const forStep = copy[id];
  return forStep[branch] ?? forStep.macos ?? (forStep.linux as StepCopy);
};

export const hostLine = {
  title: "Looking at this machine",
};

export const prompts = {
  passphrase: {
    title: "Choose the vault passphrase",
    why: "It is asked for every change to the vault. It is hashed with Argon2id; the vault never keeps it.",
    fields: ["Passphrase", "The same again"],
  },
  sudo: {
    title: "Your password, for sudo",
    why: "Setup writes to /etc and installs a system service; sudo asks once and remembers for a few minutes.",
    fields: ["Password"],
  },
};

export const failureHint = "Fix that, then press Try again; setup picks up from here.";
export const localOnly = "Everything here runs on this machine. Nothing is sent anywhere.";

export const machineName = (os: string | undefined) => (os === "macos" ? "this Mac" : "this machine");

export const strip = {
  idle: (machine: string) => `Set up ${machine}`,
  checking: (machine: string) => `Checking ${machine}`,
  running: (machine: string) => `Setting up ${machine}`,
  ready: "Ready",
  stopped: "Setup stopped",
  checked: "Checked",
  step: (current: number, total: number) => `Step ${current} of ${total}`,
  of: (done: number, total: number) => `${done} of ${total}`,
};

export const states = {
  warned: "Done, with a note",
  unsupported: "Not on this machine",
  failed: "Failed",
  skipped: "Skipped",
  missing: "Not set up yet",
  unknown: "Could not tell",
  input: "Needs you",
};

export const stepText = {
  showLog: "Show the log",
  hideLog: "Hide the log",
  noLog: "Nothing written yet.",
  tryAgain: "Try again",
  copyLog: "Copy the log",
  copied: "Copied",
};

export const inputText = {
  continue: "Continue",
  cancel: "Cancel setup",
  differ: "The two passphrases differ. Type the same one twice.",
};

export const doneText = {
  setUp: (machine: string) => `${machine.charAt(0).toUpperCase()}${machine.slice(1)} is set up`,
  checked: (machine: string) => `Checked ${machine}`,
  allReady: "Every step is in place. Nothing was changed.",
  needed: (count: number) => `${count} ${count === 1 ? "step needs" : "steps need"} setting up. Nothing was changed.`,
  stopped: (title: string) => `Setup stopped at ${title}`,
  address: "Your address",
  bookmark: "Bookmark it; ai2 ui prints it again.",
  open: "Open ai2",
  setItUp: "Set it up",
};

export const idleText = {
  title: "Nothing has run yet",
  why: "Setup changes only what is missing, and asks here before anything needs a password.",
  start: "Set up",
  check: "Check only",
};

export const fullLog = "Full log";
export const fullLogWhy = "Every line setup wrote, step by step.";
export const disconnected = "Reconnecting to ai2";
