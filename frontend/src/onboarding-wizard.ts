/* ──────────────────────────────────────────────────────────────────────────────
 * onboarding-wizard.ts — Setup wizard for HA VoIP
 *
 * Steps:
 *   1. Welcome & microphone permissions
 *   2. Network test (WSS / STUN / TURN connectivity)
 *   3. Mode selection (Local / Federated)
 *   4. Extension assignment (map HA users to extensions)
 *   5. Certificate setup (auto / manual / self-signed)
 *   6. Test call (loopback demo)
 *
 * Also supports a two-screen "condensed" mode for users who accept defaults.
 * ────────────────────────────────────────────────────────────────────────── */

import { LitElement, html, css, nothing, type PropertyValues } from "lit";
import { customElement, property, state } from "lit/decorators.js";
import {
  cardStyles,
  buttonStyles,
  wizardStyles,
  formStyles,
  diagnosticsStyles,
  responsiveStyles,
} from "./styles";
import { localize } from "./localize";
import type {
  HomeAssistant,
  OnboardingStep,
  OnboardingConfig,
  VoipMode,
  CertificateMode,
  ExtensionConfig,
  DiagnosticResult,
  DiagnosticTestStatus,
} from "./types";
import { fireEvent } from "./types";
import { WebRtcManager } from "./webrtc-manager";

const STEPS: OnboardingStep[] = [
  "welcome",
  "network_test",
  "mode_selection",
  "extension_assignment",
  "certificate_setup",
  "test_call",
];

@customElement("ha-voip-onboarding")
export class HaVoipOnboarding extends LitElement {
  /* ── Properties ────────────────────────────────────────────────────────── */

  @property({ attribute: false }) hass?: HomeAssistant;

  @state() private _currentStep: OnboardingStep = "welcome";
  @state() private _condensedMode = false;

  /* Step 1: Welcome */
  @state() private _micPermission: "prompt" | "granted" | "denied" = "prompt";

  /* Step 2: Network test */
  @state() private _networkTests: DiagnosticResult[] = [];
  @state() private _networkTestRunning = false;

  /* Step 3: Mode */
  @state() private _selectedMode: VoipMode = "local";

  /* Step 4: Extensions */
  @state() private _extensions: ExtensionConfig[] = [];

  /* Step 5: Certificate */
  @state() private _certMode: CertificateMode = "auto";
  @state() private _certPath = "";

  /* Step 6: Test call */
  @state() private _testCallState: "idle" | "testing" | "success" | "failure" = "idle";

  private _webrtc: WebRtcManager | null = null;

  /* ── Styles ────────────────────────────────────────────────────────────── */

  static styles = [
    cardStyles,
    buttonStyles,
    wizardStyles,
    formStyles,
    diagnosticsStyles,
    responsiveStyles,
    css`
      :host {
        display: block;
      }

      .condensed-choice {
        display: flex;
        gap: 12px;
        margin-bottom: 16px;
      }

      .condensed-choice button {
        flex: 1;
      }

      .ext-row {
        display: flex;
        gap: 8px;
        align-items: center;
        margin-bottom: 8px;
      }

      .ext-row .form-input {
        flex: 1;
      }

      .ext-row .ext-number {
        width: 80px;
        flex: none;
      }

      .test-call-result {
        text-align: center;
        padding: 24px 0;
      }

      .test-call-result__icon {
        font-size: 48px;
        margin-bottom: 12px;
      }

      .test-call-result__message {
        font-size: 14px;
        color: var(--voip-secondary-text);
        line-height: 1.5;
      }

      .test-call-result--success .test-call-result__icon {
        color: var(--voip-success);
      }

      .test-call-result--failure .test-call-result__icon {
        color: var(--voip-error);
      }
    `,
  ];

  /* ── Lifecycle ─────────────────────────────────────────────────────────── */

  disconnectedCallback(): void {
    super.disconnectedCallback();
    if (this._webrtc) {
      this._webrtc.hangup();
      this._webrtc = null;
    }
  }

  /* ── Render ────────────────────────────────────────────────────────────── */

