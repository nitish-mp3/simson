/* ──────────────────────────────────────────────────────────────────────────────
 * localize.ts — Localization helper for HA VoIP card
 * Embeds English strings and delegates to HA's localize when available.
 * ────────────────────────────────────────────────────────────────────────── */

import type { HomeAssistant } from "./types";

/** Flat key-value map of English UI strings */
const EN_STRINGS: Record<string, string> = {
  /* ── Card chrome ────────────────────────────────────────────────────── */
  "card.title": "VoIP Phone",
  "card.no_config": "No configuration provided",
  "card.loading": "Loading...",

  /* ── Call states ────────────────────────────────────────────────────── */
  "call.idle": "Idle",
  "call.ringing": "Ringing",
  "call.dialing": "Dialing",
  "call.connected": "Connected",
  "call.on_hold": "On Hold",
  "call.transferring": "Transferring",
  "call.ended": "Call Ended",
  "call.incoming": "Incoming Call",
  "call.duration": "Duration",
  "call.unknown_caller": "Unknown Caller",

  /* ── Dialpad ────────────────────────────────────────────────────────── */
  "dialpad.title": "Dialpad",
  "dialpad.placeholder": "Enter number...",
  "dialpad.call": "Call",
  "dialpad.hangup": "Hang Up",
  "dialpad.backspace": "Backspace",

  /* ── Controls ───────────────────────────────────────────────────────── */
  "controls.mute": "Mute",
  "controls.unmute": "Unmute",
  "controls.hold": "Hold",
  "controls.unhold": "Resume",
  "controls.hangup": "Hang Up",
  "controls.transfer": "Transfer",
  "controls.record": "Record",
  "controls.stop_record": "Stop Recording",
  "controls.speaker": "Speaker",
  "controls.keypad": "Keypad",
  "controls.accept": "Accept",
  "controls.reject": "Reject",
  "controls.audio_device": "Audio Device",

  /* ── Extensions ─────────────────────────────────────────────────────── */
  "ext.title": "Extensions",
  "ext.available": "Available",
  "ext.busy": "Busy",
  "ext.ringing": "Ringing",
  "ext.offline": "Offline",
  "ext.dnd": "Do Not Disturb",

  /* ── Quick dial ─────────────────────────────────────────────────────── */
  "quickdial.title": "Quick Dial",

  /* ── Call history ───────────────────────────────────────────────────── */
  "history.title": "Recent Calls",
  "history.no_calls": "No recent calls",
  "history.inbound": "Inbound",
  "history.outbound": "Outbound",
  "history.missed": "Missed",
  "history.today": "Today",
  "history.yesterday": "Yesterday",

  /* ── Popup ──────────────────────────────────────────────────────────── */
  "popup.incoming_call": "Incoming Call",
  "popup.active_call": "Active Call",
  "popup.camera_snapshot": "Doorbell Camera",

  /* ── Onboarding wizard ──────────────────────────────────────────────── */
  "onboarding.title": "VoIP Setup",
  "onboarding.skip": "Skip (use defaults)",
  "onboarding.back": "Back",
  "onboarding.next": "Next",
  "onboarding.finish": "Finish Setup",
  "onboarding.condensed": "Quick Setup",
  "onboarding.full": "Full Setup",

  "onboarding.step1.title": "Welcome to HA VoIP",
  "onboarding.step1.subtitle":
    "Let's set up voice calling for your smart home. First, we need microphone permission.",
  "onboarding.step1.request_mic": "Grant Microphone Access",
  "onboarding.step1.mic_granted": "Microphone access granted",
  "onboarding.step1.mic_denied":
    "Microphone access denied. Please allow it in your browser settings.",

  "onboarding.step2.title": "Network Test",
  "onboarding.step2.subtitle":
    "Checking your network for VoIP compatibility.",
  "onboarding.step2.running": "Running tests...",
  "onboarding.step2.complete": "Network tests complete",

  "onboarding.step3.title": "Mode Selection",
  "onboarding.step3.subtitle": "Choose how VoIP will operate.",
  "onboarding.step3.local": "Local Only",
  "onboarding.step3.local_desc":
    "Calls stay within your local network. Best for intercom and room-to-room calling.",
  "onboarding.step3.federated": "Federated",
  "onboarding.step3.federated_desc":
    "Connect to external SIP providers for PSTN calls. Requires port forwarding or a SIP trunk.",

  "onboarding.step4.title": "Extension Assignment",
  "onboarding.step4.subtitle":
    "Map Home Assistant users to extension numbers for internal calling.",
  "onboarding.step4.user": "User",
  "onboarding.step4.extension": "Extension",
  "onboarding.step4.add": "Add Extension",

  "onboarding.step5.title": "Certificate Setup",
  "onboarding.step5.subtitle":
    "WebRTC requires secure connections. Choose a certificate option.",
  "onboarding.step5.auto": "Automatic (Let's Encrypt)",
  "onboarding.step5.auto_desc":
    "Automatically obtain and renew certificates via Let's Encrypt.",
  "onboarding.step5.manual": "Manual",
  "onboarding.step5.manual_desc":
    "Provide your own certificate and key files.",
  "onboarding.step5.self_signed": "Self-Signed",
  "onboarding.step5.self_signed_desc":
    "Generate a self-signed certificate. Not recommended for production.",

  "onboarding.step6.title": "Test Call",
  "onboarding.step6.subtitle":
    "Make a loopback test call to verify everything works.",
  "onboarding.step6.start": "Start Test Call",
  "onboarding.step6.testing": "Testing...",
  "onboarding.step6.success": "Test call succeeded! Everything is working.",
  "onboarding.step6.failure":
    "Test call failed. Check diagnostics for details.",

  /* ── Diagnostics ────────────────────────────────────────────────────── */
  "diag.title": "Network Diagnostics",
  "diag.run_all": "Run All Tests",
  "diag.export": "Export as JSON",
  "diag.wss": "WebSocket (WSS)",
  "diag.stun": "STUN Server",
  "diag.turn": "TURN Server",
  "diag.rtt": "Network RTT",
  "diag.one_way_audio": "One-Way Audio Test",
  "diag.ice_candidates": "ICE Candidates",
  "diag.pass": "Pass",
  "diag.fail": "Fail",
  "diag.warning": "Warning",
  "diag.pending": "Pending",
  "diag.running": "Running",

  /* ── Config editor ──────────────────────────────────────────────────── */
  "config.title": "VoIP Card Configuration",
  "config.card_title": "Card Title",
  "config.entity": "VoIP Entity",
  "config.show_recent": "Show Recent Calls",
  "config.recent_count": "Number of Recent Calls",
  "config.show_dialpad": "Show Dialpad",
  "config.show_diagnostics": "Show Diagnostics Button",
  "config.compact_mode": "Compact Mode",
  "config.enable_dtmf": "Enable DTMF Tones",
  "config.auto_answer": "Auto-Answer Calls",
  "config.ringtone": "Ringtone URL",
  "config.quick_dial": "Quick Dial Entries",
  "config.add_quick_dial": "Add Quick Dial",
  "config.name": "Name",
  "config.number": "Number",
  "config.icon": "Icon",
  "config.remove": "Remove",
};

