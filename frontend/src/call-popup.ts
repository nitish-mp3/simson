/* ──────────────────────────────────────────────────────────────────────────────
 * call-popup.ts — Incoming / active call popup for HA VoIP
 *
 * Features:
 *   - Incoming call notification with accept / reject
 *   - Caller ID display
 *   - Call timer
 *   - In-call controls: mute, speaker, hold, transfer, record, keypad
 *   - Camera doorbell snapshot integration
 *   - Drag-to-dismiss on mobile
 *   - Audio device selection dropdown
 * ────────────────────────────────────────────────────────────────────────── */

import { LitElement, html, css, nothing, type PropertyValues } from "lit";
import { customElement, property, state, query } from "lit/decorators.js";
import {
  cardStyles,
  buttonStyles,
  callControlStyles,
  popupStyles,
  responsiveStyles,
} from "./styles";
import { localize, formatDuration } from "./localize";
import type {
  HomeAssistant,
  CallState,
  AudioDeviceInfo,
} from "./types";
import { fireEvent } from "./types";

@customElement("ha-voip-call-popup")
export class HaVoipCallPopup extends LitElement {
  /* ── Properties ────────────────────────────────────────────────────────── */

  @property({ attribute: false }) hass?: HomeAssistant;
  @property({ attribute: false }) callState?: CallState;
  @property({ type: String }) cameraEntityId?: string;

  @state() private _elapsed = 0;
  @state() private _showKeypad = false;
  @state() private _showDeviceMenu = false;
  @state() private _audioDevices: AudioDeviceInfo[] = [];
  @state() private _dragOffsetY = 0;
  @state() private _isDragging = false;
  @state() private _cameraUrl: string | null = null;

  @query(".popup-card") private _card!: HTMLElement;

  private _timerInterval: ReturnType<typeof setInterval> | null = null;
  private _touchStartY = 0;

  /* ── Styles ────────────────────────────────────────────────────────────── */

  static styles = [
    cardStyles,
    buttonStyles,
    callControlStyles,
    popupStyles,
    responsiveStyles,
    css`
      .caller-info {
        text-align: center;
        padding: 24px 20px 8px;
      }

      .caller-avatar {
        width: 72px;
        height: 72px;
        border-radius: 50%;
        background-color: var(--voip-primary);
        color: #fff;
        display: flex;
        align-items: center;
        justify-content: center;
        font-size: 28px;
        font-weight: 500;
        margin: 0 auto 12px;
      }

      .caller-name {
        font-size: 22px;
        font-weight: 500;
        margin: 0 0 4px;
      }

      .caller-number {
        font-size: 14px;
        color: var(--voip-secondary-text);
        margin: 0 0 8px;
      }

      .call-status {
        font-size: 13px;
        font-weight: 500;
        margin: 0;
      }

      .call-status--ringing {
        color: var(--voip-warning);
        animation: pulse 1.5s infinite;
      }

      .call-status--connected {
        color: var(--voip-success);
      }

      .call-status--on_hold {
        color: var(--voip-info);
      }

      .call-timer {
        font-size: 32px;
        font-weight: 300;
        text-align: center;
        padding: 12px 0;
        font-variant-numeric: tabular-nums;
        letter-spacing: 2px;
      }

      .incoming-actions {
        display: flex;
        justify-content: center;
        gap: 48px;
        padding: 24px 20px 32px;
      }

      .incoming-action-label {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 8px;
        font-size: 13px;
        font-weight: 500;
      }

      .incoming-action-label--accept {
        color: var(--voip-success);
      }

      .incoming-action-label--reject {
        color: var(--voip-error);
      }

      .camera-snapshot {
        margin: 8px 16px;
        border-radius: 8px;
        overflow: hidden;
        background-color: #000;
        aspect-ratio: 16 / 9;
      }

      .camera-snapshot img {
        width: 100%;
        height: 100%;
        object-fit: contain;
        display: block;
      }

      .device-menu {
        position: absolute;
        bottom: 100%;
        left: 50%;
        transform: translateX(-50%);
        background-color: var(--voip-surface);
        border-radius: 8px;
        box-shadow: 0 4px 16px rgba(0, 0, 0, 0.2);
        min-width: 220px;
        max-height: 200px;
        overflow-y: auto;
        z-index: 10;
      }

      .device-menu__item {
        display: block;
        width: 100%;
        padding: 10px 16px;
        border: none;
        background: none;
        text-align: left;
        font-size: 13px;
        font-family: inherit;
        color: var(--voip-primary-text);
        cursor: pointer;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
      }

      .device-menu__item:hover {
        background-color: rgba(0, 0, 0, 0.06);
      }

      .device-menu__item--active {
        color: var(--voip-primary);
        font-weight: 500;
      }

      .keypad-inline {
        padding: 0 16px 8px;
      }

      .drag-handle {
        width: 40px;
        height: 4px;
        border-radius: 2px;
        background-color: var(--voip-divider);
        margin: 8px auto 0;
      }

      @media (max-width: 600px) {
        .popup-card {
          position: fixed;
          bottom: 0;
          left: 0;
          right: 0;
          max-width: 100%;
          width: 100%;
          border-radius: 16px 16px 0 0;
          max-height: 95vh;
          animation: slideUpMobile 0.3s ease;
        }

        @keyframes slideUpMobile {
          from {
            transform: translateY(100%);
          }
          to {
            transform: translateY(0);
          }
        }

        .incoming-actions {
          padding-bottom: calc(32px + env(safe-area-inset-bottom, 0px));
        }
      }
    `,
  ];

