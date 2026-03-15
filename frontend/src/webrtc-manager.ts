/* ──────────────────────────────────────────────────────────────────────────────
 * webrtc-manager.ts — WebRTC peer-connection manager for HA VoIP
 *
 * Manages the RTCPeerConnection lifecycle, getUserMedia, ICE candidate
 * relay through Home Assistant WebSocket, TURN credential injection,
 * connection-state monitoring, audio-level metering, reconnection logic,
 * and statistics collection.
 * ────────────────────────────────────────────────────────────────────────── */

import type {
  HomeAssistant,
  TurnServerConfig,
  WebRtcConnectionState,
  WebRtcStats,
  AudioDeviceInfo,
} from "./types";

/* ── Callback signatures ─────────────────────────────────────────────────── */

export interface WebRtcCallbacks {
  onConnectionStateChange?: (state: WebRtcConnectionState) => void;
  onRemoteStream?: (stream: MediaStream) => void;
  onAudioLevel?: (level: number) => void;
  onStats?: (stats: WebRtcStats) => void;
  onIceCandidate?: (candidate: RTCIceCandidate) => void;
  onError?: (error: Error) => void;
  onReconnecting?: () => void;
  onReconnected?: () => void;
}

export interface WebRtcManagerConfig {
  stunServers?: string[];
  turnServers?: TurnServerConfig[];
  audioConstraints?: MediaTrackConstraints;
  enableStats?: boolean;
  statsIntervalMs?: number;
  maxReconnectAttempts?: number;
  audioLevelIntervalMs?: number;
}

/* ── Default configuration ───────────────────────────────────────────────── */

const DEFAULT_STUN_SERVERS = ["stun:stun.l.google.com:19302"];
const DEFAULT_STATS_INTERVAL = 2000;
const DEFAULT_AUDIO_LEVEL_INTERVAL = 100;
const MAX_RECONNECT_ATTEMPTS = 3;
const RECONNECT_DELAY_BASE = 1000;

/* ──────────────────────────────────────────────────────────────────────────
 * WebRtcManager
 * ────────────────────────────────────────────────────────────────────────── */

export class WebRtcManager {
  private _pc: RTCPeerConnection | null = null;
  private _localStream: MediaStream | null = null;
  private _remoteStream: MediaStream | null = null;
  private _hass: HomeAssistant | null = null;
  private _callId: string | null = null;

  private _config: Required<WebRtcManagerConfig>;
  private _callbacks: WebRtcCallbacks;

  private _statsTimer: ReturnType<typeof setInterval> | null = null;
  private _audioLevelTimer: ReturnType<typeof setInterval> | null = null;
  private _audioContext: AudioContext | null = null;
  private _analyser: AnalyserNode | null = null;

  private _reconnectAttempts = 0;
  private _connectionState: WebRtcConnectionState = "new";
  private _isMuted = false;

  private _unsubscribe: (() => void) | null = null;

  constructor(callbacks: WebRtcCallbacks = {}, config: WebRtcManagerConfig = {}) {
    this._callbacks = callbacks;
    this._config = {
      stunServers: config.stunServers ?? DEFAULT_STUN_SERVERS,
      turnServers: config.turnServers ?? [],
      audioConstraints: config.audioConstraints ?? {
        echoCancellation: true,
        noiseSuppression: true,
        autoGainControl: true,
      },
      enableStats: config.enableStats ?? true,
      statsIntervalMs: config.statsIntervalMs ?? DEFAULT_STATS_INTERVAL,
      maxReconnectAttempts: config.maxReconnectAttempts ?? MAX_RECONNECT_ATTEMPTS,
      audioLevelIntervalMs: config.audioLevelIntervalMs ?? DEFAULT_AUDIO_LEVEL_INTERVAL,
    };
  }

  /* ── Public API ────────────────────────────────────────────────────────── */

  /** Attach a Home Assistant instance (call whenever hass updates). */
  setHass(hass: HomeAssistant): void {
    this._hass = hass;
  }

