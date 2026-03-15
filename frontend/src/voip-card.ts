/* ──────────────────────────────────────────────────────────────────────────────
 * voip-card.ts — Main HA VoIP Lovelace card
 *
 * This is the entry point for the ha-voip-card custom element. It is
 * registered as a Lovelace card and coordinates all sub-components:
 *   - Extension info display (name, number, status)
 *   - Quick-dial buttons
 *   - Recent calls list (last N)
 *   - Active call controls (mute, hold, hangup, transfer)
 *   - Call popup (incoming / active)
 *   - Dialpad (inline)
 *   - Onboarding wizard (first-run)
 *   - Diagnostics panel
 *   - WebSocket subscription for real-time updates
 *   - WebRTC session management
 *
 * Sub-component imports (side-effect registrations):
 * ────────────────────────────────────────────────────────────────────────── */

import { LitElement, html, css, nothing, type PropertyValues } from "lit";
import { customElement, property, state } from "lit/decorators.js";

// Sub-component registrations (side effects)
import "./dialpad";
import "./call-popup";
import "./onboarding-wizard";
import "./diagnostics-panel";

import {
  cardStyles,
  buttonStyles,
  callControlStyles,
  statusStyles,
  historyStyles,
  responsiveStyles,
} from "./styles";
import { localize, formatDuration, formatRelativeTime } from "./localize";
import type {
  HomeAssistant,
  LovelaceCard,
  LovelaceCardConfig,
  LovelaceCardEditor,
  VoipCardConfig,
  CallState,
  Extension,
  CallHistoryEntry,
  VoipEvent,
  QuickDialEntry,
} from "./types";
import { fireEvent } from "./types";
import { WebRtcManager } from "./webrtc-manager";

/* ──────────────────────────────────────────────────────────────────────────
 * Card configuration editor
 * ────────────────────────────────────────────────────────────────────────── */

@customElement("ha-voip-card-editor")
export class HaVoipCardEditor extends LitElement implements LovelaceCardEditor {
  @property({ attribute: false }) hass?: HomeAssistant;
  @state() private _config?: VoipCardConfig;

  static styles = [
    cardStyles,
    css`
      .editor {
        padding: 16px;
      }
      .editor-row {
        margin-bottom: 12px;
      }
      .editor-row label {
        display: block;
        font-size: 13px;
        font-weight: 500;
        color: var(--voip-secondary-text, #727272);
        margin-bottom: 4px;
      }
      .editor-row input,
      .editor-row select {
        width: 100%;
        padding: 8px 10px;
        border: 1px solid var(--voip-divider, rgba(0,0,0,0.12));
        border-radius: 6px;
        font-size: 14px;
        font-family: inherit;
        background: var(--voip-surface, #fff);
        color: var(--voip-primary-text, #212121);
      }
      .editor-row input:focus,
      .editor-row select:focus {
        outline: none;
        border-color: var(--voip-primary, #03a9f4);
      }
      .editor-toggle {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 8px 0;
      }
      .editor-toggle label {
        margin: 0;
      }
      .quick-dial-entry {
        display: flex;
        gap: 8px;
        margin-bottom: 6px;
        align-items: center;
      }
      .quick-dial-entry input {
        flex: 1;
      }
      .add-btn {
        font-size: 13px;
        color: var(--voip-primary);
        cursor: pointer;
        background: none;
        border: none;
        padding: 4px 0;
        font-family: inherit;
      }
      .remove-btn {
        width: 28px;
        height: 28px;
        border-radius: 50%;
        border: none;
        background: none;
        cursor: pointer;
        color: var(--voip-error, #db4437);
        display: flex;
        align-items: center;
        justify-content: center;
      }
    `,
  ];

  setConfig(config: LovelaceCardConfig): void {
    this._config = config as VoipCardConfig;
  }