  protected render() {
    const stepIndex = STEPS.indexOf(this._currentStep);
    const totalSteps = STEPS.length;

    return html`
      <div class="wizard">
        <!-- Progress indicator -->
        ${!this._condensedMode
          ? html`
              <div class="wizard-progress" role="progressbar" aria-valuenow=${stepIndex + 1} aria-valuemax=${totalSteps}>
                ${STEPS.map((_, i) => {
                  let cls = "wizard-progress__step";
                  if (i < stepIndex) cls += " wizard-progress__step--completed";
                  else if (i === stepIndex) cls += " wizard-progress__step--active";
                  return html`<div class=${cls}></div>`;
                })}
              </div>
            `
          : nothing}

        <!-- Step content -->
        ${this._renderStep()}

        <!-- Navigation -->
        ${this._renderNav()}
      </div>
    `;
  }

  private _renderStep() {
    if (this._condensedMode) {
      return this._renderCondensedStep();
    }

    switch (this._currentStep) {
      case "welcome":
        return this._renderWelcome();
      case "network_test":
        return this._renderNetworkTest();
      case "mode_selection":
        return this._renderModeSelection();
      case "extension_assignment":
        return this._renderExtensionAssignment();
      case "certificate_setup":
        return this._renderCertificateSetup();
      case "test_call":
        return this._renderTestCall();
      default:
        return nothing;
    }
  }

  /* ── Step 1: Welcome ───────────────────────────────────────────────────── */

  private _renderWelcome() {
    return html`
      <h2 class="wizard-title">${localize("onboarding.step1.title", this.hass)}</h2>
      <p class="wizard-subtitle">${localize("onboarding.step1.subtitle", this.hass)}</p>

      <!-- Condensed vs full choice -->
      <div class="condensed-choice">
        <button class="wizard-btn wizard-btn--primary" @click=${() => { this._condensedMode = false; }}>
          ${localize("onboarding.full", this.hass)}
        </button>
        <button class="wizard-btn wizard-btn--secondary" @click=${this._startCondensed}>
          ${localize("onboarding.condensed", this.hass)}
        </button>
      </div>

      <!-- Microphone permission -->
      <div class="form-group">
        ${this._micPermission === "granted"
          ? html`
              <div style="display:flex;align-items:center;gap:8px;color:var(--voip-success)">
                <svg viewBox="0 0 24 24" width="24" height="24">
                  <path fill="currentColor" d="M21,7L9,19L3.5,13.5L4.91,12.09L9,16.17L19.59,5.59L21,7Z" />
                </svg>
                <span>${localize("onboarding.step1.mic_granted", this.hass)}</span>
              </div>
            `
          : this._micPermission === "denied"
            ? html`
                <div style="display:flex;align-items:center;gap:8px;color:var(--voip-error)">
                  <svg viewBox="0 0 24 24" width="24" height="24">
                    <path fill="currentColor" d="M19,6.41L17.59,5L12,10.59L6.41,5L5,6.41L10.59,12L5,17.59L6.41,19L12,13.41L17.59,19L19,17.59L13.41,12L19,6.41Z" />
                  </svg>
                  <span>${localize("onboarding.step1.mic_denied", this.hass)}</span>
                </div>
              `
            : html`
                <button class="wizard-btn wizard-btn--primary" @click=${this._requestMicrophone}>
                  <svg viewBox="0 0 24 24" width="18" height="18" style="margin-right:6px">
                    <path fill="currentColor" d="M12,2A3,3 0 0,1 15,5V11A3,3 0 0,1 12,14A3,3 0 0,1 9,11V5A3,3 0 0,1 12,2M19,11C19,14.53 16.39,17.44 13,17.93V21H11V17.93C7.61,17.44 5,14.53 5,11H7A5,5 0 0,0 12,16A5,5 0 0,0 17,11H19Z" />
                  </svg>
                  ${localize("onboarding.step1.request_mic", this.hass)}
                </button>
              `}
      </div>
    `;
  }

  /* ── Step 2: Network test ──────────────────────────────────────────────── */

