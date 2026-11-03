export type DenyReason =
  | "token_unknown"
  | "token_expired"
  | "token_used_up"
  | "wrong_sandbox"
  | "wrong_user"
  | "key_not_allowed"
  | "method_not_allowed"
  | "url_not_allowed"
  | "vetting_denied"
  | "vetting_unavailable"
  | "approval_denied"
  | "approval_timeout";

export declare class VaultDenied extends Error {
  constructor(reason: DenyReason | string, intent?: string);
  readonly name: "VaultDenied";
  readonly reason: DenyReason | string;
  readonly intent: string | undefined;
}

export interface VaultOptions {
  token?: string;
  endpoint?: `unix:${string}` | `http://${string}` | string;
}

export interface VaultRequestInit extends RequestInit {
  request?: string;
}

export type FetchLike = (input: string | URL | Request, init?: RequestInit) => Promise<Response>;

export interface Vault {
  fetch(key: string, url: string | URL | Request, init?: VaultRequestInit): Promise<Response>;
  wrap<F extends FetchLike>(fetchFn: F, routes: Record<string, string>): (input: string | URL | Request, init?: VaultRequestInit) => Promise<Response>;
}

export declare function vault(options?: VaultOptions): Vault;