  protected render() {
    if (!this._config) return nothing;

    return html`
      <div class="editor">
        <div class="editor-row">
          <label>${localize("config.card_title", this.hass)}</label>
          <input
            type="text"
            .value=${this._config.title || ""}
            @input=${(e: InputEvent) => this._update("title", (e.target as HTMLInputElement).value)}
          />
        </div>

        <div class="editor-row">
          <label>${localize("config.entity", this.hass)}</label>
          <input
            type="text"
            .value=${this._config.entity || ""}
            placeholder="sensor.voip_status"
            @input=${(e: InputEvent) => this._update("entity", (e.target as HTMLInputElement).value)}
          />
        </div>

        <div class="editor-toggle">
          <label>${localize("config.show_dialpad", this.hass)}</label>
          <input
            type="checkbox"
            .checked=${this._config.show_dialpad !== false}
            @change=${(e: Event) => this._update("show_dialpad", (e.target as HTMLInputElement).checked)}
          />
        </div>

        <div class="editor-toggle">
          <label>${localize("config.show_recent", this.hass)}</label>
          <input
            type="checkbox"
            .checked=${this._config.show_recent_calls !== false}
            @change=${(e: Event) => this._update("show_recent_calls", (e.target as HTMLInputElement).checked)}
          />
        </div>

        <div class="editor-row">
          <label>${localize("config.recent_count", this.hass)}</label>
          <input
            type="number"
            min="1"
            max="20"
            .value=${String(this._config.recent_calls_count ?? 5)}
            @input=${(e: InputEvent) => this._update("recent_calls_count", parseInt((e.target as HTMLInputElement).value) || 5)}
          />
        </div>

        <div class="editor-toggle">
          <label>${localize("config.show_diagnostics", this.hass)}</label>
          <input
            type="checkbox"
            .checked=${this._config.show_diagnostics === true}
            @change=${(e: Event) => this._update("show_diagnostics", (e.target as HTMLInputElement).checked)}
          />
        </div>

        <div class="editor-toggle">
          <label>${localize("config.compact_mode", this.hass)}</label>
          <input
            type="checkbox"
            .checked=${this._config.compact_mode === true}
            @change=${(e: Event) => this._update("compact_mode", (e.target as HTMLInputElement).checked)}
          />
        </div>

        <div class="editor-toggle">
          <label>${localize("config.enable_dtmf", this.hass)}</label>
          <input
            type="checkbox"
            .checked=${this._config.enable_dtmf_tones !== false}
            @change=${(e: Event) => this._update("enable_dtmf_tones", (e.target as HTMLInputElement).checked)}
          />
        </div>

        <div class="editor-toggle">
          <label>${localize("config.auto_answer", this.hass)}</label>
          <input
            type="checkbox"
            .checked=${this._config.auto_answer === true}
            @change=${(e: Event) => this._update("auto_answer", (e.target as HTMLInputElement).checked)}
          />
        </div>

        <!-- Quick dial entries -->
        <div class="editor-row">
          <label>${localize("config.quick_dial", this.hass)}</label>
          ${(this._config.quick_dial || []).map(
            (entry, idx) => html`
              <div class="quick-dial-entry">
                <input
                  type="text"
                  placeholder=${localize("config.name", this.hass)}
                  .value=${entry.name}
                  @input=${(e: InputEvent) => this._updateQuickDial(idx, "name", (e.target as HTMLInputElement).value)}
                />
                <input
                  type="tel"
                  placeholder=${localize("config.number", this.hass)}
                  .value=${entry.number}
                  @input=${(e: InputEvent) => this._updateQuickDial(idx, "number", (e.target as HTMLInputElement).value)}
                />
                <button class="remove-btn" @click=${() => this._removeQuickDial(idx)}>
                  <svg viewBox="0 0 24 24" width="16" height="16">
                    <path fill="currentColor" d="M19,6.41L17.59,5L12,10.59L6.41,5L5,6.41L10.59,12L5,17.59L6.41,19L12,13.41L17.59,19L19,17.59L13.41,12L19,6.41Z" />
                  </svg>
                </button>
              </div>
            `,
          )}
          <button class="add-btn" @click=${this._addQuickDial}>
            + ${localize("config.add_quick_dial", this.hass)}
          </button>
        </div>
      </div>
    `;
  }

  private _update(key: string, value: unknown): void {
    if (!this._config) return;
    this._config = { ...this._config, [key]: value };
    fireEvent(this, "config-changed", { config: this._config });
  }

  private _updateQuickDial(
    idx: number,
    key: "name" | "number",
    value: string,
  ): void {
    const entries = [...(this._config?.quick_dial || [])];
    entries[idx] = { ...entries[idx], [key]: value };
    this._update("quick_dial", entries);
  }

  private _addQuickDial(): void {
    const entries = [...(this._config?.quick_dial || []), { name: "", number: "" }];
    this._update("quick_dial", entries);
  }

  private _removeQuickDial(idx: number): void {
    const entries = (this._config?.quick_dial || []).filter((_, i) => i !== idx);
    this._update("quick_dial", entries);
  }
}

/* ──────────────────────────────────────────────────────────────────────────
 * Main VoIP Lovelace card
 * ────────────────────────────────────────────────────────────────────────── */

@customElement("ha-voip-card")
export class HaVoipCard extends LitElement implements LovelaceCard {
  /* ── Properties ────────────────────────────────────────────────────────── */

  @property({ attribute: false }) hass?: HomeAssistant;

  @state() private _config?: VoipCardConfig;
  @state() private _callState: CallState | null = null;
  @state() private _extensions: Extension[] = [];
  @state() private _history: CallHistoryEntry[] = [];
  @state() private _showPopup = false;
  @state() private _showDialpad = false;
  @state() private _showDiagnostics = false;
  @state() private _showOnboarding = false;
  @state() private _view: "main" | "dialpad" | "diagnostics" | "onboarding" = "main";
  @state() private _incomingCameraEntity?: string;

  private _webrtc: WebRtcManager;
  private _unsubscribe?: () => void;
  private _remoteAudio: HTMLAudioElement | null = null;
  private _ringtoneAudio: HTMLAudioElement | null = null;

  constructor() {
    super();
    this._webrtc = new WebRtcManager({
      onConnectionStateChange: (state) => {
        if (state === "connected") {
          this._stopRingtone();
        }
      },
      onRemoteStream: (stream) => {
        this._attachRemoteAudio(stream);
      },
      onError: (err) => {
        console.error("[VoIP Card] WebRTC error:", err);
      },
    });
  }

  /* ── Lovelace card API ─────────────────────────────────────────────────── */

  static getConfigElement(): LovelaceCardEditor {
    return document.createElement("ha-voip-card-editor") as LovelaceCardEditor;
  }

  static getStubConfig(): VoipCardConfig {
    return {
      type: "custom:ha-voip-card",
      title: "VoIP Phone",
      show_recent_calls: true,
      recent_calls_count: 5,
      show_dialpad: true,
      enable_dtmf_tones: true,
    };
  }