  /** Get the current connection state. */
  get connectionState(): WebRtcConnectionState {
    return this._connectionState;
  }

  /** Whether the local microphone is muted. */
  get isMuted(): boolean {
    return this._isMuted;
  }

  /** The active RTCPeerConnection, if any. */
  get peerConnection(): RTCPeerConnection | null {
    return this._pc;
  }

  /** The local media stream, if any. */
  get localStream(): MediaStream | null {
    return this._localStream;
  }

  /** The remote media stream, if any. */
  get remoteStream(): MediaStream | null {
    return this._remoteStream;
  }

  /* ── Device Enumeration ────────────────────────────────────────────────── */

  /** List available audio input and output devices. */
  async enumerateAudioDevices(): Promise<AudioDeviceInfo[]> {
    const devices = await navigator.mediaDevices.enumerateDevices();
    return devices
      .filter((d) => d.kind === "audioinput" || d.kind === "audiooutput")
      .map((d) => ({
        deviceId: d.deviceId,
        label: d.label || `${d.kind} (${d.deviceId.slice(0, 6)})`,
        kind: d.kind as "audioinput" | "audiooutput",
      }));
  }

  /** Request microphone permission and return the stream. */
  async requestMicrophone(deviceId?: string): Promise<MediaStream> {
    const constraints: MediaStreamConstraints = {
      audio: {
        ...this._config.audioConstraints,
        ...(deviceId ? { deviceId: { exact: deviceId } } : {}),
      },
      video: false,
    };
    try {
      const stream = await navigator.mediaDevices.getUserMedia(constraints);
      return stream;
    } catch (err) {
      const error =
        err instanceof Error ? err : new Error("Failed to access microphone");
      this._callbacks.onError?.(error);
      throw error;
    }
  }

  /* ── Connection lifecycle ──────────────────────────────────────────────── */

  /**
   * Initiate an outbound call: acquire mic, create offer, send via HA WS.
   */
  async startCall(callId: string, deviceId?: string): Promise<void> {
    this._callId = callId;
    this._reconnectAttempts = 0;
    await this._createConnection(deviceId);

    const offer = await this._pc!.createOffer();
    await this._pc!.setLocalDescription(offer);

    // Send offer through HA WebSocket
    await this._sendWs({
      type: "voip/webrtc_offer",
      call_id: callId,
      sdp: offer.sdp!,
    });

    // Subscribe to signalling events
    await this._subscribeSignalling();
  }

  /**
   * Accept an inbound call: acquire mic, process remote offer, send answer.
   */
  async answerCall(callId: string, remoteSdp: string, deviceId?: string): Promise<void> {
    this._callId = callId;
    this._reconnectAttempts = 0;
    await this._createConnection(deviceId);

    await this._pc!.setRemoteDescription(
      new RTCSessionDescription({ type: "offer", sdp: remoteSdp }),
    );

    const answer = await this._pc!.createAnswer();
    await this._pc!.setLocalDescription(answer);

    await this._sendWs({
      type: "voip/webrtc_answer",
      call_id: callId,
      sdp: answer.sdp!,
    });

    await this._subscribeSignalling();
  }

  /** Process a remote SDP answer (outbound call flow). */
  async handleRemoteAnswer(sdp: string): Promise<void> {
    if (!this._pc) return;
    await this._pc.setRemoteDescription(
      new RTCSessionDescription({ type: "answer", sdp }),
    );
  }

  /** Add a remote ICE candidate. */
  async addIceCandidate(candidate: RTCIceCandidateInit): Promise<void> {
    if (!this._pc) return;
    try {
      await this._pc.addIceCandidate(new RTCIceCandidate(candidate));
    } catch (err) {
      console.warn("[WebRTC] Failed to add ICE candidate:", err);
    }
  }

  /** Toggle local microphone mute. */
  setMute(muted: boolean): void {
    this._isMuted = muted;
    if (this._localStream) {
      this._localStream.getAudioTracks().forEach((t) => {
        t.enabled = !muted;
      });
    }
  }

