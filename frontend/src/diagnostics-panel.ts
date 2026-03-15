/* ──────────────────────────────────────────────────────────────────────────────
 * diagnostics-panel.ts — Network diagnostics component for HA VoIP
 *
 * Features:
 *   - WSS connection test with timing
 *   - STUN test (gather ICE candidates, display results)
 *   - TURN allocation test
 *   - Network RTT measurement
 *   - One-way audio test tool
 *   - ICE candidate display
 *   - Export diagnostics as JSON
 *   - Visual pass / fail indicators
 * ────────────────────────────────────────────────────────────────────────── */

import { LitElement, html, css, nothing } from "lit";
import { customElement, property, state } from "lit/decorators.js";
import {
  cardStyles,
  buttonStyles,
  diagnosticsStyles,
  responsiveStyles,
} from "./styles";
import { localize } from "./localize";
import type {
  HomeAssistant,
  DiagnosticResult,
  DiagnosticTestStatus,
  DiagnosticsReport,
  IceCandidateInfo,
} from "./types";
import { WebRtcManager } from "./webrtc-manager";

@customElement("ha-voip-diagnostics")
export class HaVoipDiagnostics extends LitElement {
  /* ── Properties ────────────────────────────────────────────────────────── */

  @property({ attribute: false }) hass?: HomeAssistant;

  @state() private _results: DiagnosticResult[] = [];
  @state() private _iceCandidates: IceCandidateInfo[] = [];
  @state() private _isRunning = false;
  @state() private _networkRtt: number | null = null;
  @state() private _showCandidates = false;
  @state() private _oneWayAudioResult: string | null = null;

  private _webrtc = new WebRtcManager();

  /* ── Styles ────────────────────────────────────────────────────────────── */

  static styles = [
    cardStyles,
    buttonStyles,
    diagnosticsStyles,
    responsiveStyles,
    css`
      :host {
        display: block;
      }

      .diag-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 16px;
        border-bottom: 1px solid var(--voip-divider);
      }

      .diag-header__title {
        font-size: 16px;
        font-weight: 500;
        margin: 0;
      }

      .diag-header__actions {
        display: flex;
        gap: 8px;
      }

      .diag-btn {
        display: inline-flex;
        align-items: center;
        gap: 6px;
        padding: 6px 14px;
        border: 1px solid var(--voip-divider);
        border-radius: 8px;
        background: none;
        font-size: 13px;
        font-family: inherit;
        color: var(--voip-primary-text);
        cursor: pointer;
        transition: background-color 0.15s;
      }

      .diag-btn:hover {
        background-color: rgba(0, 0, 0, 0.04);
      }

      .diag-btn--primary {
        background-color: var(--voip-primary);
        border-color: var(--voip-primary);
        color: #fff;
      }

      .diag-btn--primary:hover {
        filter: brightness(1.1);
      }

      .diag-btn:disabled {
        opacity: 0.5;
        cursor: not-allowed;
      }

      .candidates-section {
        padding: 12px 16px;
        border-top: 1px solid var(--voip-divider);
      }

      .candidates-toggle {
        display: flex;
        align-items: center;
        gap: 8px;
        cursor: pointer;
        background: none;
        border: none;
        font-size: 13px;
        font-weight: 500;
        color: var(--voip-primary);
        padding: 0;
        font-family: inherit;
      }

      .candidates-table {
        width: 100%;
        margin-top: 8px;
        font-size: 12px;
        border-collapse: collapse;
      }

      .candidates-table th {
        text-align: left;
        padding: 6px 8px;
        color: var(--voip-secondary-text);
        font-weight: 500;
        border-bottom: 1px solid var(--voip-divider);
      }

      .candidates-table td {
        padding: 6px 8px;
        border-bottom: 1px solid var(--voip-divider);
        font-family: monospace;
        font-size: 11px;
      }

      .rtt-display {
        text-align: center;
        padding: 16px;
        border-top: 1px solid var(--voip-divider);
      }

      .rtt-value {
        font-size: 32px;
        font-weight: 300;
        color: var(--voip-primary);
      }

      .rtt-label {
        font-size: 12px;
        color: var(--voip-secondary-text);
        margin-top: 4px;
      }

      .one-way-audio-section {
        padding: 12px 16px;
        border-top: 1px solid var(--voip-divider);
      }

      .one-way-audio-result {
        margin-top: 8px;
        padding: 8px 12px;
        border-radius: 6px;
        font-size: 13px;
        background-color: rgba(0, 0, 0, 0.04);
      }
    `,
  ];

  /* ── Render ────────────────────────────────────────────────────────────── */