  setConfig(config: LovelaceCardConfig): void {
    if (!config) {
      throw new Error(localize("card.no_config"));
    }
    this._config = {
      show_recent_calls: true,
      recent_calls_count: 5,
      show_dialpad: true,
      enable_dtmf_tones: true,
      ...config,
    } as VoipCardConfig;
  }

  getCardSize(): number {
    let size = 3; // header + extension info
    if (this._config?.show_dialpad !== false) size += 5;
    if (this._config?.show_recent_calls !== false) size += 3;
    if (this._config?.quick_dial?.length) size += 1;
    return size;
  }

  /* ── Styles ────────────────────────────────────────────────────────────── */

  static styles = [
    cardStyles,
    buttonStyles,
    callControlStyles,
    statusStyles,
    historyStyles,
    responsiveStyles,
    css`
      .card-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 16px 16px 8px;
      }

      .card-title {
        font-size: 18px;
        font-weight: 500;
        margin: 0;
      }

      .card-header__actions {
        display: flex;
        gap: 4px;
      }

      .extension-info {
        display: flex;
        align-items: center;
        gap: 12px;
        padding: 8px 16px 16px;
      }

      .extension-avatar {
        width: 44px;
        height: 44px;
        border-radius: 50%;
        background-color: var(--voip-primary);
        color: #fff;
        display: flex;
        align-items: center;
        justify-content: center;
        font-size: 18px;
        font-weight: 500;
        flex-shrink: 0;
      }

      .extension-details {
        flex: 1;
        min-width: 0;
      }

      .extension-name {
        font-size: 16px;
        font-weight: 500;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
      }

      .extension-number {
        font-size: 13px;
        color: var(--voip-secondary-text);
      }

      .extension-status {
        display: flex;
        align-items: center;
        font-size: 12px;
        color: var(--voip-secondary-text);
        margin-top: 2px;
      }

      .active-call-banner {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 10px 16px;
        background-color: rgba(67, 160, 71, 0.1);
        border-bottom: 1px solid var(--voip-divider);
        cursor: pointer;
      }

      .active-call-banner:hover {
        background-color: rgba(67, 160, 71, 0.15);
      }

      .active-call-info {
        display: flex;
        align-items: center;
        gap: 8px;
        flex: 1;
        min-width: 0;
      }

      .active-call-name {
        font-size: 14px;
        font-weight: 500;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
      }

      .active-call-timer {
        font-size: 13px;
        font-variant-numeric: tabular-nums;
        color: var(--voip-success);
        flex-shrink: 0;
      }

      .quick-dial-section {
        padding: 12px 16px;
        border-bottom: 1px solid var(--voip-divider);
      }

      .quick-dial-title {
        font-size: 12px;
        font-weight: 500;
        text-transform: uppercase;
        letter-spacing: 0.5px;
        color: var(--voip-secondary-text);
        margin: 0 0 8px;
      }

      .quick-dial-grid {
        display: flex;
        flex-wrap: wrap;
        gap: 8px;
      }

      .quick-dial-chip {
        display: inline-flex;
        align-items: center;
        gap: 6px;
        padding: 6px 14px;
        border-radius: 20px;
        border: 1px solid var(--voip-divider);
        background: none;
        font-size: 13px;
        font-family: inherit;
        color: var(--voip-primary-text);
        cursor: pointer;
        transition: background-color 0.15s, border-color 0.15s;
      }

      .quick-dial-chip:hover {
        background-color: rgba(0, 0, 0, 0.04);
        border-color: var(--voip-primary);
      }

      .quick-dial-chip:active {
        background-color: var(--voip-primary);
        color: #fff;
        border-color: var(--voip-primary);
      }

      .section-title {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 12px 16px 4px;
      }

      .section-title h3 {
        font-size: 12px;
        font-weight: 500;
        text-transform: uppercase;
        letter-spacing: 0.5px;
        color: var(--voip-secondary-text);
        margin: 0;
      }

      .dialpad-section {
        border-top: 1px solid var(--voip-divider);
      }

      .empty-state {
        text-align: center;
        padding: 20px;
        color: var(--voip-disabled);
        font-size: 13px;
      }

      .nav-tabs {
        display: flex;
        border-bottom: 1px solid var(--voip-divider);
      }

      .nav-tab {
        flex: 1;
        padding: 10px;
        text-align: center;
        font-size: 13px;
        font-weight: 500;
        color: var(--voip-secondary-text);
        background: none;
        border: none;
        border-bottom: 2px solid transparent;
        cursor: pointer;
        font-family: inherit;
        transition: color 0.2s, border-color 0.2s;
      }

      .nav-tab:hover {
        color: var(--voip-primary-text);
      }

      .nav-tab--active {
        color: var(--voip-primary);
        border-bottom-color: var(--voip-primary);
      }
    `,
  ];

  /* ── Lifecycle ─────────────────────────────────────────────────────────── */

  connectedCallback(): void {
    super.connectedCallback();
    this._createRemoteAudio();
  }

  disconnectedCallback(): void {
    super.disconnectedCallback();
    this._unsubscribeWs();
    this._webrtc.hangup();
    this._destroyAudioElements();
  }

  protected updated(changed: PropertyValues): void {
    super.updated(changed);

    if (changed.has("hass") && this.hass) {
      this._webrtc.setHass(this.hass);

      // Subscribe to VoIP events on first hass update
      if (!this._unsubscribe) {
        this._subscribeWs();
      }
    }
  }

  /* ── Render ────────────────────────────────────────────────────────────── */