  /** Switch to a different audio input device mid-call. */
  async switchAudioInput(deviceId: string): Promise<void> {
    if (!this._pc || !this._localStream) return;

    // Get new stream
    const newStream = await this.requestMicrophone(deviceId);
    const newTrack = newStream.getAudioTracks()[0];
    if (!newTrack) return;

    // Replace track on the peer connection sender
    const sender = this._pc.getSenders().find((s) => s.track?.kind === "audio");
    if (sender) {
      await sender.replaceTrack(newTrack);
    }

    // Stop old tracks
    this._localStream.getAudioTracks().forEach((t) => t.stop());

    // Replace in local stream
    this._localStream.removeTrack(this._localStream.getAudioTracks()[0]);
    this._localStream.addTrack(newTrack);

    // Update audio level analyser
    this._setupAudioLevelMonitor();

    // Honour mute state
    newTrack.enabled = !this._isMuted;
  }

  /** Set the audio output device on a given audio element (if supported). */
  async setAudioOutput(element: HTMLAudioElement, deviceId: string): Promise<void> {
    if (typeof (element as any).setSinkId === "function") {
      await (element as any).setSinkId(deviceId);
    }
  }

  /** Tear down the connection completely. */
  async hangup(): Promise<void> {
    this._stopTimers();

    if (this._unsubscribe) {
      this._unsubscribe();
      this._unsubscribe = null;
    }

    if (this._localStream) {
      this._localStream.getTracks().forEach((t) => t.stop());
      this._localStream = null;
    }

    if (this._audioContext) {
      try {
        await this._audioContext.close();
      } catch { /* ignore */ }
      this._audioContext = null;
      this._analyser = null;
    }

    if (this._pc) {
      this._pc.close();
      this._pc = null;
    }

    this._remoteStream = null;
    this._callId = null;
    this._updateConnectionState("closed");
  }

  /* ── Statistics ────────────────────────────────────────────────────────── */

  /** Collect a one-shot RTCStatsReport snapshot. */
  async getStats(): Promise<WebRtcStats | null> {
    if (!this._pc) return null;

    try {
      const report = await this._pc.getStats();
      let bytesReceived = 0;
      let bytesSent = 0;
      let packetsReceived = 0;
      let packetsSent = 0;
      let packetsLost = 0;
      let jitter = 0;
      let roundTripTime = 0;
      let audioLevel = 0;

      report.forEach((stat) => {
        if (stat.type === "inbound-rtp" && stat.kind === "audio") {
          bytesReceived = stat.bytesReceived ?? 0;
          packetsReceived = stat.packetsReceived ?? 0;
          packetsLost = stat.packetsLost ?? 0;
          jitter = stat.jitter ?? 0;
          audioLevel = stat.audioLevel ?? 0;
        }
        if (stat.type === "outbound-rtp" && stat.kind === "audio") {
          bytesSent = stat.bytesSent ?? 0;
          packetsSent = stat.packetsSent ?? 0;
        }
        if (stat.type === "candidate-pair" && stat.state === "succeeded") {
          roundTripTime = stat.currentRoundTripTime ?? 0;
        }
      });

      return {
        bytesReceived,
        bytesSent,
        packetsReceived,
        packetsSent,
        packetsLost,
        jitter,
        roundTripTime,
        audioLevel,
        timestamp: Date.now(),
      };
    } catch {
      return null;
    }
  }

