/* ──────────────────────────────────────────────────────────────────────────────
 * dialpad.ts — Numeric dialpad component for HA VoIP card
 *
 * Features:
 *   - 0-9, *, # keys with telephone letter labels
 *   - Input display with backspace
 *   - Call (green) and hangup (red) action buttons
 *   - DTMF tone generation via Web Audio API
 *   - Full keyboard support for accessibility
 *   - Touch-friendly sizing
 * ────────────────────────────────────────────────────────────────────────── */

import { LitElement, html, nothing, type PropertyValues } from "lit";
import { customElement, property, state, query } from "lit/decorators.js";
import {
  cardStyles,
  buttonStyles,
  dialpadStyles,
  responsiveStyles,
} from "./styles";
import { localize } from "./localize";
import type { HomeAssistant, CallStateValue } from "./types";
import { fireEvent } from "./types";

/* ── DTMF frequency pairs (ITU-T Q.23) ──────────────────────────────────── */
const DTMF_FREQUENCIES: Record<string, [number, number]> = {
  "1": [697, 1209],
  "2": [697, 1336],
  "3": [697, 1477],
  "4": [770, 1209],
  "5": [770, 1336],
  "6": [770, 1477],
  "7": [852, 1209],
  "8": [852, 1336],
  "9": [852, 1477],
  "*": [941, 1209],
  "0": [941, 1336],
  "#": [941, 1477],
};

/** Sub-letter labels beneath each digit (standard telephone mapping) */
const KEY_LETTERS: Record<string, string> = {
  "1": "",
  "2": "ABC",
  "3": "DEF",
  "4": "GHI",
  "5": "JKL",
  "6": "MNO",
  "7": "PQRS",
  "8": "TUV",
  "9": "WXYZ",
  "*": "",
  "0": "+",
  "#": "",
};

const KEY_ORDER = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "*", "0", "#"];

@customElement("ha-voip-dialpad")
export class HaVoipDialpad extends LitElement {
  /* ── Properties ────────────────────────────────────────────────────────── */

  @property({ attribute: false }) hass?: HomeAssistant;

  /** Current call state — controls which action buttons are visible */
  @property({ type: String, reflect: true }) callState: CallStateValue = "idle";

  /** Whether DTMF tones should play on key press */
  @property({ type: Boolean }) enableDtmf = true;

  /** The dialled number string */
  @state() private _number = "";

  @query("#dial-input") private _inputEl!: HTMLInputElement;

  /* ── Audio context for DTMF ────────────────────────────────────────────── */
  private _audioCtx: AudioContext | null = null;

  /* ── Styles ────────────────────────────────────────────────────────────── */
  static styles = [cardStyles, buttonStyles, dialpadStyles, responsiveStyles];

  /* ── Lifecycle ─────────────────────────────────────────────────────────── */

  connectedCallback(): void {
    super.connectedCallback();
    this.addEventListener("keydown", this._handleKeyboard);
  }

  disconnectedCallback(): void {
    super.disconnectedCallback();
    this.removeEventListener("keydown", this._handleKeyboard);
    if (this._audioCtx) {
      this._audioCtx.close();
      this._audioCtx = null;
    }
  }

  /* ── Render ────────────────────────────────────────────────────────────── */