  protected render() {
    if (!this._config) {
      return html`<ha-card><div class="empty-state">${localize("card.no_config")}</div></ha-card>`;
    }

    return html`
      <ha-card>
        ${this._renderHeader()}
        ${this._renderContent()}
      </ha-card>

      <!-- Call popup overlay -->
      ${this._showPopup && this._callState
        ? html`
            <ha-voip-call-popup
              .hass=${this.hass}
              .callState=${this._callState}
              .cameraEntityId=${this._incomingCameraEntity}
              @voip-answer=${this._handleAnswer}
              @voip-hangup=${this._handleHangup}
              @voip-mute=${this._handleMute}
              @voip-hold=${this._handleHold}
              @voip-speaker=${this._handleSpeaker}
              @voip-record=${this._handleRecord}
              @voip-transfer-start=${this._handleTransferStart}
              @voip-dtmf=${this._handleDtmf}
              @voip-device-change=${this._handleDeviceChange}
              @voip-popup-minimize=${() => { this._showPopup = false; }}
            ></ha-voip-call-popup>
          `
        : nothing}

      <!-- Onboarding wizard overlay -->
      ${this._showOnboarding
        ? html`
            <div class="popup-overlay" style="position:fixed;inset:0;z-index:1001;display:flex;align-items:center;justify-content:center;background:rgba(0,0,0,0.6)">
              <div style="background:var(--voip-surface,#fff);border-radius:var(--voip-radius,12px);max-width:480px;width:90vw;max-height:90vh;overflow-y:auto">
                <ha-voip-onboarding
                  .hass=${this.hass}
                  @voip-onboarding-complete=${this._handleOnboardingComplete}
                ></ha-voip-onboarding>
              </div>
            </div>
          `
        : nothing}
    `;
  }

  private _renderHeader() {
    const title = this._config?.title || localize("card.title", this.hass);

    return html`
      <div class="card-header">
        <h2 class="card-title">${title}</h2>
        <div class="card-header__actions">
          ${this._config?.show_diagnostics
            ? html`
                <button
                  class="btn btn--sm btn--icon"
                  @click=${() => { this._view = this._view === "diagnostics" ? "main" : "diagnostics"; }}
                  aria-label=${localize("diag.title", this.hass)}
                >
                  <svg viewBox="0 0 24 24" width="20" height="20">
                    <path fill="currentColor" d="M12,15.5A3.5,3.5 0 0,1 8.5,12A3.5,3.5 0 0,1 12,8.5A3.5,3.5 0 0,1 15.5,12A3.5,3.5 0 0,1 12,15.5M19.43,12.97C19.47,12.65 19.5,12.33 19.5,12C19.5,11.67 19.47,11.34 19.43,11L21.54,9.37C21.73,9.22 21.78,8.95 21.66,8.73L19.66,5.27C19.54,5.05 19.27,4.96 19.05,5.05L16.56,6.05C16.04,5.66 15.5,5.32 14.87,5.07L14.5,2.42C14.46,2.18 14.25,2 14,2H10C9.75,2 9.54,2.18 9.5,2.42L9.13,5.07C8.5,5.32 7.96,5.66 7.44,6.05L4.95,5.05C4.73,4.96 4.46,5.05 4.34,5.27L2.34,8.73C2.21,8.95 2.27,9.22 2.46,9.37L4.57,11C4.53,11.34 4.5,11.67 4.5,12C4.5,12.33 4.53,12.65 4.57,12.97L2.46,14.63C2.27,14.78 2.21,15.05 2.34,15.27L4.34,18.73C4.46,18.95 4.73,19.03 4.95,18.95L7.44,17.94C7.96,18.34 8.5,18.68 9.13,18.93L9.5,21.58C9.54,21.82 9.75,22 10,22H14C14.25,22 14.46,21.82 14.5,21.58L14.87,18.93C15.5,18.67 16.04,18.34 16.56,17.94L19.05,18.95C19.27,19.03 19.54,18.95 19.66,18.73L21.66,15.27C21.78,15.05 21.73,14.78 21.54,14.63L19.43,12.97Z" />
                  </svg>
                </button>
              `
            : nothing}
          <button
            class="btn btn--sm btn--icon"
            @click=${() => { this._showOnboarding = true; }}
            aria-label="Setup"
          >
            <svg viewBox="0 0 24 24" width="20" height="20">
              <path fill="currentColor" d="M11,9H13V7H11M12,20C7.59,20 4,16.41 4,12C4,7.59 7.59,4 12,4C16.41,4 20,7.59 20,12C20,16.41 16.41,20 12,20M12,2A10,10 0 0,0 2,12A10,10 0 0,0 12,22A10,10 0 0,0 22,12A10,10 0 0,0 12,2M11,17H13V11H11V17Z" />
            </svg>
          </button>
        </div>
      </div>
    `;
  }

  private _renderContent() {
    switch (this._view) {
      case "diagnostics":
        return html`
          <ha-voip-diagnostics .hass=${this.hass}></ha-voip-diagnostics>
        `;
      case "main":
      default:
        return this._renderMainView();
    }
  }

