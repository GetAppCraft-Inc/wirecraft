// Typed wrappers over the Tauri commands in src-tauri/src/lib.rs.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type FlowState = "pending" | "complete" | "error" | "tunnel" | "live" | "paused";
export type Headers = [string, string][];

export interface FlowSummary {
  id: number;
  method: string;
  scheme: string;
  host: string;
  port: number;
  path: string;
  httpVersion: string;
  status: number | null;
  state: FlowState;
  kind: string;
  contentType: string | null;
  reqSize: number;
  respSize: number;
  started: number;
  durationMs: number | null;
  client: string | null;
  clientAddr: string;
  tls: boolean;
  xhr: boolean;
  error: string | null;
  note: string | null;
  reqHeaderCount: number;
  respHeaderCount: number;
  applied: string[];
  wsMessages: number;
}

export interface WsMessage {
  at: number;
  outgoing: boolean;
  kind: "text" | "binary" | "ping" | "pong" | "close";
  len: number;
  data: string;
  truncated: boolean;
}

// ---- rules (crates/wirecraft-core/src/rules.rs)
export interface Matcher {
  url: string;
  methods: string[];
}
export interface MapLocal {
  id: string;
  enabled: boolean;
  name: string;
  match: Matcher;
  status: number;
  headers: Headers;
  body: string;
  file: string;
}
export interface MapRemote {
  id: string;
  enabled: boolean;
  name: string;
  match: Matcher;
  to: string;
  replacePath: boolean;
}
export type RewriteTarget = "requestHeader" | "responseHeader" | "requestBody" | "responseBody" | "url" | "queryParam" | "status";
export type RewriteAction = "set" | "add" | "remove" | "replace";
export interface Rewrite {
  id: string;
  enabled: boolean;
  name: string;
  match: Matcher;
  target: RewriteTarget;
  action: RewriteAction;
  key: string;
  value: string;
  find: string;
  regex: boolean;
}
export interface Breakpoint {
  id: string;
  enabled: boolean;
  name: string;
  match: Matcher;
  request: boolean;
  response: boolean;
}
export interface Block {
  id: string;
  enabled: boolean;
  match: Matcher;
}
export interface Throttle {
  enabled: boolean;
  preset: string;
  latencyMs: number;
  downloadKbps: number;
  uploadKbps: number;
  hosts: string[];
}
export interface Script {
  id: string;
  enabled: boolean;
  name: string;
  match: Matcher;
  request: boolean;
  response: boolean;
  code: string;
  timeoutMs: number;
}
export interface ScriptLog {
  seq: number;
  at: number;
  flowId: number | null;
  script: string;
  level: "log" | "info" | "warn" | "error" | "debug";
  message: string;
}
export interface Rules {
  mapLocal: MapLocal[];
  mapRemote: MapRemote[];
  rewrite: Rewrite[];
  breakpoints: Breakpoint[];
  block: Block[];
  throttle: Throttle;
  scripts: Script[];
  scriptsOn: boolean;
  mapLocalOn: boolean;
  mapRemoteOn: boolean;
  rewriteOn: boolean;
  breakpointsOn: boolean;
}
export interface Paused {
  flowId: number;
  phase: "request" | "response";
  method: string;
  url: string;
  status: number | null;
  headers: Headers;
  body: string;
  binary: boolean;
}
export interface Resume {
  abort?: boolean;
  method?: string;
  url?: string;
  status?: number;
  headers?: Headers;
  body?: string;
}
export interface SessionFile {
  name: string;
  path: string;
  size: number;
  modified: number;
}

export interface BodyView {
  size: number;
  decodedSize: number;
  text: string | null;
  base64: string | null;
  truncated: boolean;
  clipped: boolean;
  decodeError: string | null;
  /** Schemaless protobuf decode for gRPC / gRPC-Web / protobuf bodies. */
  grpc: GrpcBody | null;
}

export interface PbField {
  no: number;
  wire: "varint" | "fixed64" | "fixed32" | "len";
  type?: "message" | "string" | "bytes";
  value: number | string | PbField[];
  size?: number;
  sint?: number;
  int64?: number;
  double?: number;
  float?: number;
  packed?: number[];
}
export interface GrpcMessage {
  compressed?: boolean;
  size?: number;
  fields?: PbField[];
  error?: string;
  hex?: string;
}
export interface GrpcBody {
  kind: "grpc" | "grpc-web" | "grpc-web-text" | "protobuf";
  messages?: GrpcMessage[];
  trailers?: Record<string, string>;
  error?: string;
}

export interface Timing {
  started: number;
  requestMs: number | null;
  waitingMs: number | null;
  downloadMs: number | null;
  totalMs: number | null;
}

export interface TlsInfo {
  version: string | null;
  alpn: string | null;
  sni: string | null;
  cipher: string | null;
}

export interface FlowDetail {
  summary: FlowSummary;
  reqHeaders: Headers;
  reqBody: BodyView;
  statusText: string | null;
  respHttpVersion: string | null;
  respHeaders: Headers;
  respBody: BodyView;
  respTrailers: Headers;
  timing: Timing;
  tls: TlsInfo | null;
}

export type StoreEvent =
  | { type: "upsert"; flow: FlowSummary }
  | { type: "removed"; ids: number[] }
  | { type: "cleared" };

export type InterceptMode = "all" | "listed" | "off";
export interface InterceptConfig {
  mode: InterceptMode;
  listed: string[];
  never: string[];
}