  /* ── Lifecycle ─────────────────────────────────────────────────────────── */

  connectedCallback(): void {
    super.connectedCallback();
    this._loadAudioDevices();
  }

  disconnectedCallback(): void {
    super.disconnectedCallback();
    this._stopTimer();
  }

  protected updated(changed: PropertyValues): void {
    super.updated(changed);

    if (changed.has("callState")) {
      this._onCallStateChanged();
    }

    if (changed.has("cameraEntityId") && this.cameraEntityId) {
      this._loadCameraSnapshot();
    }
  }

  /* ── Render ────────────────────────────────────────────────────────────── */

  protected render() {
    if (!this.callState) return nothing;

    const isIncoming =
      this.callState.state === "ringing" &&
      this.callState.direction === "inbound";
    const isActive =
      this.callState.state === "connected" ||
      this.callState.state === "on_hold";

    return html`
      <div
        class="popup-overlay"
        @click=${this._handleOverlayClick}
        role="dialog"
        aria-label=${isIncoming
          ? localize("popup.incoming_call", this.hass)
          : localize("popup.active_call", this.hass)}
      >
        <div
          class="popup-card"
          style=${this._isDragging
            ? `transform: translateY(${this._dragOffsetY}px)`
            : ""}
          @click=${(e: Event) => e.stopPropagation()}
          @touchstart=${this._handleTouchStart}
          @touchmove=${this._handleTouchMove}
          @touchend=${this._handleTouchEnd}
        >
          <!-- Drag handle (mobile) -->
          <div class="drag-handle"></div>

          <!-- Caller info -->
          ${this._renderCallerInfo()}

          <!-- Camera snapshot (doorbell integration) -->
          ${this._renderCameraSnapshot()}

          <!-- Call timer (active calls) -->
          ${isActive ? this._renderTimer() : nothing}

          <!-- Incoming call actions -->
          ${isIncoming ? this._renderIncomingActions() : nothing}

          <!-- Active call controls -->
          ${isActive ? this._renderActiveControls() : nothing}

          <!-- Inline keypad -->
          ${this._showKeypad && isActive ? this._renderInlineKeypad() : nothing}
        </div>
      </div>
    `;
  }