  private _renderMainView() {
    const isActive =
      this._callState &&
      this._callState.state !== "idle" &&
      this._callState.state !== "ended";

    return html`
      <!-- Extension info -->
      ${this._renderExtensionInfo()}

      <!-- Active call banner -->
      ${isActive ? this._renderActiveCallBanner() : nothing}

      <!-- Call status display -->
      ${this._callState && this._callState.state !== "idle"
        ? this._renderCallStatus()
        : nothing}

      <!-- Active call controls -->
      ${isActive ? this._renderCallControls() : nothing}

      <!-- Quick dial -->
      ${this._config?.quick_dial?.length ? this._renderQuickDial() : nothing}

      <!-- Nav tabs: Dialpad / Recent -->
      ${this._renderTabs()}

      <!-- Tab content -->
      ${this._showDialpad
        ? html`
            <div class="dialpad-section">
              <ha-voip-dialpad
                .hass=${this.hass}
                .callState=${this._callState?.state ?? "idle"}
                .enableDtmf=${this._config?.enable_dtmf_tones !== false}
                @voip-call=${this._handleDialCall}
                @voip-hangup=${this._handleHangup}
                @voip-dtmf=${this._handleDtmf}
              ></ha-voip-dialpad>
            </div>
          `
        : this._renderRecentCalls()}
    `;
  }

  private _renderExtensionInfo() {
    // Show the current user's extension or the first configured one
    const myExt = this._extensions.find(
      (e) => e.userId === this.hass?.user?.id,
    ) || this._extensions[0];

    if (!myExt) {
      return html`
        <div class="extension-info">
          <div class="extension-avatar">
            <svg viewBox="0 0 24 24" width="24" height="24">
              <path fill="currentColor" d="M6.62,10.79C8.06,13.62 10.38,15.94 13.21,17.38L15.41,15.18C15.69,14.9 16.08,14.82 16.43,14.93C17.55,15.3 18.75,15.5 20,15.5A1,1 0 0,1 21,16.5V20A1,1 0 0,1 20,21A17,17 0 0,1 3,4A1,1 0 0,1 4,3H7.5A1,1 0 0,1 8.5,4C8.5,5.25 8.7,6.45 9.07,7.57C9.18,7.92 9.1,8.31 8.82,8.59L6.62,10.79Z" />
            </svg>
          </div>
          <div class="extension-details">
            <div class="extension-name">${this.hass?.user?.name || "VoIP"}</div>
            <div class="extension-status">
              <span class="status-dot status-dot--offline"></span>
              <span>${localize("ext.offline", this.hass)}</span>
            </div>
          </div>
        </div>
      `;
    }

    const initials = myExt.name
      .split(" ")
      .map((w) => w[0])
      .join("")
      .slice(0, 2)
      .toUpperCase();

    return html`
      <div class="extension-info">
        <div class="extension-avatar">${initials}</div>
        <div class="extension-details">
          <div class="extension-name">${myExt.name}</div>
          <div class="extension-number">Ext. ${myExt.number}</div>
          <div class="extension-status">
            <span class="status-dot status-dot--${myExt.status}"></span>
            <span>${localize(`ext.${myExt.status}`, this.hass)}</span>
          </div>
        </div>
        <span class="badge badge--${this._callState?.state || "idle"}">
          ${localize(`call.${this._callState?.state || "idle"}`, this.hass)}
        </span>
      </div>
    `;
  }

  private _renderActiveCallBanner() {
    if (!this._callState) return nothing;

    const name = this._callState.remoteName || this._callState.remoteNumber;
    const elapsed = this._callState.connectTime
      ? Math.floor((Date.now() - this._callState.connectTime) / 1000)
      : 0;

    return html`
      <div class="active-call-banner" @click=${() => { this._showPopup = true; }}>
        <div class="active-call-info">
          <svg viewBox="0 0 24 24" width="18" height="18" style="color:var(--voip-success)">
            <path fill="currentColor" d="M6.62,10.79C8.06,13.62 10.38,15.94 13.21,17.38L15.41,15.18C15.69,14.9 16.08,14.82 16.43,14.93C17.55,15.3 18.75,15.5 20,15.5A1,1 0 0,1 21,16.5V20A1,1 0 0,1 20,21A17,17 0 0,1 3,4A1,1 0 0,1 4,3H7.5A1,1 0 0,1 8.5,4C8.5,5.25 8.7,6.45 9.07,7.57C9.18,7.92 9.1,8.31 8.82,8.59L6.62,10.79Z" />
          </svg>
          <span class="active-call-name">${name}</span>
        </div>
        <span class="active-call-timer">${formatDuration(elapsed)}</span>
      </div>
    `;
  }

  private _renderCallStatus() {
    // Simple inline status for non-popup view
    return nothing; // Detailed status is shown in the banner + popup
  }