  private _renderNetworkTest() {
    return html`
      <h2 class="wizard-title">${localize("onboarding.step2.title", this.hass)}</h2>
      <p class="wizard-subtitle">${localize("onboarding.step2.subtitle", this.hass)}</p>

      <div class="diag-table">
        ${this._networkTests.map(
          (test) => html`
            <div class="diag-row">
              <div class="diag-icon diag-icon--${test.status}">
                ${this._renderStatusIcon(test.status)}
              </div>
              <div class="diag-info">
                <div class="diag-name">${test.name}</div>
                ${test.message ? html`<div class="diag-message">${test.message}</div>` : nothing}
              </div>
              ${test.durationMs != null
                ? html`<div class="diag-time">${test.durationMs}ms</div>`
                : nothing}
            </div>
          `,
        )}
      </div>

      ${!this._networkTestRunning && this._networkTests.length === 0
        ? html`
            <button class="wizard-btn wizard-btn--primary" @click=${this._runNetworkTests}>
              ${localize("diag.run_all", this.hass)}
            </button>
          `
        : nothing}
      ${this._networkTestRunning
        ? html`<p style="text-align:center;color:var(--voip-secondary-text)">${localize("onboarding.step2.running", this.hass)}</p>`
        : nothing}
    `;
  }

  /* ── Step 3: Mode selection ────────────────────────────────────────────── */

  private _renderModeSelection() {
    return html`
      <h2 class="wizard-title">${localize("onboarding.step3.title", this.hass)}</h2>
      <p class="wizard-subtitle">${localize("onboarding.step3.subtitle", this.hass)}</p>

      <div class="form-radio-group">
        <label class="form-radio ${this._selectedMode === "local" ? "form-radio--selected" : ""}">
          <input
            type="radio"
            name="mode"
            value="local"
            .checked=${this._selectedMode === "local"}
            @change=${() => { this._selectedMode = "local"; }}
          />
          <div>
            <div class="form-radio__label">${localize("onboarding.step3.local", this.hass)}</div>
            <div class="form-radio__description">${localize("onboarding.step3.local_desc", this.hass)}</div>
          </div>
        </label>
        <label class="form-radio ${this._selectedMode === "federated" ? "form-radio--selected" : ""}">
          <input
            type="radio"
            name="mode"
            value="federated"
            .checked=${this._selectedMode === "federated"}
            @change=${() => { this._selectedMode = "federated"; }}
          />
          <div>
            <div class="form-radio__label">${localize("onboarding.step3.federated", this.hass)}</div>
            <div class="form-radio__description">${localize("onboarding.step3.federated_desc", this.hass)}</div>
          </div>
        </label>
      </div>
    `;
  }

  /* ── Step 4: Extension assignment ──────────────────────────────────────── */

  private _renderExtensionAssignment() {
    return html`
      <h2 class="wizard-title">${localize("onboarding.step4.title", this.hass)}</h2>
      <p class="wizard-subtitle">${localize("onboarding.step4.subtitle", this.hass)}</p>

      ${this._extensions.map(
        (ext, idx) => html`
          <div class="ext-row">
            <input
              class="form-input"
              type="text"
              placeholder=${localize("onboarding.step4.user", this.hass)}
              .value=${ext.name}
              @input=${(e: InputEvent) => this._updateExtName(idx, (e.target as HTMLInputElement).value)}
            />
            <input
              class="form-input ext-number"
              type="tel"
              placeholder="100"
              .value=${ext.number}
              @input=${(e: InputEvent) => this._updateExtNumber(idx, (e.target as HTMLInputElement).value)}
            />
            <button
              class="btn btn--sm btn--icon"
              @click=${() => this._removeExtension(idx)}
              aria-label=${localize("config.remove", this.hass)}
            >
              <svg viewBox="0 0 24 24" width="18" height="18">
                <path fill="currentColor" d="M19,6.41L17.59,5L12,10.59L6.41,5L5,6.41L10.59,12L5,17.59L6.41,19L12,13.41L17.59,19L19,17.59L13.41,12L19,6.41Z" />
              </svg>
            </button>
          </div>
        `,
      )}

      <button class="wizard-btn wizard-btn--secondary" @click=${this._addExtension}>
        + ${localize("onboarding.step4.add", this.hass)}
      </button>
    `;
  }

  /* ── Step 5: Certificate setup ─────────────────────────────────────────── */