  protected render() {
    const isInCall =
      this.callState === "connected" ||
      this.callState === "dialing" ||
      this.callState === "on_hold";

    return html`
      <!-- Number display -->
      <div class="dialpad-display" role="textbox" aria-label="${localize("dialpad.placeholder", this.hass)}">
        <input
          id="dial-input"
          class="dialpad-display__input"
          type="tel"
          .value=${this._number}
          placeholder=${localize("dialpad.placeholder", this.hass)}
          @input=${this._handleInput}
          aria-label="${localize("dialpad.placeholder", this.hass)}"
        />
        ${this._number
          ? html`
              <button
                class="btn btn--icon btn--sm dialpad-display__backspace"
                @click=${this._handleBackspace}
                aria-label=${localize("dialpad.backspace", this.hass)}
              >
                <svg viewBox="0 0 24 24" width="20" height="20">
                  <path fill="currentColor" d="M22,3H7C6.31,3 5.77,3.35 5.41,3.88L0,12L5.41,20.11C5.77,20.64 6.31,21 7,21H22A2,2 0 0,0 24,19V5A2,2 0 0,0 22,3M19,15.59L17.59,17L14,13.41L10.41,17L9,15.59L12.59,12L9,8.41L10.41,7L14,10.59L17.59,7L19,8.41L15.41,12" />
                </svg>
              </button>
            `
          : nothing}
      </div>

      <!-- Key grid -->
      <div class="dialpad-grid" role="group" aria-label="${localize("dialpad.title", this.hass)}">
        ${KEY_ORDER.map(
          (key) => html`
            <button
              class="dialpad-key"
              data-key=${key}
              @click=${() => this._pressKey(key)}
              @touchstart=${(e: TouchEvent) => { e.preventDefault(); this._pressKey(key); }}
              aria-label="${key} ${KEY_LETTERS[key] || ""}"
            >
              <span class="dialpad-key__digit">${key}</span>
              ${KEY_LETTERS[key]
                ? html`<span class="dialpad-key__letters">${KEY_LETTERS[key]}</span>`
                : nothing}
            </button>
          `,
        )}
      </div>

      <!-- Action buttons -->
      <div class="dialpad-actions">
        ${isInCall
          ? html`
              <button
                class="btn btn--lg btn--hangup"
                @click=${this._handleHangup}
                aria-label=${localize("controls.hangup", this.hass)}
              >
                <svg viewBox="0 0 24 24" width="28" height="28">
                  <path fill="currentColor" d="M12,9C10.4,9 8.85,9.25 7.4,9.72V12.82C7.4,13.22 7.17,13.56 6.84,13.72C5.86,14.21 4.97,14.84 4.17,15.57C4,15.75 3.75,15.86 3.5,15.86C3.2,15.86 2.95,15.74 2.77,15.56L0.29,13.08C0.11,12.9 0,12.65 0,12.38C0,12.1 0.11,11.85 0.29,11.67C3.34,8.77 7.46,7 12,7C16.54,7 20.66,8.77 23.71,11.67C23.89,11.85 24,12.1 24,12.38C24,12.65 23.89,12.9 23.71,13.08L21.23,15.56C21.05,15.74 20.8,15.86 20.5,15.86C20.25,15.86 20,15.75 19.83,15.57C19.03,14.84 18.14,14.21 17.16,13.72C16.83,13.56 16.6,13.22 16.6,12.82V9.72C15.15,9.25 13.6,9 12,9Z" />
                </svg>
              </button>
            `
          : html`
              <button
                class="btn btn--lg btn--call"
                @click=${this._handleCall}
                ?disabled=${!this._number}
                aria-label=${localize("dialpad.call", this.hass)}
              >
                <svg viewBox="0 0 24 24" width="28" height="28">
                  <path fill="currentColor" d="M6.62,10.79C8.06,13.62 10.38,15.94 13.21,17.38L15.41,15.18C15.69,14.9 16.08,14.82 16.43,14.93C17.55,15.3 18.75,15.5 20,15.5A1,1 0 0,1 21,16.5V20A1,1 0 0,1 20,21A17,17 0 0,1 3,4A1,1 0 0,1 4,3H7.5A1,1 0 0,1 8.5,4C8.5,5.25 8.7,6.45 9.07,7.57C9.18,7.92 9.1,8.31 8.82,8.59L6.62,10.79Z" />
                </svg>
              </button>
            `}
      </div>
    `;
  }

  /* ── Key handling ──────────────────────────────────────────────────────── */

  private _pressKey(key: string): void {
    this._number += key;
    if (this.enableDtmf) {
      this._playDtmf(key);
    }

    // If in a call, emit DTMF event for the WebRTC manager
    if (this.callState === "connected") {
      fireEvent(this, "voip-dtmf", { digit: key });
    }
  }

  private _handleBackspace(): void {
    this._number = this._number.slice(0, -1);
  }