  private _renderCallControls() {
    if (!this._callState) return nothing;
    const cs = this._callState;

    return html`
      <div class="call-controls">
        <div class="call-controls__label">
          <button
            class="btn btn--md btn--action ${cs.isMuted ? "active" : ""}"
            @click=${() => this._handleMute(new CustomEvent("voip-mute", { detail: { mute: !cs.isMuted } }))}
            aria-label=${cs.isMuted ? localize("controls.unmute", this.hass) : localize("controls.mute", this.hass)}
          >
            <svg viewBox="0 0 24 24" width="20" height="20">
              ${cs.isMuted
                ? html`<path fill="currentColor" d="M19,11C19,12.19 18.66,13.3 18.1,14.28L16.87,13.05C17.14,12.43 17.3,11.74 17.3,11H19M15,11.16L9,5.18V5A3,3 0 0,1 12,2A3,3 0 0,1 15,5V11L15,11.16M4.27,3L3,4.27L9.01,10.28V11A3,3 0 0,0 12.01,14C12.22,14 12.42,13.97 12.62,13.92L14.01,15.31C13.39,15.6 12.72,15.78 12.01,15.83V19H14.01V21H10.01V19H12.01V15.83C9.24,15.56 7.01,13.5 7.01,11H8.71C8.71,13 10.41,14.29 12.01,14.29C12.33,14.29 12.63,14.24 12.92,14.15L11.51,12.74C11.35,12.77 11.18,12.8 11.01,12.8A1.8,1.8 0 0,1 9.21,11V10.28L4.27,3Z" />`
                : html`<path fill="currentColor" d="M12,2A3,3 0 0,1 15,5V11A3,3 0 0,1 12,14A3,3 0 0,1 9,11V5A3,3 0 0,1 12,2M19,11C19,14.53 16.39,17.44 13,17.93V21H11V17.93C7.61,17.44 5,14.53 5,11H7A5,5 0 0,0 12,16A5,5 0 0,0 17,11H19Z" />`}
            </svg>
          </button>
          <span>${cs.isMuted ? localize("controls.unmute", this.hass) : localize("controls.mute", this.hass)}</span>
        </div>

        <div class="call-controls__label">
          <button
            class="btn btn--md btn--action ${cs.isOnHold ? "active" : ""}"
            @click=${() => this._handleHold(new CustomEvent("voip-hold", { detail: { hold: !cs.isOnHold } }))}
            aria-label=${localize("controls.hold", this.hass)}
          >
            <svg viewBox="0 0 24 24" width="20" height="20">
              <path fill="currentColor" d="M14,19H18V5H14M6,19H10V5H6V19Z" />
            </svg>
          </button>
          <span>${cs.isOnHold ? localize("controls.unhold", this.hass) : localize("controls.hold", this.hass)}</span>
        </div>

        <div class="call-controls__label">
          <button
            class="btn btn--md btn--hangup"
            @click=${() => this._handleHangup()}
            aria-label=${localize("controls.hangup", this.hass)}
          >
            <svg viewBox="0 0 24 24" width="20" height="20">
              <path fill="currentColor" d="M12,9C10.4,9 8.85,9.25 7.4,9.72V12.82C7.4,13.22 7.17,13.56 6.84,13.72C5.86,14.21 4.97,14.84 4.17,15.57C4,15.75 3.75,15.86 3.5,15.86C3.2,15.86 2.95,15.74 2.77,15.56L0.29,13.08C0.11,12.9 0,12.65 0,12.38C0,12.1 0.11,11.85 0.29,11.67C3.34,8.77 7.46,7 12,7C16.54,7 20.66,8.77 23.71,11.67C23.89,11.85 24,12.1 24,12.38C24,12.65 23.89,12.9 23.71,13.08L21.23,15.56C21.05,15.74 20.8,15.86 20.5,15.86C20.25,15.86 20,15.75 19.83,15.57C19.03,14.84 18.14,14.21 17.16,13.72C16.83,13.56 16.6,13.22 16.6,12.82V9.72C15.15,9.25 13.6,9 12,9Z" />
            </svg>
          </button>
          <span>${localize("controls.hangup", this.hass)}</span>
        </div>

        <div class="call-controls__label">
          <button
            class="btn btn--md btn--action"
            @click=${this._handleTransferStart}
            aria-label=${localize("controls.transfer", this.hass)}
          >
            <svg viewBox="0 0 24 24" width="20" height="20">
              <path fill="currentColor" d="M18,13V5H20V13H18M14,5V13H16V5H14M11,5L6,10L11,15V12C15.39,12 19.17,13.58 22,16.28C20.63,11.11 16.33,7.15 11,6.34V5Z" />
            </svg>
          </button>
          <span>${localize("controls.transfer", this.hass)}</span>
        </div>
      </div>
    `;
  }

  private _renderQuickDial() {
    const entries = this._config?.quick_dial || [];

    return html`
      <div class="quick-dial-section">
        <p class="quick-dial-title">${localize("quickdial.title", this.hass)}</p>
        <div class="quick-dial-grid">
          ${entries.map(
            (entry) => html`
              <button
                class="quick-dial-chip"
                @click=${() => this._dialNumber(entry.number)}
                aria-label="${entry.name} (${entry.number})"
              >
                ${entry.icon
                  ? html`<ha-icon icon=${entry.icon}></ha-icon>`
                  : nothing}
                ${entry.name}
              </button>
            `,
          )}
        </div>
      </div>
    `;
  }

  private _renderTabs() {
    if (this._config?.compact_mode) return nothing;

    return html`
      <div class="nav-tabs">
        <button
          class="nav-tab ${this._showDialpad ? "nav-tab--active" : ""}"
          @click=${() => { this._showDialpad = true; }}
          ?hidden=${this._config?.show_dialpad === false}
        >
          ${localize("dialpad.title", this.hass)}
        </button>
        <button
          class="nav-tab ${!this._showDialpad ? "nav-tab--active" : ""}"
          @click=${() => { this._showDialpad = false; }}
          ?hidden=${this._config?.show_recent_calls === false}
        >
          ${localize("history.title", this.hass)}
        </button>
      </div>
    `;
  }