  protected render() {
    return html`
      <!-- Header with actions -->
      <div class="diag-header">
        <h3 class="diag-header__title">${localize("diag.title", this.hass)}</h3>
        <div class="diag-header__actions">
          <button
            class="diag-btn diag-btn--primary"
            ?disabled=${this._isRunning}
            @click=${this._runAllTests}
          >
            ${this._isRunning
              ? html`<svg viewBox="0 0 24 24" width="14" height="14" style="animation:spin 1s linear infinite"><path fill="currentColor" d="M12,4V2A10,10 0 0,0 2,12H4A8,8 0 0,1 12,4Z"/></svg>`
              : nothing}
            ${localize("diag.run_all", this.hass)}
          </button>
          <button
            class="diag-btn"
            @click=${this._exportJson}
            ?disabled=${this._results.length === 0}
          >
            ${localize("diag.export", this.hass)}
          </button>
        </div>
      </div>

      <!-- Test results -->
      <div class="diag-table">
        ${this._results.map(
          (test) => html`
            <div class="diag-row">
              <div class="diag-icon diag-icon--${test.status}">
                ${this._renderStatusIcon(test.status)}
              </div>
              <div class="diag-info">
                <div class="diag-name">${test.name}</div>
                ${test.message
                  ? html`<div class="diag-message">${test.message}</div>`
                  : nothing}
                ${test.details
                  ? html`<div class="diag-message" style="margin-top:4px;font-family:monospace;font-size:11px">${test.details}</div>`
                  : nothing}
              </div>
              ${test.durationMs != null
                ? html`<div class="diag-time">${test.durationMs}ms</div>`
                : nothing}
            </div>
          `,
        )}
      </div>

      <!-- RTT display -->
      ${this._networkRtt != null
        ? html`
            <div class="rtt-display">
              <div class="rtt-value">${this._networkRtt}<span style="font-size:14px">ms</span></div>
              <div class="rtt-label">${localize("diag.rtt", this.hass)}</div>
            </div>
          `
        : nothing}

      <!-- ICE candidates section -->
      ${this._iceCandidates.length > 0
        ? html`
            <div class="candidates-section">
              <button class="candidates-toggle" @click=${() => { this._showCandidates = !this._showCandidates; }}>
                <svg viewBox="0 0 24 24" width="16" height="16" style="transform:rotate(${this._showCandidates ? 90 : 0}deg);transition:transform 0.2s">
                  <path fill="currentColor" d="M8.59,16.58L13.17,12L8.59,7.41L10,6L16,12L10,18L8.59,16.58Z"/>
                </svg>
                ${localize("diag.ice_candidates", this.hass)} (${this._iceCandidates.length})
              </button>

              ${this._showCandidates
                ? html`
                    <table class="candidates-table">
                      <thead>
                        <tr>
                          <th>Type</th>
                          <th>Protocol</th>
                          <th>Address</th>
                          <th>Port</th>
                          <th>Priority</th>
                        </tr>
                      </thead>
                      <tbody>
                        ${this._iceCandidates.map(
                          (c) => html`
                            <tr>
                              <td>${c.type}</td>
                              <td>${c.protocol}</td>
                              <td>${c.address}</td>
                              <td>${c.port}</td>
                              <td>${c.priority}</td>
                            </tr>
                          `,
                        )}
                      </tbody>
                    </table>
                  `
                : nothing}
            </div>
          `
        : nothing}

      <!-- One-way audio test -->
      <div class="one-way-audio-section">
        <button
          class="diag-btn"
          @click=${this._runOneWayAudioTest}
          ?disabled=${this._isRunning}
        >
          ${localize("diag.one_way_audio", this.hass)}
        </button>
        ${this._oneWayAudioResult
          ? html`<div class="one-way-audio-result">${this._oneWayAudioResult}</div>`
          : nothing}
      </div>
    `;
  }

  /* ── Test runner ───────────────────────────────────────────────────────── */

  private async _runAllTests(): Promise<void> {
    this._isRunning = true;
    this._iceCandidates = [];
    this._networkRtt = null;

    this._results = [
      { name: localize("diag.wss", this.hass), status: "running" },
      { name: localize("diag.stun", this.hass), status: "pending" },
      { name: localize("diag.turn", this.hass), status: "pending" },
      { name: localize("diag.rtt", this.hass), status: "pending" },
    ];

    // 1. WSS test
    await this._testWss();

    // 2. STUN test
    this._updateResult(1, { status: "running" });
    await this._testStun();

    // 3. TURN test
    this._updateResult(2, { status: "running" });
    await this._testTurn();

    // 4. RTT test
    this._updateResult(3, { status: "running" });
    await this._testRtt();

    this._isRunning = false;
  }