  private _renderCallerInfo() {
    const cs = this.callState!;
    const name = cs.remoteName || localize("call.unknown_caller", this.hass);
    const initials = name
      .split(" ")
      .map((w) => w[0])
      .join("")
      .slice(0, 2)
      .toUpperCase();

    let statusClass = "";
    let statusText = "";
    switch (cs.state) {
      case "ringing":
        statusClass = "call-status--ringing";
        statusText =
          cs.direction === "inbound"
            ? localize("call.incoming", this.hass)
            : localize("call.ringing", this.hass);
        break;
      case "dialing":
        statusClass = "call-status--ringing";
        statusText = localize("call.dialing", this.hass);
        break;
      case "connected":
        statusClass = "call-status--connected";
        statusText = localize("call.connected", this.hass);
        break;
      case "on_hold":
        statusClass = "call-status--on_hold";
        statusText = localize("call.on_hold", this.hass);
        break;
      default:
        statusText = localize(`call.${cs.state}`, this.hass);
    }

    return html`
      <div class="caller-info">
        <div class="caller-avatar" aria-hidden="true">${initials}</div>
        <p class="caller-name">${name}</p>
        <p class="caller-number">${cs.remoteNumber}</p>
        <p class="call-status ${statusClass}">${statusText}</p>
      </div>
    `;
  }

  private _renderCameraSnapshot() {
    if (!this._cameraUrl) return nothing;

    return html`
      <div class="camera-snapshot">
        <img
          src=${this._cameraUrl}
          alt=${localize("popup.camera_snapshot", this.hass)}
          loading="lazy"
        />
      </div>
    `;
  }

  private _renderTimer() {
    return html`
      <div class="call-timer" role="timer" aria-live="polite">
        ${formatDuration(this._elapsed)}
      </div>
    `;
  }

  private _renderIncomingActions() {
    return html`
      <div class="incoming-actions">
        <div class="incoming-action-label incoming-action-label--reject">
          <button
            class="btn btn--lg btn--hangup"
            @click=${this._handleReject}
            aria-label=${localize("controls.reject", this.hass)}
          >
            <svg viewBox="0 0 24 24" width="28" height="28">
              <path fill="currentColor" d="M12,9C10.4,9 8.85,9.25 7.4,9.72V12.82C7.4,13.22 7.17,13.56 6.84,13.72C5.86,14.21 4.97,14.84 4.17,15.57C4,15.75 3.75,15.86 3.5,15.86C3.2,15.86 2.95,15.74 2.77,15.56L0.29,13.08C0.11,12.9 0,12.65 0,12.38C0,12.1 0.11,11.85 0.29,11.67C3.34,8.77 7.46,7 12,7C16.54,7 20.66,8.77 23.71,11.67C23.89,11.85 24,12.1 24,12.38C24,12.65 23.89,12.9 23.71,13.08L21.23,15.56C21.05,15.74 20.8,15.86 20.5,15.86C20.25,15.86 20,15.75 19.83,15.57C19.03,14.84 18.14,14.21 17.16,13.72C16.83,13.56 16.6,13.22 16.6,12.82V9.72C15.15,9.25 13.6,9 12,9Z" />
            </svg>
          </button>
          <span>${localize("controls.reject", this.hass)}</span>
        </div>
        <div class="incoming-action-label incoming-action-label--accept">
          <button
            class="btn btn--lg btn--call"
            @click=${this._handleAccept}
            aria-label=${localize("controls.accept", this.hass)}
          >
            <svg viewBox="0 0 24 24" width="28" height="28">
              <path fill="currentColor" d="M6.62,10.79C8.06,13.62 10.38,15.94 13.21,17.38L15.41,15.18C15.69,14.9 16.08,14.82 16.43,14.93C17.55,15.3 18.75,15.5 20,15.5A1,1 0 0,1 21,16.5V20A1,1 0 0,1 20,21A17,17 0 0,1 3,4A1,1 0 0,1 4,3H7.5A1,1 0 0,1 8.5,4C8.5,5.25 8.7,6.45 9.07,7.57C9.18,7.92 9.1,8.31 8.82,8.59L6.62,10.79Z" />
            </svg>
          </button>
          <span>${localize("controls.accept", this.hass)}</span>
        </div>
      </div>
    `;
  }