  private _renderRecentCalls() {
    if (this._config?.show_recent_calls === false) return nothing;

    const count = this._config?.recent_calls_count ?? 5;
    const calls = this._history.slice(0, count);

    if (calls.length === 0) {
      return html`<div class="empty-state">${localize("history.no_calls", this.hass)}</div>`;
    }

    return html`
      <ul class="history-list">
        ${calls.map(
          (entry) => html`
            <li
              class="history-item"
              @click=${() => this._dialNumber(entry.remoteNumber)}
              tabindex="0"
              role="button"
              aria-label="${entry.remoteName || entry.remoteNumber}"
            >
              <div class="history-item__icon ${this._historyIconClass(entry)}">
                ${this._historyIcon(entry)}
              </div>
              <div class="history-item__info">
                <div class="history-item__name">
                  ${entry.remoteName || entry.remoteNumber}
                </div>
                <div class="history-item__number">
                  ${entry.remoteName ? entry.remoteNumber : ""}
                </div>
              </div>
              <div class="history-item__meta">
                <div class="history-item__time">
                  ${formatRelativeTime(entry.startTime)}
                </div>
                <div class="history-item__duration">
                  ${entry.answered ? formatDuration(entry.duration) : localize("history.missed", this.hass)}
                </div>
              </div>
            </li>
          `,
        )}
      </ul>
    `;
  }

  private _historyIconClass(entry: CallHistoryEntry): string {
    if (!entry.answered) return "history-item__icon--missed";
    return entry.direction === "inbound"
      ? "history-item__icon--inbound"
      : "history-item__icon--outbound";
  }

  private _historyIcon(entry: CallHistoryEntry) {
    if (!entry.answered) {
      return html`<svg viewBox="0 0 24 24" width="18" height="18"><path fill="currentColor" d="M6.62,10.79C8.06,13.62 10.38,15.94 13.21,17.38L15.41,15.18C15.69,14.9 16.08,14.82 16.43,14.93C17.55,15.3 18.75,15.5 20,15.5A1,1 0 0,1 21,16.5V20A1,1 0 0,1 20,21A17,17 0 0,1 3,4A1,1 0 0,1 4,3H7.5A1,1 0 0,1 8.5,4C8.5,5.25 8.7,6.45 9.07,7.57C9.18,7.92 9.1,8.31 8.82,8.59L6.62,10.79Z"/></svg>`;
    }
    if (entry.direction === "inbound") {
      return html`<svg viewBox="0 0 24 24" width="18" height="18"><path fill="currentColor" d="M20,5.41L18.59,4L7,15.59V9H5V19H15V17H8.41L20,5.41Z"/></svg>`;
    }
    return html`<svg viewBox="0 0 24 24" width="18" height="18"><path fill="currentColor" d="M4,18.59L5.41,20L17,8.41V15H19V5H9V7H15.59L4,18.59Z"/></svg>`;
  }

  /* ── WebSocket subscription ────────────────────────────────────────────── */

  private async _subscribeWs(): Promise<void> {
    if (!this.hass) return;

    try {
      const unsub = await this.hass.connection.subscribeMessage<VoipEvent>(
        (event) => this._handleVoipEvent(event),
        { type: "voip/subscribe" },
      );
      this._unsubscribe = unsub;

      // Fetch initial state
      this._fetchExtensions();
      this._fetchHistory();
    } catch (err) {
      console.error("[VoIP Card] Failed to subscribe:", err);
    }
  }

  private _unsubscribeWs(): void {
    if (this._unsubscribe) {
      this._unsubscribe();
      this._unsubscribe = undefined;
    }
  }

  private _handleVoipEvent(event: VoipEvent): void {
    switch (event.event) {
      case "call_state":
        this._callState = event.data;

        // Show popup for incoming / active calls
        if (
          event.data.state === "ringing" &&
          event.data.direction === "inbound"
        ) {
          this._showPopup = true;
          this._playRingtone();

          // Auto-answer if configured
          if (this._config?.auto_answer) {
            setTimeout(() => {
              this._handleAnswer();
            }, 1000);
          }
        } else if (event.data.state === "ended") {
          this._stopRingtone();
          // Delay hiding popup so the user sees "Call Ended"
          setTimeout(() => {
            if (this._callState?.state === "ended") {
              this._showPopup = false;
              this._callState = null;
            }
          }, 2000);
          // Refresh history
          this._fetchHistory();
        }
        break;

      case "extensions":
        this._extensions = event.data;
        break;

      case "history":
        this._history = event.data;
        break;

      case "incoming_call":
        this._incomingCameraEntity = event.data.camera_entity_id;
        break;

      case "webrtc_offer":
        // Inbound call — answer with WebRTC
        this._webrtc.answerCall(event.call_id, event.sdp);
        break;

      case "webrtc_answer":
        this._webrtc.handleRemoteAnswer(event.sdp);
        break;

      case "webrtc_candidate":
        this._webrtc.addIceCandidate(event.candidate);
        break;
    }
  }

  private async _fetchExtensions(): Promise<void> {
    if (!this.hass) return;
    try {
      const data = await this.hass.callWS<Extension[]>({ type: "voip/extensions" });
      if (data) this._extensions = data;
    } catch {
      // Not critical
    }
  }

  private async _fetchHistory(): Promise<void> {
    if (!this.hass) return;
    try {
      const data = await this.hass.callWS<CallHistoryEntry[]>({ type: "voip/history" });
      if (data) this._history = data;
    } catch {
      // Not critical
    }
  }

  /* ── Call actions ──────────────────────────────────────────────────────── */