  /** Gather ICE candidates for diagnostics (without a real call). */
  async gatherIceCandidates(
    stunServers?: string[],
    turnServers?: TurnServerConfig[],
  ): Promise<RTCIceCandidate[]> {
    const iceServers = this._buildIceServers(
      stunServers ?? this._config.stunServers,
      turnServers ?? this._config.turnServers,
    );

    const pc = new RTCPeerConnection({ iceServers });
    const candidates: RTCIceCandidate[] = [];

    return new Promise((resolve) => {
      const timeout = setTimeout(() => {
        pc.close();
        resolve(candidates);
      }, 10000);

      pc.onicecandidate = (e) => {
        if (e.candidate) {
          candidates.push(e.candidate);
        } else {
          clearTimeout(timeout);
          pc.close();
          resolve(candidates);
        }
      };

      // Need a transceiver to trigger gathering
      pc.addTransceiver("audio", { direction: "sendrecv" });
      pc.createOffer().then((offer) => pc.setLocalDescription(offer));
    });
  }

  /* ── Internals ─────────────────────────────────────────────────────────── */

  private _buildIceServers(
    stunServers: string[],
    turnServers: TurnServerConfig[],
  ): RTCIceServer[] {
    const servers: RTCIceServer[] = [];

    if (stunServers.length > 0) {
      servers.push({ urls: stunServers });
    }

    for (const turn of turnServers) {
      servers.push({
        urls: turn.urls,
        username: turn.username,
        credential: turn.credential,
      });
    }

    return servers;
  }

  private async _createConnection(deviceId?: string): Promise<void> {
    // Acquire microphone
    this._localStream = await this.requestMicrophone(deviceId);

    // Build ICE server list
    const iceServers = this._buildIceServers(
      this._config.stunServers,
      this._config.turnServers,
    );

    // Create peer connection
    this._pc = new RTCPeerConnection({
      iceServers,
      iceCandidatePoolSize: 2,
    });

    // Add local tracks
    this._localStream.getTracks().forEach((track) => {
      this._pc!.addTrack(track, this._localStream!);
    });

    // Wire events
    this._pc.onicecandidate = (e) => this._handleIceCandidate(e);
    this._pc.ontrack = (e) => this._handleTrack(e);
    this._pc.onconnectionstatechange = () => this._handleConnectionStateChange();
    this._pc.oniceconnectionstatechange = () => this._handleIceConnectionStateChange();

    this._updateConnectionState("connecting");

    // Start audio level monitoring
    this._setupAudioLevelMonitor();

    // Start stats polling
    if (this._config.enableStats) {
      this._startStatsPolling();
    }
  }

  private _handleIceCandidate(event: RTCPeerConnectionIceEvent): void {
    if (!event.candidate || !this._callId) return;

    this._callbacks.onIceCandidate?.(event.candidate);

    // Relay through HA WebSocket
    this._sendWs({
      type: "voip/webrtc_candidate",
      call_id: this._callId,
      candidate: event.candidate.toJSON(),
    });
  }

  private _handleTrack(event: RTCTrackEvent): void {
    if (event.streams[0]) {
      this._remoteStream = event.streams[0];
      this._callbacks.onRemoteStream?.(this._remoteStream);
    }
  }

  private _handleConnectionStateChange(): void {
    if (!this._pc) return;

    const state = this._pc.connectionState as WebRtcConnectionState;
    this._updateConnectionState(state);

    if (state === "failed") {
      this._attemptReconnect();
    }
  }

  private _handleIceConnectionStateChange(): void {
    if (!this._pc) return;

    // Map ICE states to our connection states for extra reliability
    const iceState = this._pc.iceConnectionState;
    if (iceState === "connected" || iceState === "completed") {
      this._updateConnectionState("connected");
      this._reconnectAttempts = 0;
    } else if (iceState === "disconnected") {
      this._updateConnectionState("disconnected");
    } else if (iceState === "failed") {
      this._attemptReconnect();
    }
  }

  private _updateConnectionState(state: WebRtcConnectionState): void {
    if (state === this._connectionState) return;
    this._connectionState = state;
    this._callbacks.onConnectionStateChange?.(state);
  }

  /* ── Reconnection ──────────────────────────────────────────────────────── */