  private _renderActiveControls() {
    const cs = this.callState!;

    return html`
      <div class="call-controls">
        <!-- Mute -->
        <div class="call-controls__label">
          <button
            class="btn btn--md btn--action ${cs.isMuted ? "active" : ""}"
            @click=${this._handleMute}
            aria-label=${cs.isMuted
              ? localize("controls.unmute", this.hass)
              : localize("controls.mute", this.hass)}
            aria-pressed=${cs.isMuted}
          >
            <svg viewBox="0 0 24 24" width="22" height="22">
              ${cs.isMuted
                ? html`<path fill="currentColor" d="M19,11C19,12.19 18.66,13.3 18.1,14.28L16.87,13.05C17.14,12.43 17.3,11.74 17.3,11H19M15,11.16L9,5.18V5A3,3 0 0,1 12,2A3,3 0 0,1 15,5V11L15,11.16M4.27,3L3,4.27L9.01,10.28V11A3,3 0 0,0 12.01,14C12.22,14 12.42,13.97 12.62,13.92L14.01,15.31C13.39,15.6 12.72,15.78 12.01,15.83V19H14.01V21H10.01V19H12.01V15.83C9.24,15.56 7.01,13.5 7.01,11H8.71C8.71,13 10.41,14.29 12.01,14.29C12.33,14.29 12.63,14.24 12.92,14.15L11.51,12.74C11.35,12.77 11.18,12.8 11.01,12.8A1.8,1.8 0 0,1 9.21,11V10.28L4.27,3Z" />`
                : html`<path fill="currentColor" d="M12,2A3,3 0 0,1 15,5V11A3,3 0 0,1 12,14A3,3 0 0,1 9,11V5A3,3 0 0,1 12,2M19,11C19,14.53 16.39,17.44 13,17.93V21H11V17.93C7.61,17.44 5,14.53 5,11H7A5,5 0 0,0 12,16A5,5 0 0,0 17,11H19Z" />`}
            </svg>
          </button>
          <span>${cs.isMuted ? localize("controls.unmute", this.hass) : localize("controls.mute", this.hass)}</span>
        </div>

        <!-- Speaker -->
        <div class="call-controls__label">
          <button
            class="btn btn--md btn--action ${cs.isSpeaker ? "active" : ""}"
            @click=${this._handleSpeaker}
            aria-label=${localize("controls.speaker", this.hass)}
            aria-pressed=${cs.isSpeaker}
          >
            <svg viewBox="0 0 24 24" width="22" height="22">
              <path fill="currentColor" d="M14,3.23V5.29C16.89,6.15 19,8.83 19,12C19,15.17 16.89,17.84 14,18.7V20.77C18,19.86 21,16.28 21,12C21,7.72 18,4.14 14,3.23M16.5,12C16.5,10.23 15.5,8.71 14,7.97V16C15.5,15.29 16.5,13.76 16.5,12M3,9V15H7L12,20V4L7,9H3Z" />
            </svg>
          </button>
          <span>${localize("controls.speaker", this.hass)}</span>
        </div>

        <!-- Hold -->
        <div class="call-controls__label">
          <button
            class="btn btn--md btn--action ${cs.isOnHold ? "active" : ""}"
            @click=${this._handleHold}
            aria-label=${cs.isOnHold
              ? localize("controls.unhold", this.hass)
              : localize("controls.hold", this.hass)}
            aria-pressed=${cs.isOnHold}
          >
            <svg viewBox="0 0 24 24" width="22" height="22">
              <path fill="currentColor" d="M14,19H18V5H14M6,19H10V5H6V19Z" />
            </svg>
          </button>
          <span>${cs.isOnHold ? localize("controls.unhold", this.hass) : localize("controls.hold", this.hass)}</span>
        </div>

        <!-- Record -->
        <div class="call-controls__label">
          <button
            class="btn btn--md btn--action ${cs.isRecording ? "active" : ""}"
            @click=${this._handleRecord}
            aria-label=${cs.isRecording
              ? localize("controls.stop_record", this.hass)
              : localize("controls.record", this.hass)}
            aria-pressed=${cs.isRecording}
          >
            <svg viewBox="0 0 24 24" width="22" height="22">
              ${cs.isRecording
                ? html`<path fill="currentColor" d="M12,2A10,10 0 0,0 2,12A10,10 0 0,0 12,22A10,10 0 0,0 22,12A10,10 0 0,0 12,2M12,20A8,8 0 0,1 4,12A8,8 0 0,1 12,4A8,8 0 0,1 20,12A8,8 0 0,1 12,20M9,8H15V16H9V8Z" />`
                : html`<path fill="currentColor" d="M12,2A10,10 0 0,0 2,12A10,10 0 0,0 12,22A10,10 0 0,0 22,12A10,10 0 0,0 12,2M12,20A8,8 0 0,1 4,12A8,8 0 0,1 12,4A8,8 0 0,1 20,12A8,8 0 0,1 12,20M12,7A5,5 0 0,0 7,12A5,5 0 0,0 12,17A5,5 0 0,0 17,12A5,5 0 0,0 12,7Z" />`}
            </svg>
          </button>
          <span>${cs.isRecording ? localize("controls.stop_record", this.hass) : localize("controls.record", this.hass)}</span>
        </div>

        <!-- Keypad toggle -->
        <div class="call-controls__label">
          <button
            class="btn btn--md btn--action ${this._showKeypad ? "active" : ""}"
            @click=${this._toggleKeypad}
            aria-label=${localize("controls.keypad", this.hass)}
            aria-pressed=${this._showKeypad}
          >
            <svg viewBox="0 0 24 24" width="22" height="22">
              <path fill="currentColor" d="M12,19A2,2 0 0,0 14,17A2,2 0 0,0 12,15A2,2 0 0,0 10,17A2,2 0 0,0 12,19M6,1H18A2,2 0 0,1 20,3V21A2,2 0 0,1 18,23H6A2,2 0 0,1 4,21V3A2,2 0 0,1 6,1M6,3V21H18V3H6M8,5H10V7H8V5M12,5H14V7H12V5M16,5H18V7H16V5M8,9H10V11H8V9M12,9H14V11H12V9M16,9H18V11H16V9M8,13H10V15H8V13M12,13H14V15H12V13M16,13H18V15H16V13Z" />
            </svg>
          </button>
          <span>${localize("controls.keypad", this.hass)}</span>
        </div>

        <!-- Transfer -->
        <div class="call-controls__label">
          <button
            class="btn btn--md btn--action"
            @click=${this._handleTransfer}
            aria-label=${localize("controls.transfer", this.hass)}
          >
            <svg viewBox="0 0 24 24" width="22" height="22">
              <path fill="currentColor" d="M18,13V5H20V13H18M14,5V13H16V5H14M11,5L6,10L11,15V12C15.39,12 19.17,13.58 22,16.28C20.63,11.11 16.33,7.15 11,6.34V5Z" />
            </svg>
          </button>
          <span>${localize("controls.transfer", this.hass)}</span>
        </div>
      </div>

      <!-- Audio device selector (relative container) -->
      <div style="position:relative; text-align:center; padding-bottom:8px;">
        <button
          class="btn btn--sm btn--icon"
          @click=${this._toggleDeviceMenu}
          aria-label=${localize("controls.audio_device", this.hass)}
        >
          <svg viewBox="0 0 24 24" width="18" height="18">
            <path fill="currentColor" d="M12,1C7,1 3,5 3,10V17A3,3 0 0,0 6,20H9V12H5V10A7,7 0 0,1 12,3A7,7 0 0,1 19,10V12H15V20H18A3,3 0 0,0 21,17V10C21,5 16.97,1 12,1Z" />
          </svg>
        </button>
        ${this._showDeviceMenu ? this._renderDeviceMenu() : nothing}
      </div>

      <!-- Hangup button -->
      <div class="popup-footer">
        <button
          class="btn btn--lg btn--hangup"
          @click=${this._handleHangup}
          aria-label=${localize("controls.hangup", this.hass)}
        >
          <svg viewBox="0 0 24 24" width="28" height="28">
            <path fill="currentColor" d="M12,9C10.4,9 8.85,9.25 7.4,9.72V12.82C7.4,13.22 7.17,13.56 6.84,13.72C5.86,14.21 4.97,14.84 4.17,15.57C4,15.75 3.75,15.86 3.5,15.86C3.2,15.86 2.95,15.74 2.77,15.56L0.29,13.08C0.11,12.9 0,12.65 0,12.38C0,12.1 0.11,11.85 0.29,11.67C3.34,8.77 7.46,7 12,7C16.54,7 20.66,8.77 23.71,11.67C23.89,11.85 24,12.1 24,12.38C24,12.65 23.89,12.9 23.71,13.08L21.23,15.56C21.05,15.74 20.8,15.86 20.5,15.86C20.25,15.86 20,15.75 19.83,15.57C19.03,14.84 18.14,14.21 17.16,13.72C16.83,13.56 16.6,13.22 16.6,12.82V9.72C15.15,9.25 13.6,9 12,9Z" />
          </svg>
        </button>
      </div>
    `;
  }