  private _renderCertificateSetup() {
    return html`
      <h2 class="wizard-title">${localize("onboarding.step5.title", this.hass)}</h2>
      <p class="wizard-subtitle">${localize("onboarding.step5.subtitle", this.hass)}</p>

      <div class="form-radio-group">
        <label class="form-radio ${this._certMode === "auto" ? "form-radio--selected" : ""}">
          <input
            type="radio"
            name="cert"
            value="auto"
            .checked=${this._certMode === "auto"}
            @change=${() => { this._certMode = "auto"; }}
          />
          <div>
            <div class="form-radio__label">${localize("onboarding.step5.auto", this.hass)}</div>
            <div class="form-radio__description">${localize("onboarding.step5.auto_desc", this.hass)}</div>
          </div>
        </label>

        <label class="form-radio ${this._certMode === "manual" ? "form-radio--selected" : ""}">
          <input
            type="radio"
            name="cert"
            value="manual"
            .checked=${this._certMode === "manual"}
            @change=${() => { this._certMode = "manual"; }}
          />
          <div>
            <div class="form-radio__label">${localize("onboarding.step5.manual", this.hass)}</div>
            <div class="form-radio__description">${localize("onboarding.step5.manual_desc", this.hass)}</div>
          </div>
        </label>

        <label class="form-radio ${this._certMode === "self_signed" ? "form-radio--selected" : ""}">
          <input
            type="radio"
            name="cert"
            value="self_signed"
            .checked=${this._certMode === "self_signed"}
            @change=${() => { this._certMode = "self_signed"; }}
          />
          <div>
            <div class="form-radio__label">${localize("onboarding.step5.self_signed", this.hass)}</div>
            <div class="form-radio__description">${localize("onboarding.step5.self_signed_desc", this.hass)}</div>
          </div>
        </label>
      </div>

      ${this._certMode === "manual"
        ? html`
            <div class="form-group" style="margin-top:12px">
              <label class="form-label">Certificate path</label>
              <input
                class="form-input"
                type="text"
                placeholder="/ssl/fullchain.pem"
                .value=${this._certPath}
                @input=${(e: InputEvent) => { this._certPath = (e.target as HTMLInputElement).value; }}
              />
            </div>
          `
        : nothing}
    `;
  }

  /* ── Step 6: Test call ─────────────────────────────────────────────────── */

  private _renderTestCall() {
    return html`
      <h2 class="wizard-title">${localize("onboarding.step6.title", this.hass)}</h2>
      <p class="wizard-subtitle">${localize("onboarding.step6.subtitle", this.hass)}</p>

      ${this._testCallState === "idle"
        ? html`
            <div style="text-align:center">
              <button class="wizard-btn wizard-btn--primary" @click=${this._startTestCall}>
                ${localize("onboarding.step6.start", this.hass)}
              </button>
            </div>
          `
        : nothing}

      ${this._testCallState === "testing"
        ? html`
            <div class="test-call-result">
              <div class="diag-icon diag-icon--running" style="width:48px;height:48px;margin:0 auto 12px;font-size:24px;">
                <svg viewBox="0 0 24 24" width="24" height="24">
                  <path fill="currentColor" d="M12,4V2A10,10 0 0,0 2,12H4A8,8 0 0,1 12,4Z" />
                </svg>
              </div>
              <p class="test-call-result__message">${localize("onboarding.step6.testing", this.hass)}</p>
            </div>
          `
        : nothing}

      ${this._testCallState === "success"
        ? html`
            <div class="test-call-result test-call-result--success">
              <div class="test-call-result__icon">
                <svg viewBox="0 0 24 24" width="48" height="48">
                  <path fill="currentColor" d="M21,7L9,19L3.5,13.5L4.91,12.09L9,16.17L19.59,5.59L21,7Z" />
                </svg>
              </div>
              <p class="test-call-result__message">${localize("onboarding.step6.success", this.hass)}</p>
            </div>
          `
        : nothing}

      ${this._testCallState === "failure"
        ? html`
            <div class="test-call-result test-call-result--failure">
              <div class="test-call-result__icon">
                <svg viewBox="0 0 24 24" width="48" height="48">
                  <path fill="currentColor" d="M19,6.41L17.59,5L12,10.59L6.41,5L5,6.41L10.59,12L5,17.59L6.41,19L12,13.41L17.59,19L19,17.59L13.41,12L19,6.41Z" />
                </svg>
              </div>
              <p class="test-call-result__message">${localize("onboarding.step6.failure", this.hass)}</p>
            </div>
          `
        : nothing}
    `;
  }