  private _updateResult(
    idx: number,
    partial: Partial<DiagnosticResult>,
  ): void {
    this._results = this._results.map((r, i) =>
      i === idx ? { ...r, ...partial } : r,
    );
  }

  /* ── Individual tests ──────────────────────────────────────────────────── */

  private async _testWss(): Promise<void> {
    const start = performance.now();

    try {
      if (!this.hass?.connection?.socket) {
        throw new Error("No HA WebSocket connection");
      }

      const socket = this.hass.connection.socket;
      if (socket.readyState !== WebSocket.OPEN) {
        throw new Error(`Socket state: ${socket.readyState}`);
      }

      // Measure round-trip by sending a ping-style WS message
      const pingStart = performance.now();
      await this.hass.callWS({ type: "ping" });
      const pingTime = Math.round(performance.now() - pingStart);

      const duration = Math.round(performance.now() - start);
      this._updateResult(0, {
        status: "pass",
        message: `WebSocket connected (ping: ${pingTime}ms)`,
        details: `URL: ${socket.url}`,
        durationMs: duration,
      });
    } catch (err) {
      const duration = Math.round(performance.now() - start);
      this._updateResult(0, {
        status: "fail",
        message: `WebSocket test failed: ${err instanceof Error ? err.message : String(err)}`,
        durationMs: duration,
      });
    }
  }

  private async _testStun(): Promise<void> {
    const start = performance.now();

    try {
      const candidates = await this._webrtc.gatherIceCandidates(
        ["stun:stun.l.google.com:19302", "stun:stun1.l.google.com:19302"],
        [],
      );

      // Parse candidates into structured info
      const parsed: IceCandidateInfo[] = candidates
        .filter((c) => c.candidate)
        .map((c) => ({
          type: (c.type || "unknown") as RTCIceCandidateType,
          protocol: c.protocol || "unknown",
          address: c.address || "unknown",
          port: c.port || 0,
          priority: c.priority || 0,
          relatedAddress: c.relatedAddress || undefined,
          relatedPort: c.relatedPort || undefined,
        }));

      this._iceCandidates = parsed;

      const hasSrflx = parsed.some((c) => c.type === "srflx");
      const hasHost = parsed.some((c) => c.type === "host");
      const duration = Math.round(performance.now() - start);

      if (hasSrflx) {
        this._updateResult(1, {
          status: "pass",
          message: `Gathered ${candidates.length} candidates (${parsed.filter((c) => c.type === "srflx").length} server-reflexive)`,
          durationMs: duration,
        });
      } else if (hasHost) {
        this._updateResult(1, {
          status: "warning",
          message: `Only host candidates gathered (${candidates.length} total). May be behind symmetric NAT.`,
          durationMs: duration,
        });
      } else {
        this._updateResult(1, {
          status: "fail",
          message: "No ICE candidates gathered",
          durationMs: duration,
        });
      }
    } catch (err) {
      const duration = Math.round(performance.now() - start);
      this._updateResult(1, {
        status: "fail",
        message: `STUN test failed: ${err instanceof Error ? err.message : String(err)}`,
        durationMs: duration,
      });
    }
  }

  private async _testTurn(): Promise<void> {
    const start = performance.now();

    try {
      // Ask the backend for TURN credentials
      if (!this.hass) throw new Error("No HA connection");

      let turnConfig: { urls: string[]; username: string; credential: string };
      try {
        turnConfig = await this.hass.callWS<{
          urls: string[];
          username: string;
          credential: string;
        }>({
          type: "voip/diagnostics",
          test: "turn_credentials",
        });
      } catch {
        throw new Error("Backend did not provide TURN credentials");
      }

      if (!turnConfig?.urls?.length) {
        throw new Error("No TURN server URLs configured");
      }

      // Attempt to gather relay candidates
      const candidates = await this._webrtc.gatherIceCandidates([], [
        {
          urls: turnConfig.urls,
          username: turnConfig.username,
          credential: turnConfig.credential,
        },
      ]);

      const relayCandidates = candidates.filter(
        (c) => c.type === "relay",
      );

      const duration = Math.round(performance.now() - start);

      if (relayCandidates.length > 0) {
        // Add relay candidates to the list
        const parsed: IceCandidateInfo[] = relayCandidates.map((c) => ({
          type: "relay" as RTCIceCandidateType,
          protocol: c.protocol || "unknown",
          address: c.address || "unknown",
          port: c.port || 0,
          priority: c.priority || 0,
          relatedAddress: c.relatedAddress || undefined,
          relatedPort: c.relatedPort || undefined,
        }));
        this._iceCandidates = [...this._iceCandidates, ...parsed];

        this._updateResult(2, {
          status: "pass",
          message: `TURN allocation succeeded (${relayCandidates.length} relay candidates)`,
          details: `Server: ${turnConfig.urls[0]}`,
          durationMs: duration,
        });
      } else {
        this._updateResult(2, {
          status: "fail",
          message: "TURN allocation failed — no relay candidates obtained",
          details: `Server: ${turnConfig.urls[0]}`,
          durationMs: duration,
        });
      }
    } catch (err) {
      const duration = Math.round(performance.now() - start);
      this._updateResult(2, {
        status: "warning",
        message: err instanceof Error ? err.message : String(err),
        durationMs: duration,
      });
    }
  }