  private _renderInlineKeypad() {
    return html`
      <div class="keypad-inline">
        <ha-voip-dialpad
          .hass=${this.hass}
          callState=${this.callState!.state}
          @voip-dtmf=${this._handleDtmf}
        ></ha-voip-dialpad>
      </div>
    `;
  }

  private _renderDeviceMenu() {
    return html`
      <div class="device-menu" role="listbox" aria-label=${localize("controls.audio_device", this.hass)}>
        ${this._audioDevices.length === 0
          ? html`<div class="device-menu__item">${localize("card.loading", this.hass)}</div>`
          : this._audioDevices.map(
              (device) => html`
                <button
                  class="device-menu__item"
                  role="option"
                  @click=${() => this._selectDevice(device)}
                >
                  ${device.kind === "audioinput" ? "\u{1F3A4} " : "\u{1F50A} "}${device.label}
                </button>
              `,
            )}
      </div>
    `;
  }

  /* ── Event handlers ────────────────────────────────────────────────────── */

  private _handleAccept(): void {
    fireEvent(this, "voip-answer", { call_id: this.callState?.id });
  }

  private _handleReject(): void {
    fireEvent(this, "voip-hangup", { call_id: this.callState?.id });
  }

  private _handleHangup(): void {
    fireEvent(this, "voip-hangup", { call_id: this.callState?.id });
  }

