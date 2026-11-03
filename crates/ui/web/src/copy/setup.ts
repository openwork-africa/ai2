export type Branch = "macos" | "linux" | "unsupported";
export type StepId = "containers" | "master-key" | "presence" | "daemon" | "dashboard";
export type StepCopy = { title: string; why: string; fact: string };

export const STEPS: StepId[] = ["containers", "master-key", "presence", "daemon", "dashboard"];

const copy: Record<StepId, Partial<Record<Branch, StepCopy>>> = {
  containers: {
    macos: {
      title: "Containers",
      why: "Agents work inside their own tiny Linux VM, so they can only see what you give them.",
      fact: "Apple's `container` boots a whole Linux virtual machine for each sandbox in well under a second. The vault's socket is the only way out.",
    },
    linux: {
      title: "Sandboxes",
      why: "Each sandbox gets its own socket to the vault, and only you and the vault's group can open it.",
      fact: "Unix sockets have guarded local services since 4.2BSD, in 1983. Each one here opens only for you and the vault's group.",
    },
    unsupported: {
      title: "Containers",
      why: "This machine can't box agents in a container yet, so setup carries on and leaves a note.",
      fact: "Apple's `container` needs macOS 26 on Apple silicon; Docker support is coming. Until then an agent on the host can read whatever you can.",
    },
  },
  "master-key": {
    macos: {
      title: "Master key",
      why: "One random key, kept by the Mac, locks every secret in the vault.",
      fact: "The login keychain has guarded Mac passwords since Mac OS 8.6, in 1999. Only the ai squared binary may read this item.",
    },
    linux: {
      title: "Master key",
      why: "One random key, sealed by systemd-creds, locks every secret in the vault.",
      fact: "`systemd-creds` arrived in systemd 250, in December 2021. With a TPM2 chip the key can only be opened on this very board.",
    },
  },
  presence: {
    macos: {
      title: "Only you",
      why: "Every change to the vault needs Touch ID or your password. No process can say yes on your behalf.",
      fact: "Your fingerprint never leaves the Secure Enclave; macOS, and ai squared, only ever get a yes or a no.",
    },
    linux: {
      title: "Only you",
      why: "Every change to the vault needs your login password, or this passphrase if the machine can't ask for it. No process can say yes on your behalf.",
      fact: "Argon2id won the Password Hashing Competition in 2015. It is slow and hungry for memory on purpose, so guessing is expensive.",
    },
  },
  daemon: {
    macos: {
      title: "Vault service",
      why: "The only process that ever holds a key. It wakes when you log in and slips the key into each approved request on the agent's behalf.",
      fact: "`launchd` has been PID 1 on every Mac since Tiger, in 2005. It will start the vault at login and again if it ever stops.",
    },
    linux: {
      title: "Vault service",
      why: "The only process that ever holds a key. It runs under its own user and sees the rest of the system as read-only.",
      fact: "`ProtectSystem=strict` makes the whole filesystem read-only for the service except the two folders it is handed.",
    },
  },
  dashboard: {
    macos: {
      title: "Dashboard",
      why: "This page starts when you log in and keeps your apps running, at one address you can bookmark.",
      fact: "A LaunchAgent runs as you, not as root, so the dashboard can reach only what you can. `launchd` brings it back if it ever stops.",
    },
    linux: {
      title: "Dashboard",
      why: "This page starts when you log in and keeps your apps running, at one address you can bookmark.",
      fact: "A systemd user unit starts when you log in and stops when you log out; `loginctl enable-linger` keeps it up even then.",
    },
  },
};

export const stepCopy = (id: StepId, branch: Branch): StepCopy => {
  const forStep = copy[id];
  return forStep[branch] ?? forStep.macos ?? (forStep.linux as StepCopy);
};

export const hostLine = {
  title: "What setup found on this machine",
};

export const prompts = {
  passphrase: {
    title: "Choose the vault passphrase",
    why: "Every change to the vault will ask for it. The vault keeps an Argon2id hash, never the passphrase itself.",
    fields: ["Passphrase", "Once more"],
  },
  sudo: {
    title: "Your login password, for sudo",
    why: "Setup writes to /etc and installs a system service. sudo asks once and remembers you for a few minutes.",
    fields: ["Password"],
  },
};

export const failureHint = "Once that's sorted, Try again picks up from this step.";
export const localOnly = "All of this happens on this machine. Nothing is sent anywhere.";

export const machineName = (os: string | undefined) => (os === "macos" ? "this Mac" : "this machine");

export const strip = {
  idle: (machine: string) => `Set up ${machine}`,
  checking: (machine: string) => `Checking ${machine}`,
  running: (machine: string) => `Setting up ${machine}`,
  ready: "Ready",
  stopped: "Setup paused",
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
  noLog: "Nothing to show yet.",
  tryAgain: "Try again",
  copyLog: "Copy the log",
  copied: "Copied",
};

export const inputText = {
  continue: "Continue",
  cancel: "Stop setup",
  differ: "Those two don't match.",
};

export const doneText = {
  setUp: (machine: string) => `${machine.charAt(0).toUpperCase()}${machine.slice(1)} is set up`,
  checked: (machine: string) => `Checked ${machine}`,
  allReady: "Every step was already in place, so nothing was touched.",
  needed: (count: number) => `${count} ${count === 1 ? "step needs" : "steps need"} setting up. Nothing was touched.`,
  stopped: (title: string) => `Setup paused at ${title}`,
  address: "Your address",
  bookmark: "Worth a bookmark, and ai2 can show it again whenever you like.",
  open: "Open ai squared",
  setItUp: "Set it up",
};

export const idleText = {
  title: "Nothing has happened yet",
  why: "Setup touches only what's missing, and it asks here before anything that needs a password.",
  start: "Set up",
  check: "Just look",
};

export const fullLog = "Full log";
export const fullLogWhy = "Every line setup wrote, in order.";
export const disconnected = "Reconnecting to ai squared";