  private async _testRtt(): Promise<void> {
    const start = performance.now();

    try {
      if (!this.hass) throw new Error("No HA connection");

      // Measure RTT with multiple pings
      const pings: number[] = [];
      for (let i = 0; i < 5; i++) {
        const pingStart = performance.now();
        await this.hass.callWS({ type: "ping" });
        pings.push(performance.now() - pingStart);
      }

      // Remove outliers (highest and lowest) and average the rest
      pings.sort((a, b) => a - b);
      const trimmed = pings.length > 2 ? pings.slice(1, -1) : pings;
      const avgRtt = Math.round(
        trimmed.reduce((sum, p) => sum + p, 0) / trimmed.length,
      );

      this._networkRtt = avgRtt;
      const duration = Math.round(performance.now() - start);

      let status: DiagnosticTestStatus = "pass";
      let message = `Average RTT: ${avgRtt}ms (${pings.length} samples)`;
      if (avgRtt > 300) {
        status = "fail";
        message += " — latency is too high for real-time voice";
      } else if (avgRtt > 150) {
        status = "warning";
        message += " — latency may cause noticeable delay";
      }

      this._updateResult(3, { status, message, durationMs: duration });
    } catch (err) {
      const duration = Math.round(performance.now() - start);
      this._updateResult(3, {
        status: "fail",
        message: `RTT test failed: ${err instanceof Error ? err.message : String(err)}`,
        durationMs: duration,
      });
    }
  }

  /* ── One-way audio test ────────────────────────────────────────────────── */

  private async _runOneWayAudioTest(): Promise<void> {
    this._oneWayAudioResult = null;

    try {
      // Acquire microphone
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });

      // Set up audio analysis
      const audioCtx = new AudioContext();
      const source = audioCtx.createMediaStreamSource(stream);
      const analyser = audioCtx.createAnalyser();
      analyser.fftSize = 256;
      source.connect(analyser);

      const data = new Uint8Array(analyser.frequencyBinCount);

      // Measure audio level over 3 seconds
      let maxLevel = 0;
      let samples = 0;
      const duration = 3000;
      const interval = 50;

      await new Promise<void>((resolve) => {
        const timer = setInterval(() => {
          analyser.getByteFrequencyData(data);
          let sum = 0;
          for (let i = 0; i < data.length; i++) {
            sum += data[i];
          }
          const avg = sum / data.length;
          if (avg > maxLevel) maxLevel = avg;
          samples++;

          if (samples >= duration / interval) {
            clearInterval(timer);
            resolve();
          }
        }, interval);
      });

      // Cleanup
      stream.getTracks().forEach((t) => t.stop());
      await audioCtx.close();

      // Analyze results
      if (maxLevel > 30) {
        this._oneWayAudioResult = `Microphone is working. Peak audio level: ${Math.round(maxLevel)}/255. Speak to verify your voice is being captured.`;
      } else if (maxLevel > 5) {
        this._oneWayAudioResult = `Microphone detected low audio. Peak level: ${Math.round(maxLevel)}/255. Check your microphone volume.`;
      } else {
        this._oneWayAudioResult = `No audio detected (peak: ${Math.round(maxLevel)}/255). The microphone may be muted or not working.`;
      }
    } catch (err) {
      this._oneWayAudioResult = `Audio test failed: ${err instanceof Error ? err.message : String(err)}`;
    }
  }

  /* ── Export ─────────────────────────────────────────────────────────────── */

  private _exportJson(): void {
    const report: DiagnosticsReport = {
      timestamp: Date.now(),
      userAgent: navigator.userAgent,
      results: this._results,
      iceCandidates: this._iceCandidates,
      networkRtt: this._networkRtt ?? undefined,
    };

    const blob = new Blob([JSON.stringify(report, null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `ha-voip-diagnostics-${new Date().toISOString().slice(0, 10)}.json`;
    a.click();
    URL.revokeObjectURL(url);
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
    "ha-voip-diagnostics": HaVoipDiagnostics;
  }
}