  /* ── Condensed mode ────────────────────────────────────────────────────── */

  private _renderCondensedStep() {
    // Screen 1: confirm defaults
    if (this._currentStep === "welcome") {
      return html`
        <h2 class="wizard-title">${localize("onboarding.condensed", this.hass)}</h2>
        <p class="wizard-subtitle">
          The following defaults will be applied:
        </p>
        <ul style="font-size:14px;line-height:2;color:var(--voip-secondary-text)">
          <li>Mode: <strong>Local</strong></li>
          <li>Certificate: <strong>Automatic (Let's Encrypt)</strong></li>
          <li>Extensions: <strong>Auto-assigned from HA users</strong></li>
        </ul>

        <div class="form-group">
          ${this._micPermission !== "granted"
            ? html`
                <button class="wizard-btn wizard-btn--primary" @click=${this._requestMicrophone}>
                  ${localize("onboarding.step1.request_mic", this.hass)}
                </button>
              `
            : html`
                <div style="display:flex;align-items:center;gap:8px;color:var(--voip-success)">
                  <svg viewBox="0 0 24 24" width="20" height="20">
                    <path fill="currentColor" d="M21,7L9,19L3.5,13.5L4.91,12.09L9,16.17L19.59,5.59L21,7Z" />
                  </svg>
                  <span>${localize("onboarding.step1.mic_granted", this.hass)}</span>
                </div>
              `}
        </div>
      `;
    }

    // Screen 2: test call
    return this._renderTestCall();
  }

  /* ── Navigation ────────────────────────────────────────────────────────── */

  private _renderNav() {
    const stepIndex = STEPS.indexOf(this._currentStep);
    const isLast = stepIndex === STEPS.length - 1;
    const isFirst = stepIndex === 0;

    if (this._condensedMode) {
      if (this._currentStep === "welcome") {
        return html`
          <div class="wizard-actions">
            <button class="wizard-btn wizard-btn--secondary" @click=${() => { this._condensedMode = false; }}>
              ${localize("onboarding.full", this.hass)}
            </button>
            <button
              class="wizard-btn wizard-btn--primary"
              ?disabled=${this._micPermission !== "granted"}
              @click=${() => { this._currentStep = "test_call"; }}
            >
              ${localize("onboarding.next", this.hass)}
            </button>
          </div>
        `;
      }

      return html`
        <div class="wizard-actions">
          <button class="wizard-btn wizard-btn--secondary" @click=${() => { this._currentStep = "welcome"; }}>
            ${localize("onboarding.back", this.hass)}
          </button>
          <button class="wizard-btn wizard-btn--primary" @click=${this._finishSetup}>
            ${localize("onboarding.finish", this.hass)}
          </button>
        </div>
      `;
    }

    return html`
      <div class="wizard-actions">
        <div>
          ${!isFirst
            ? html`
                <button class="wizard-btn wizard-btn--secondary" @click=${this._prevStep}>
                  ${localize("onboarding.back", this.hass)}
                </button>
              `
            : html`
                <button class="wizard-btn wizard-btn--secondary" @click=${this._skipAll}>
                  ${localize("onboarding.skip", this.hass)}
                </button>
              `}
        </div>
        <div>
          ${isLast
            ? html`
                <button class="wizard-btn wizard-btn--primary" @click=${this._finishSetup}>
                  ${localize("onboarding.finish", this.hass)}
                </button>
              `
            : html`
                <button class="wizard-btn wizard-btn--primary" @click=${this._nextStep}>
                  ${localize("onboarding.next", this.hass)}
                </button>
              `}
        </div>
      </div>
    `;
  }

  /* ── Step actions ──────────────────────────────────────────────────────── */

  private _nextStep(): void {
    const idx = STEPS.indexOf(this._currentStep);
    if (idx < STEPS.length - 1) {
      this._currentStep = STEPS[idx + 1];
    }
  }

  private _prevStep(): void {
    const idx = STEPS.indexOf(this._currentStep);
    if (idx > 0) {
      this._currentStep = STEPS[idx - 1];
    }
  }

  private _skipAll(): void {
    this._finishSetup();
  }