export interface Settings {
  port: number;
  listenAll: boolean;
  intercept: InterceptConfig;
  onboarded: boolean;
  systemProxyBypass: string;
  theme: ThemeSetting;
  favourites: string[];
  rules: Rules;
}

export type ThemeSetting = "system" | "dark" | "light";

export interface ProxyStatus {
  running: boolean;
  addr: string | null;
  port: number;
  listenAll: boolean;
  error: string | null;
  lanIp: string | null;
}

export interface CaInfo {
  commonName: string;
  keyAlgorithm: string;
  notBefore: number;
  notAfter: number;
  sha256: string;
  dir: string;
  certPath: string;
}

export interface OsInfo {
  os: "linux" | "macos" | "windows";
  family: string;
  prettyName: string;
  desktop: string;
  hostname: string;
}

export interface AppState {
  version: string;
  proxy: ProxyStatus;
  ca: CaInfo;
  settings: Settings;
  os: OsInfo;
  recording: boolean;
}

export interface Stats {
  flows: number;
  bytes: number;
  requestsTotal: number;
  recording: boolean;
}

export type TrustState = "trusted" | "untrusted" | "missing" | "optional" | "unknown";
export interface TrustEntry {
  id: string;
  name: string;
  state: TrustState;
  detail: string;
}

export interface SystemProxy {
  supported: boolean;
  backend: string;
  enabled: boolean;
  host: string;
  port: number;
}

export interface PortCheck {
  port: number;
  free: boolean;
  owner: string | null;
}

export const api = {
  appState: () => invoke<AppState>("app_state"),
  appReady: () => invoke<void>("app_ready"),
  setWindowTheme: (theme: "dark" | "light") => invoke<void>("set_window_theme", { theme }),
  logPath: () => invoke<string>("log_path"),
  stats: () => invoke<Stats>("stats"),
  listFlows: () => invoke<FlowSummary[]>("list_flows"),
  flowDetail: (id: number) => invoke<FlowDetail>("flow_detail", { id }),
  searchBodies: (needle: string) => invoke<number[]>("search_bodies", { needle }),
  saveBody: (id: number, response: boolean, path: string) => invoke<void>("save_body", { id, response, path }),
  setRecording: (on: boolean) => invoke<boolean>("set_recording", { on }),
  clearFlows: () => invoke<void>("clear_flows"),
  removeFlows: (ids: number[]) => invoke<void>("remove_flows", { ids }),
  replay: (id: number) => invoke<number>("replay", { id }),
  sendRequest: (method: string, url: string, headers: Headers, body: string) => invoke<number>("send_request", { method, url, headers, body }),
  getRules: () => invoke<Rules>("get_rules"),
  setRules: (rules: Rules) => invoke<void>("set_rules", { rules }),
  paused: () => invoke<Paused[]>("paused"),
  resume: (id: number, resume: Resume) => invoke<boolean>("resume", { id, resume }),
  resumeAll: () => invoke<void>("resume_all"),
  wsMessages: (id: number) => invoke<WsMessage[]>("ws_messages", { id }),
  importHar: (path: string) => invoke<number>("import_har", { path }),
  scriptLogs: (after: number) => invoke<ScriptLog[]>("script_logs", { after }),
  clearScriptLogs: () => invoke<void>("clear_script_logs"),
  checkScript: (code: string) => invoke<string[]>("check_script", { code }),
  resetScriptState: (id: string | null) => invoke<void>("reset_script_state", { id }),
  listSessions: () => invoke<SessionFile[]>("list_sessions"),
  saveSession: (name: string, ids?: number[]) => invoke<SessionFile>("save_session", { name, ids: ids ?? null }),
  deleteSession: (path: string) => invoke<void>("delete_session", { path }),
  exportHar: (path: string, ids?: number[]) => invoke<number>("export_har", { path, ids: ids ?? null }),
  restartProxy: (port: number, listenAll: boolean) => invoke<ProxyStatus>("restart_proxy", { port, listenAll }),
  setIntercept: (cfg: InterceptConfig) => invoke<void>("set_intercept", { cfg }),
  setPrefs: (patch: Partial<Settings>) => invoke<Settings>("set_prefs", { patch }),
  checkPort: (port: number) => invoke<PortCheck>("check_port", { port }),
  lanIp: () => invoke<string | null>("lan_ip"),
  caInfo: () => invoke<CaInfo>("ca_info"),
  exportCa: (format: "pem" | "der", path: string) => invoke<void>("export_ca", { format, path }),
  trustStatus: () => invoke<TrustEntry[]>("trust_status"),
  installCaSystem: () => invoke<string>("install_ca_system"),
  installNss: (firefox: boolean) => invoke<string>("install_nss", { firefox }),
  manualCommands: () => invoke<Record<string, string[]>>("manual_commands"),
  regenerateCa: () => invoke<CaInfo>("regenerate_ca"),
  verifyInterception: () => invoke<string>("verify_interception"),
  qrSvg: (text: string) => invoke<string>("qr_svg", { text }),
  systemProxy: () => invoke<SystemProxy>("system_proxy"),
  setSystemProxy: (on: boolean) => invoke<SystemProxy>("set_system_proxy", { on }),
};

export function on<T>(event: string, cb: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(event, (e) => cb(e.payload));
}

/** Tauri command errors arrive as plain strings. */
export function errMsg(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