  private async _attemptReconnect(): Promise<void> {
    if (this._reconnectAttempts >= this._config.maxReconnectAttempts) {
      this._updateConnectionState("failed");
      return;
    }

    this._reconnectAttempts++;
    this._callbacks.onReconnecting?.();

    const delay = RECONNECT_DELAY_BASE * Math.pow(2, this._reconnectAttempts - 1);
    await new Promise((r) => setTimeout(r, delay));

    if (!this._pc || !this._callId) return;

    try {
      // ICE restart
      const offer = await this._pc.createOffer({ iceRestart: true });
      await this._pc.setLocalDescription(offer);

      await this._sendWs({
        type: "voip/webrtc_offer",
        call_id: this._callId,
        sdp: offer.sdp!,
      });

      this._callbacks.onReconnected?.();
    } catch (err) {
      console.error("[WebRTC] Reconnection failed:", err);
      this._attemptReconnect();
    }
  }

  /* ── Audio level monitoring ────────────────────────────────────────────── */

  private _setupAudioLevelMonitor(): void {
    if (!this._localStream) return;

    // Clean up existing
    if (this._audioLevelTimer) {
      clearInterval(this._audioLevelTimer);
      this._audioLevelTimer = null;
    }

    try {
      if (!this._audioContext) {
        this._audioContext = new AudioContext();
      }

      const source = this._audioContext.createMediaStreamSource(this._localStream);
      this._analyser = this._audioContext.createAnalyser();
      this._analyser.fftSize = 256;
      this._analyser.smoothingTimeConstant = 0.5;
      source.connect(this._analyser);

      const data = new Uint8Array(this._analyser.frequencyBinCount);

      this._audioLevelTimer = setInterval(() => {
        if (!this._analyser) return;
        this._analyser.getByteFrequencyData(data);

        // RMS of the frequency data, normalized 0-1
        let sum = 0;
        for (let i = 0; i < data.length; i++) {
          const val = data[i] / 255;
          sum += val * val;
        }
        const level = Math.sqrt(sum / data.length);
        this._callbacks.onAudioLevel?.(level);
      }, this._config.audioLevelIntervalMs);
    } catch (err) {
      console.warn("[WebRTC] Audio level monitoring unavailable:", err);
    }
  }

  /* ── Stats polling ─────────────────────────────────────────────────────── */

  private _startStatsPolling(): void {
    this._statsTimer = setInterval(async () => {
      const stats = await this.getStats();
      if (stats) {
        this._callbacks.onStats?.(stats);
      }
    }, this._config.statsIntervalMs);
  }

  private _stopTimers(): void {
    if (this._statsTimer) {
      clearInterval(this._statsTimer);
      this._statsTimer = null;
    }
    if (this._audioLevelTimer) {
      clearInterval(this._audioLevelTimer);
      this._audioLevelTimer = null;
    }
  }

  /* ── HA WebSocket helpers ──────────────────────────────────────────────── */

  private async _sendWs(msg: Record<string, unknown>): Promise<void> {
    if (!this._hass) {
      console.error("[WebRTC] No hass instance available");
      return;
    }
    try {
      await this._hass.callWS(msg);
    } catch (err) {
      console.error("[WebRTC] WS send error:", err);
      this._callbacks.onError?.(
        err instanceof Error ? err : new Error("WebSocket send failed"),
      );
    }
  }

  private async _subscribeSignalling(): Promise<void> {
    if (!this._hass || !this._callId) return;

    try {
      const unsub = await this._hass.connection.subscribeMessage<any>(
        (msg) => {
          if (msg.event === "webrtc_answer" && msg.call_id === this._callId) {
            this.handleRemoteAnswer(msg.sdp);
          } else if (
            msg.event === "webrtc_candidate" &&
            msg.call_id === this._callId
          ) {
            this.addIceCandidate(msg.candidate);
          }
        },
        {
          type: "voip/subscribe",
          call_id: this._callId,
        },
      );

      this._unsubscribe = unsub;
    } catch (err) {
      console.error("[WebRTC] Failed to subscribe to signalling:", err);
    }
  }
}