  private _handleInput(e: InputEvent): void {
    const input = e.target as HTMLInputElement;
    // Filter to valid dialpad characters
    this._number = input.value.replace(/[^0-9*#]/g, "");
  }

  private _handleCall(): void {
    if (!this._number) return;
    fireEvent(this, "voip-call", { number: this._number });
  }

  private _handleHangup(): void {
    fireEvent(this, "voip-hangup");
  }

  /** Map physical keyboard keys to dialpad actions */
  private _handleKeyboard = (e: KeyboardEvent): void => {
    const key = e.key;
    if (DTMF_FREQUENCIES[key]) {
      e.preventDefault();
      this._pressKey(key);
    } else if (key === "Backspace") {
      this._handleBackspace();
    } else if (key === "Enter" && this._number) {
      if (this.callState === "idle") {
        this._handleCall();
      }
    } else if (key === "Escape") {
      this._handleHangup();
    }
  };

  /* ── DTMF tone generation ──────────────────────────────────────────────── */

  private _playDtmf(key: string): void {
    const freqs = DTMF_FREQUENCIES[key];
    if (!freqs) return;

    try {
      if (!this._audioCtx) {
        this._audioCtx = new AudioContext();
      }

      const duration = 0.15;
      const now = this._audioCtx.currentTime;

      // Create two oscillators at the dual-tone frequencies
      const osc1 = this._audioCtx.createOscillator();
      const osc2 = this._audioCtx.createOscillator();
      const gain = this._audioCtx.createGain();

      osc1.frequency.value = freqs[0];
      osc2.frequency.value = freqs[1];
      osc1.type = "sine";
      osc2.type = "sine";

      gain.gain.setValueAtTime(0.15, now);
      gain.gain.exponentialRampToValueAtTime(0.001, now + duration);

      osc1.connect(gain);
      osc2.connect(gain);
      gain.connect(this._audioCtx.destination);

      osc1.start(now);
      osc2.start(now);
      osc1.stop(now + duration);
      osc2.stop(now + duration);
    } catch {
      // Audio context may not be available — that's fine
    }
  }

  /* ── Public API ────────────────────────────────────────────────────────── */

  /** Clear the dialled number */
  public clear(): void {
    this._number = "";
  }

  /** Get the currently dialled number */
  public get number(): string {
    return this._number;
  }

  /** Programmatically set the number */
  public set number(value: string) {
    this._number = value.replace(/[^0-9*#]/g, "");
  }

  /* ── Extra styles (scoped to this component) ───────────────────────────── */

  static get additionalStyles() {
    return [
      /* already included via static styles */
    ];
  }

  protected createRenderRoot() {
    // We need shadow DOM for style encapsulation
    return super.createRenderRoot();
  }

  protected updated(changed: PropertyValues): void {
    super.updated(changed);

    // Keep the native input in sync after renders
    if (this._inputEl && this._inputEl.value !== this._number) {
      this._inputEl.value = this._number;
    }
  }
}

/* ── Lit extra styles injected via static styles ─────────────────────────── */
// The styles below are appended to the static styles array via a patched getter
// so they coexist with the shared imports.
const extraStyles = document.createElement("style");
extraStyles.textContent = `
  ha-voip-dialpad .dialpad-display {
    display: flex;
    align-items: center;
    padding: 8px 16px;
    gap: 8px;
  }

  ha-voip-dialpad .dialpad-display__input {
    flex: 1;
    border: none;
    outline: none;
    font-size: 24px;
    font-weight: 500;
    text-align: center;
    background: transparent;
    color: var(--voip-primary-text, #212121);
    font-family: inherit;
    letter-spacing: 2px;
  }

  ha-voip-dialpad .dialpad-display__input::placeholder {
    font-size: 14px;
    letter-spacing: normal;
    color: var(--voip-disabled, #bdbdbd);
  }

  ha-voip-dialpad .dialpad-actions {
    display: flex;
    justify-content: center;
    padding: 12px 16px 16px;
  }
`;

// We add this *before* the element upgrades on first use
if (!document.querySelector("style[data-voip-dialpad]")) {
  extraStyles.setAttribute("data-voip-dialpad", "");
  document.head.appendChild(extraStyles);
}

declare global {
  interface HTMLElementTagNameMap {
    "ha-voip-dialpad": HaVoipDialpad;
  }
}