  private _handleMute(): void {
    fireEvent(this, "voip-mute", {
      call_id: this.callState?.id,
      mute: !this.callState?.isMuted,
    });
  }

  private _handleSpeaker(): void {
    fireEvent(this, "voip-speaker", {
      call_id: this.callState?.id,
      speaker: !this.callState?.isSpeaker,
    });
  }

  private _handleHold(): void {
    fireEvent(this, "voip-hold", {
      call_id: this.callState?.id,
      hold: !this.callState?.isOnHold,
    });
  }

  private _handleRecord(): void {
    fireEvent(this, "voip-record", {
      call_id: this.callState?.id,
      record: !this.callState?.isRecording,
    });
  }

  private _handleTransfer(): void {
    fireEvent(this, "voip-transfer-start", { call_id: this.callState?.id });
  }

  private _handleDtmf(e: CustomEvent): void {
    fireEvent(this, "voip-dtmf", {
      call_id: this.callState?.id,
      digit: e.detail.digit,
    });
  }

  private _toggleKeypad(): void {
    this._showKeypad = !this._showKeypad;
  }

  private _toggleDeviceMenu(): void {
    this._showDeviceMenu = !this._showDeviceMenu;
    if (this._showDeviceMenu) {
      this._loadAudioDevices();
    }
  }

