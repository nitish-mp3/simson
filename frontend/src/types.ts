/* ──────────────────────────────────────────────────────────────────────────────
 * types.ts — Type definitions for the HA VoIP card
 * ────────────────────────────────────────────────────────────────────────── */

// ── Home Assistant core types (minimal subset we depend on) ─────────────────

export interface HomeAssistant {
  callWS: <T>(msg: Record<string, unknown>) => Promise<T>;
  callService: (
    domain: string,
    service: string,
    data?: Record<string, unknown>,
  ) => Promise<void>;
  connection: {
    subscribeMessage: <T>(
      callback: (msg: T) => void,
      subscribeMsg: Record<string, unknown>,
    ) => Promise<() => void>;
    sendMessage: (msg: Record<string, unknown>) => void;
    sendMessagePromise: <T>(msg: Record<string, unknown>) => Promise<T>;
    socket: WebSocket;
  };
  states: Record<string, HassEntity>;
  themes: {
    darkMode: boolean;
    theme: string;
  };
  language: string;
  user: {
    id: string;
    name: string;
    is_admin: boolean;
  };
  config: {
    components: string[];
  };
  localize: (key: string, ...args: string[]) => string;
}

export interface HassEntity {
  entity_id: string;
  state: string;
  attributes: Record<string, unknown>;
  last_changed: string;
  last_updated: string;
}

export interface LovelaceCard extends HTMLElement {
  hass?: HomeAssistant;
  setConfig(config: LovelaceCardConfig): void;
  getCardSize(): number;
}

export interface LovelaceCardConfig {
  type: string;
  [key: string]: unknown;
}

export interface LovelaceCardEditor extends HTMLElement {
  hass?: HomeAssistant;
  setConfig(config: LovelaceCardConfig): void;
}

// ── VoIP card configuration ─────────────────────────────────────────────────

export interface VoipCardConfig extends LovelaceCardConfig {
  type: string;
  title?: string;
  entity?: string;
  extensions?: ExtensionConfig[];
  quick_dial?: QuickDialEntry[];
  show_recent_calls?: boolean;
  recent_calls_count?: number;
  show_dialpad?: boolean;
  show_diagnostics?: boolean;
  compact_mode?: boolean;
  theme_override?: "light" | "dark" | "auto";
  ringtone_url?: string;
  enable_dtmf_tones?: boolean;
  auto_answer?: boolean;
  stun_servers?: string[];
  turn_servers?: TurnServerConfig[];
}

export interface ExtensionConfig {
  user_id?: string;
  name: string;
  number: string;
}

export interface QuickDialEntry {
  name: string;
  number: string;
  icon?: string;
}

export interface TurnServerConfig {
  urls: string | string[];
  username?: string;
  credential?: string;
}

// ── Call state types ────────────────────────────────────────────────────────

export type CallDirection = "inbound" | "outbound";

export type CallStateValue =
  | "idle"
  | "ringing"
  | "dialing"
  | "connected"
  | "on_hold"
  | "transferring"
  | "ended";

export interface CallState {
  id: string;
  state: CallStateValue;
  direction: CallDirection;
  remoteNumber: string;
  remoteName?: string;
  startTime?: number;
  connectTime?: number;
  endTime?: number;
  isMuted: boolean;
  isOnHold: boolean;
  isRecording: boolean;
  isSpeaker: boolean;
  duration: number;
}

// ── Extension types ─────────────────────────────────────────────────────────

export type ExtensionStatus = "available" | "busy" | "ringing" | "offline" | "dnd";

export interface Extension {
  id: string;
  number: string;
  name: string;
  status: ExtensionStatus;
  userId?: string;
  registeredAt?: string;
  callState?: CallStateValue;
}

// ── Call history ────────────────────────────────────────────────────────────

export interface CallHistoryEntry {
  id: string;
  direction: CallDirection;
  remoteNumber: string;
  remoteName?: string;
  startTime: number;
  endTime: number;
  duration: number;
  answered: boolean;
  recorded: boolean;
}

// ── WebSocket message types ─────────────────────────────────────────────────

export type WsMessageType =
  | "voip/subscribe"
  | "voip/call"
  | "voip/answer"
  | "voip/hangup"
  | "voip/hold"
  | "voip/transfer"
  | "voip/mute"
  | "voip/dtmf"
  | "voip/record"
  | "voip/webrtc_offer"
  | "voip/webrtc_answer"
  | "voip/webrtc_candidate"
  | "voip/diagnostics"
  | "voip/extensions"
  | "voip/history"
  | "voip/onboarding";

export interface WsBaseMessage {
  type: WsMessageType;
  id?: number;
}

export interface WsSubscribeMessage extends WsBaseMessage {
  type: "voip/subscribe";
}

export interface WsCallMessage extends WsBaseMessage {
  type: "voip/call";
  number: string;
}

export interface WsAnswerMessage extends WsBaseMessage {
  type: "voip/answer";
  call_id: string;
}

export interface WsHangupMessage extends WsBaseMessage {
  type: "voip/hangup";
  call_id: string;
}

export interface WsHoldMessage extends WsBaseMessage {
  type: "voip/hold";
  call_id: string;
  hold: boolean;
}