  private _startCondensed(): void {
    this._condensedMode = true;
    this._selectedMode = "local";
    this._certMode = "auto";
    this._requestMicrophone();
  }

  private async _requestMicrophone(): Promise<void> {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      stream.getTracks().forEach((t) => t.stop());
      this._micPermission = "granted";
    } catch {
      this._micPermission = "denied";
    }
  }

  /* ── Network tests ─────────────────────────────────────────────────────── */

  private async _runNetworkTests(): Promise<void> {
    this._networkTestRunning = true;
    this._networkTests = [
      { name: localize("diag.wss", this.hass), status: "running" },
      { name: localize("diag.stun", this.hass), status: "pending" },
      { name: localize("diag.turn", this.hass), status: "pending" },
    ];

    // Test 1: WSS
    await this._testWss();

    // Test 2: STUN
    this._networkTests = this._networkTests.map((t, i) =>
      i === 1 ? { ...t, status: "running" as DiagnosticTestStatus } : t,
    );
    await this._testStun();

    // Test 3: TURN
    this._networkTests = this._networkTests.map((t, i) =>
      i === 2 ? { ...t, status: "running" as DiagnosticTestStatus } : t,
    );
    await this._testTurn();

    this._networkTestRunning = false;
  }

  private async _testWss(): Promise<void> {
    const start = performance.now();
    try {
      if (this.hass?.connection?.socket?.readyState === WebSocket.OPEN) {
        const duration = Math.round(performance.now() - start);
        this._networkTests = this._networkTests.map((t, i) =>
          i === 0
            ? { ...t, status: "pass" as DiagnosticTestStatus, message: "WebSocket connected", durationMs: duration }
            : t,
        );
      } else {
        throw new Error("Socket not open");
      }
    } catch {
      const duration = Math.round(performance.now() - start);
      this._networkTests = this._networkTests.map((t, i) =>
        i === 0
          ? { ...t, status: "fail" as DiagnosticTestStatus, message: "WebSocket not available", durationMs: duration }
          : t,
      );
    }
  }

  private async _testStun(): Promise<void> {
    const start = performance.now();
    try {
      const pc = new RTCPeerConnection({
        iceServers: [{ urls: "stun:stun.l.google.com:19302" }],
      });

      const gotCandidate = new Promise<boolean>((resolve) => {
        const timeout = setTimeout(() => resolve(false), 5000);
        pc.onicecandidate = (e) => {
          if (e.candidate && e.candidate.type === "srflx") {
            clearTimeout(timeout);
            resolve(true);
          }
        };
      });

      pc.addTransceiver("audio", { direction: "sendrecv" });
      const offer = await pc.createOffer();
      await pc.setLocalDescription(offer);

      const result = await gotCandidate;
      pc.close();

      const duration = Math.round(performance.now() - start);
      this._networkTests = this._networkTests.map((t, i) =>
        i === 1
          ? {
              ...t,
              status: result ? ("pass" as DiagnosticTestStatus) : ("warning" as DiagnosticTestStatus),
              message: result
                ? "STUN server reachable, server-reflexive candidates gathered"
                : "No server-reflexive candidates (may be behind symmetric NAT)",
              durationMs: duration,
            }
          : t,
      );
    } catch (err) {
      const duration = Math.round(performance.now() - start);
      this._networkTests = this._networkTests.map((t, i) =>
        i === 1
          ? { ...t, status: "fail" as DiagnosticTestStatus, message: String(err), durationMs: duration }
          : t,
      );
    }
  }

  private async _testTurn(): Promise<void> {
    const start = performance.now();
    // For the onboarding wizard, we do a basic check. A real TURN test
    // would require valid credentials from the backend.
    try {
      if (this.hass) {
        // Ask backend for TURN credentials
        const turnInfo = await this.hass.callWS<{
          urls: string[];
          username: string;
          credential: string;
        }>({ type: "voip/diagnostics", test: "turn_credentials" });

        if (turnInfo?.urls?.length) {
          const duration = Math.round(performance.now() - start);
          this._networkTests = this._networkTests.map((t, i) =>
            i === 2
              ? {
                  ...t,
                  status: "pass" as DiagnosticTestStatus,
                  message: `TURN server configured: ${turnInfo.urls[0]}`,
                  durationMs: duration,
                }
              : t,
          );
          return;
        }
      }
      throw new Error("No TURN configuration available");
    } catch {
      const duration = Math.round(performance.now() - start);
      this._networkTests = this._networkTests.map((t, i) =>
        i === 2
          ? {
              ...t,
              status: "warning" as DiagnosticTestStatus,
              message: "TURN not configured — calls may fail behind strict NAT",
              durationMs: duration,
            }
          : t,
      );
    }
  }

  /* ── Extensions ────────────────────────────────────────────────────────── */

  private _addExtension(): void {
    const nextNum = String(100 + this._extensions.length);
    this._extensions = [...this._extensions, { name: "", number: nextNum }];
  }

  private _removeExtension(idx: number): void {
    this._extensions = this._extensions.filter((_, i) => i !== idx);
  }

  private _updateExtName(idx: number, value: string): void {
    this._extensions = this._extensions.map((ext, i) =>
      i === idx ? { ...ext, name: value } : ext,
    );
  }

  private _updateExtNumber(idx: number, value: string): void {
    this._extensions = this._extensions.map((ext, i) =>
      i === idx ? { ...ext, number: value } : ext,
    );
  }

  /* ── Test call ─────────────────────────────────────────────────────────── */

  private async _startTestCall(): Promise<void> {
    this._testCallState = "testing";

    try {
      if (!this.hass) throw new Error("No HA connection");

      // Request a loopback test from the backend
      const result = await this.hass.callWS<{ success: boolean }>({
        type: "voip/onboarding",
        action: "test_call",
      });

      this._testCallState = result?.success ? "success" : "failure";
    } catch {
      this._testCallState = "failure";
    }
  }

  /* ── Finish ────────────────────────────────────────────────────────────── */

  private async _finishSetup(): Promise<void> {
    const config: OnboardingConfig = {
      mode: this._selectedMode,
      extensions: this._extensions.filter((e) => e.name && e.number),
      certificateMode: this._certMode,
      certificatePath: this._certMode === "manual" ? this._certPath : undefined,
      stunServers: ["stun:stun.l.google.com:19302"],
      turnServers: [],
      completed: true,
    };

    // Persist to backend
    if (this.hass) {
      try {
        await this.hass.callWS({
          type: "voip/onboarding",
          action: "save_config",
          config,
        });
      } catch (err) {
        console.error("[Onboarding] Failed to save config:", err);
      }
    }

    fireEvent(this, "voip-onboarding-complete", { config });
  }

  /* ── Helpers ───────────────────────────────────────────────────────────── */

  private _renderStatusIcon(status: DiagnosticTestStatus) {
    switch (status) {
      case "pass":
        return html`<svg viewBox="0 0 24 24" width="16" height="16"><path fill="currentColor" d="M21,7L9,19L3.5,13.5L4.91,12.09L9,16.17L19.59,5.59L21,7Z"/></svg>`;
      case "fail":
        return html`<svg viewBox="0 0 24 24" width="16" height="16"><path fill="currentColor" d="M19,6.41L17.59,5L12,10.59L6.41,5L5,6.41L10.59,12L5,17.59L6.41,19L12,13.41L17.59,19L19,17.59L13.41,12L19,6.41Z"/></svg>`;
      case "warning":
        return html`<svg viewBox="0 0 24 24" width="16" height="16"><path fill="currentColor" d="M13,14H11V10H13M13,18H11V16H13M1,21H23L12,2L1,21Z"/></svg>`;
      case "running":
        return html`<svg viewBox="0 0 24 24" width="16" height="16"><path fill="currentColor" d="M12,4V2A10,10 0 0,0 2,12H4A8,8 0 0,1 12,4Z"/></svg>`;
      case "pending":
      default:
        return html`<svg viewBox="0 0 24 24" width="16" height="16"><path fill="currentColor" d="M12,20A8,8 0 0,1 4,12A8,8 0 0,1 12,4A8,8 0 0,1 20,12A8,8 0 0,1 12,20M12,2A10,10 0 0,0 2,12A10,10 0 0,0 12,22A10,10 0 0,0 22,12A10,10 0 0,0 12,2Z"/></svg>`;
    }
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "ha-voip-onboarding": HaVoipOnboarding;
  }
}