  private _selectDevice(device: AudioDeviceInfo): void {
    fireEvent(this, "voip-device-change", {
      deviceId: device.deviceId,
      kind: device.kind,
    });
    this._showDeviceMenu = false;
  }

  private _handleOverlayClick(): void {
    // Only dismiss for active calls, not incoming
    if (this.callState?.state !== "ringing") {
      fireEvent(this, "voip-popup-minimize");
    }
  }

  /* ── Drag to dismiss (mobile) ──────────────────────────────────────────── */

  private _handleTouchStart(e: TouchEvent): void {
    this._touchStartY = e.touches[0].clientY;
    this._isDragging = true;
    this._dragOffsetY = 0;
  }

  private _handleTouchMove(e: TouchEvent): void {
    if (!this._isDragging) return;
    const dy = e.touches[0].clientY - this._touchStartY;
    // Only allow downward drag
    if (dy > 0) {
      this._dragOffsetY = dy;
    }
  }

  private _handleTouchEnd(): void {
    if (!this._isDragging) return;
    this._isDragging = false;

    if (this._dragOffsetY > 150) {
      // Dismiss
      fireEvent(this, "voip-popup-minimize");
    }

    this._dragOffsetY = 0;
  }

  /* ── Timer ─────────────────────────────────────────────────────────────── */

  private _onCallStateChanged(): void {
    if (!this.callState) {
      this._stopTimer();
      return;
    }

    if (
      this.callState.state === "connected" &&
      !this._timerInterval &&
      this.callState.connectTime
    ) {
      this._startTimer();
    } else if (
      this.callState.state !== "connected" &&
      this.callState.state !== "on_hold"
    ) {
      this._stopTimer();
    }
  }

  private _startTimer(): void {
    this._updateElapsed();
    this._timerInterval = setInterval(() => this._updateElapsed(), 1000);
  }

  private _stopTimer(): void {
    if (this._timerInterval) {
      clearInterval(this._timerInterval);
      this._timerInterval = null;
    }
  }

  private _updateElapsed(): void {
    if (!this.callState?.connectTime) return;
    this._elapsed = Math.floor((Date.now() - this.callState.connectTime) / 1000);
  }

  /* ── Helpers ───────────────────────────────────────────────────────────── */

  private async _loadAudioDevices(): Promise<void> {
    try {
      const devices = await navigator.mediaDevices.enumerateDevices();
      this._audioDevices = devices
        .filter((d) => d.kind === "audioinput" || d.kind === "audiooutput")
        .map((d) => ({
          deviceId: d.deviceId,
          label: d.label || `${d.kind} (${d.deviceId.slice(0, 6)})`,
          kind: d.kind as "audioinput" | "audiooutput",
        }));
    } catch {
      this._audioDevices = [];
    }
  }

  private async _loadCameraSnapshot(): Promise<void> {
    if (!this.cameraEntityId || !this.hass) return;

    try {
      const result = await this.hass.callWS<{ content_type: string; content: string }>({
        type: "camera_thumbnail",
        entity_id: this.cameraEntityId,
      });
      if (result?.content) {
        this._cameraUrl = `data:${result.content_type};base64,${result.content}`;
      }
    } catch {
      // Camera may not be available — set a proxy URL fallback
      this._cameraUrl = `/api/camera_proxy/${this.cameraEntityId}`;
    }
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "ha-voip-call-popup": HaVoipCallPopup;
  }
}