export interface WsTransferMessage extends WsBaseMessage {
  type: "voip/transfer";
  call_id: string;
  target: string;
}

export interface WsMuteMessage extends WsBaseMessage {
  type: "voip/mute";
  call_id: string;
  mute: boolean;
}

export interface WsDtmfMessage extends WsBaseMessage {
  type: "voip/dtmf";
  call_id: string;
  digit: string;
}

export interface WsRecordMessage extends WsBaseMessage {
  type: "voip/record";
  call_id: string;
  record: boolean;
}

export interface WsWebRtcOfferMessage extends WsBaseMessage {
  type: "voip/webrtc_offer";
  call_id: string;
  sdp: string;
}

export interface WsWebRtcAnswerMessage extends WsBaseMessage {
  type: "voip/webrtc_answer";
  call_id: string;
  sdp: string;
}

export interface WsWebRtcCandidateMessage extends WsBaseMessage {
  type: "voip/webrtc_candidate";
  call_id: string;
  candidate: RTCIceCandidateInit;
}

export type WsOutgoingMessage =
  | WsSubscribeMessage
  | WsCallMessage
  | WsAnswerMessage
  | WsHangupMessage
  | WsHoldMessage
  | WsTransferMessage
  | WsMuteMessage
  | WsDtmfMessage
  | WsRecordMessage
  | WsWebRtcOfferMessage
  | WsWebRtcAnswerMessage
  | WsWebRtcCandidateMessage;

// ── Incoming event types ────────────────────────────────────────────────────

export interface VoipEventCallState {
  event: "call_state";
  data: CallState;
}

export interface VoipEventExtensions {
  event: "extensions";
  data: Extension[];
}

export interface VoipEventHistory {
  event: "history";
  data: CallHistoryEntry[];
}

export interface VoipEventWebRtcOffer {
  event: "webrtc_offer";
  call_id: string;
  sdp: string;
}

export interface VoipEventWebRtcAnswer {
  event: "webrtc_answer";
  call_id: string;
  sdp: string;
}

export interface VoipEventWebRtcCandidate {
  event: "webrtc_candidate";
  call_id: string;
  candidate: RTCIceCandidateInit;
}

export interface VoipEventIncomingCall {
  event: "incoming_call";
  data: {
    call_id: string;
    caller_number: string;
    caller_name?: string;
    camera_entity_id?: string;
  };
}

export type VoipEvent =
  | VoipEventCallState
  | VoipEventExtensions
  | VoipEventHistory
  | VoipEventWebRtcOffer
  | VoipEventWebRtcAnswer
  | VoipEventWebRtcCandidate
  | VoipEventIncomingCall;

// ── Diagnostics types ───────────────────────────────────────────────────────

export type DiagnosticTestStatus = "pending" | "running" | "pass" | "fail" | "warning";

export interface DiagnosticResult {
  name: string;
  status: DiagnosticTestStatus;
  message?: string;
  details?: string;
  durationMs?: number;
}

export interface IceCandidateInfo {
  type: RTCIceCandidateType;
  protocol: string;
  address: string;
  port: number;
  priority: number;
  relatedAddress?: string;
  relatedPort?: number;
}

export interface DiagnosticsReport {
  timestamp: number;
  userAgent: string;
  results: DiagnosticResult[];
  iceCandidates: IceCandidateInfo[];
  networkRtt?: number;
}

// ── Onboarding types ────────────────────────────────────────────────────────

export type OnboardingStep =
  | "welcome"
  | "network_test"
  | "mode_selection"
  | "extension_assignment"
  | "certificate_setup"
  | "test_call";

export type VoipMode = "local" | "federated";

export type CertificateMode = "auto" | "manual" | "self_signed";

export interface OnboardingConfig {
  mode: VoipMode;
  extensions: ExtensionConfig[];
  certificateMode: CertificateMode;
  certificatePath?: string;
  stunServers: string[];
  turnServers: TurnServerConfig[];
  completed: boolean;
}

// ── WebRTC manager types ────────────────────────────────────────────────────

export type WebRtcConnectionState =
  | "new"
  | "connecting"
  | "connected"
  | "disconnected"
  | "failed"
  | "closed";

export interface WebRtcStats {
  bytesReceived: number;
  bytesSent: number;
  packetsReceived: number;
  packetsSent: number;
  packetsLost: number;
  jitter: number;
  roundTripTime: number;
  audioLevel: number;
  timestamp: number;
}

export interface AudioDeviceInfo {
  deviceId: string;
  label: string;
  kind: "audioinput" | "audiooutput";
}

// ── Utility types ───────────────────────────────────────────────────────────

export interface FireEventDetail {
  [key: string]: unknown;
}

/** Helper to fire DOM custom events on HA elements */
export function fireEvent(
  node: HTMLElement,
  type: string,
  detail?: FireEventDetail,
  options?: { bubbles?: boolean; composed?: boolean; cancelable?: boolean },
): void {
  const event = new CustomEvent(type, {
    bubbles: options?.bubbles ?? true,
    composed: options?.composed ?? true,
    cancelable: options?.cancelable ?? false,
    detail: detail ?? {},
  });
  node.dispatchEvent(event);
}