/**
 * Look up a translation string.
 *
 * Priority order:
 *  1. HA's built-in localize (for standard HA keys)
 *  2. Embedded English strings (for VoIP-specific keys)
 *  3. The raw key itself (fallback)
 *
 * Supports simple {0}, {1}, ... positional placeholders.
 */
export function localize(
  key: string,
  hass?: HomeAssistant,
  ...args: (string | number)[]
): string {
  let str: string | undefined;

  // Try HA localize first for common HA keys
  if (hass?.localize) {
    const haResult = hass.localize(key);
    if (haResult && haResult !== key) {
      str = haResult;
    }
  }

  // Fall back to built-in strings
  if (!str) {
    str = EN_STRINGS[key];
  }

  // Last resort: return the key
  if (!str) {
    return key;
  }

  // Replace positional placeholders
  if (args.length > 0) {
    args.forEach((arg, i) => {
      str = str!.replace(`{${i}}`, String(arg));
    });
  }

  return str;
}

/**
 * Shorthand factory: bind localize to a specific hass instance.
 * Useful inside LitElement components.
 *
 * @example
 *   private _t = createLocalizer(() => this.hass);
 *   render() { return html`<span>${this._t("call.idle")}</span>`; }
 */
export function createLocalizer(
  hassGetter: () => HomeAssistant | undefined,
): (key: string, ...args: (string | number)[]) => string {
  return (key: string, ...args: (string | number)[]) =>
    localize(key, hassGetter(), ...args);
}

/**
 * Format a duration in seconds to mm:ss or hh:mm:ss.
 */
export function formatDuration(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  const mm = String(m).padStart(2, "0");
  const ss = String(s).padStart(2, "0");
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}

/**
 * Format a UNIX timestamp to a human-readable relative time string.
 */
export function formatRelativeTime(timestamp: number): string {
  const now = Date.now();
  const diff = now - timestamp;
  const seconds = Math.floor(diff / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);

  if (days > 1) {
    return new Date(timestamp).toLocaleDateString();
  }
  if (days === 1) {
    return EN_STRINGS["history.yesterday"];
  }
  if (hours > 0) {
    return `${hours}h ago`;
  }
  if (minutes > 0) {
    return `${minutes}m ago`;
  }
  return "Just now";
}