  private _dialNumber(number: string): void {
    if (!this.hass || !number) return;
    this.hass.callWS({ type: "voip/call", number }).then((result: any) => {
      if (result?.call_id) {
        this._webrtc.startCall(result.call_id);
      }
    });
  }

  private _handleDialCall(e: CustomEvent): void {
    this._dialNumber(e.detail.number);
  }

  private async _handleAnswer(e?: CustomEvent): Promise<void> {
    if (!this.hass || !this._callState) return;
    const callId = e?.detail?.call_id || this._callState.id;
    this._stopRingtone();
    await this.hass.callWS({ type: "voip/answer", call_id: callId });
  }

  private async _handleHangup(e?: CustomEvent): Promise<void> {
    if (!this.hass) return;
    const callId = e?.detail?.call_id || this._callState?.id;
    if (callId) {
      await this.hass.callWS({ type: "voip/hangup", call_id: callId });
    }
    await this._webrtc.hangup();
    this._stopRingtone();
  }

  private async _handleMute(e: CustomEvent): Promise<void> {
    if (!this.hass || !this._callState) return;
    const mute = e.detail?.mute ?? !this._callState.isMuted;
    this._webrtc.setMute(mute);
    await this.hass.callWS({
      type: "voip/mute",
      call_id: this._callState.id,
      mute,
    });
  }

  private async _handleHold(e: CustomEvent): Promise<void> {
    if (!this.hass || !this._callState) return;
    const hold = e.detail?.hold ?? !this._callState.isOnHold;
    await this.hass.callWS({
      type: "voip/hold",
      call_id: this._callState.id,
      hold,
    });
  }

  private async _handleSpeaker(e: CustomEvent): Promise<void> {
    // Speaker toggle is handled client-side by routing audio output
    // This is a UI state change; actual audio routing happens in the popup
    if (!this._callState) return;
    this._callState = {
      ...this._callState,
      isSpeaker: e.detail?.speaker ?? !this._callState.isSpeaker,
    };
  }

  private async _handleRecord(e: CustomEvent): Promise<void> {
    if (!this.hass || !this._callState) return;
    const record = e.detail?.record ?? !this._callState.isRecording;
    await this.hass.callWS({
      type: "voip/record",
      call_id: this._callState.id,
      record,
    });
  }

  private _handleTransferStart(): void {
    // Prompt user for transfer target (could open a sub-dialog)
    const target = prompt(localize("controls.transfer", this.hass));
    if (target && this.hass && this._callState) {
      this.hass.callWS({
        type: "voip/transfer",
        call_id: this._callState.id,
        target,
      });
    }
  }

  private async _handleDtmf(e: CustomEvent): Promise<void> {
    if (!this.hass || !this._callState) return;
    await this.hass.callWS({
      type: "voip/dtmf",
      call_id: this._callState.id,
      digit: e.detail.digit,
    });
  }

  private async _handleDeviceChange(e: CustomEvent): Promise<void> {
    const { deviceId, kind } = e.detail;
    if (kind === "audioinput") {
      await this._webrtc.switchAudioInput(deviceId);
    } else if (kind === "audiooutput" && this._remoteAudio) {
      await this._webrtc.setAudioOutput(this._remoteAudio, deviceId);
    }
  }

  private _handleOnboardingComplete(e: CustomEvent): void {
    this._showOnboarding = false;
    // Config is saved via the onboarding component directly
  }

  /* ── Audio helpers ─────────────────────────────────────────────────────── */

  private _createRemoteAudio(): void {
    this._remoteAudio = document.createElement("audio");
    this._remoteAudio.autoplay = true;
    this._remoteAudio.playsInline = true;
    // Append to body so it persists outside shadow DOM
    document.body.appendChild(this._remoteAudio);
  }

  private _attachRemoteAudio(stream: MediaStream): void {
    if (this._remoteAudio) {
      this._remoteAudio.srcObject = stream;
    }
  }

  private _playRingtone(): void {
    const url = this._config?.ringtone_url;
    if (!url) return;

    try {
      this._ringtoneAudio = new Audio(url);
      this._ringtoneAudio.loop = true;
      this._ringtoneAudio.play().catch(() => {
        // Autoplay may be blocked
      });
    } catch {
      // Ringtone not critical
    }
  }

  private _stopRingtone(): void {
    if (this._ringtoneAudio) {
      this._ringtoneAudio.pause();
      this._ringtoneAudio.currentTime = 0;
      this._ringtoneAudio = null;
    }
  }

  private _destroyAudioElements(): void {
    this._stopRingtone();
    if (this._remoteAudio) {
      this._remoteAudio.pause();
      this._remoteAudio.srcObject = null;
      this._remoteAudio.remove();
      this._remoteAudio = null;
    }
  }
}

/* ──────────────────────────────────────────────────────────────────────────
 * Register with Home Assistant Lovelace card registry
 * ────────────────────────────────────────────────────────────────────────── */

declare global {
  interface HTMLElementTagNameMap {
    "ha-voip-card": HaVoipCard;
    "ha-voip-card-editor": HaVoipCardEditor;
  }

  interface Window {
    customCards?: Array<{
      type: string;
      name: string;
      description: string;
      preview?: boolean;
    }>;
  }
}

// Register as a custom Lovelace card
window.customCards = window.customCards || [];
window.customCards.push({
  type: "ha-voip-card",
  name: "VoIP Phone Card",
  description:
    "A full-featured VoIP calling interface for Home Assistant with WebRTC support, dialpad, call history, and doorbell camera integration.",
  preview: true,
});
