/**
 * HA VoIP Card — Production Lovelace card for browser-based SIP calling.
 *
 * Uses JsSIP (loaded from CDN) for SIP-over-WebSocket signaling and
 * native WebRTC for audio.  Drops into any HA dashboard as:
 *
 *   type: custom:ha-voip-card
 *   extension: "100"
 *   password: "secret"
 *   name: "Alice"               # optional display name
 *   server: "192.168.1.100"     # optional — auto-detected from HA
 *   ws_port: 8088               # optional — default 8088
 *
 * @version 1.0.0
 */

/* ------------------------------------------------------------------ */
/*  Constants                                                          */
/* ------------------------------------------------------------------ */

const CARD_VERSION = '1.0.0';
const JSSIP_CDN = 'https://cdn.jsdelivr.net/npm/jssip@3.10.1/dist/jssip.min.js';
const DEFAULT_WS_PORT = 8088;
const RING_TIMEOUT = 30;

const S = Object.freeze({
  LOADING:      'loading',
  UNREGISTERED: 'unregistered',
  REGISTERED:   'registered',
  CALLING:      'calling',
  RINGING:      'ringing',
  IN_CALL:      'in_call',
  ERROR:        'error',
});

/* ------------------------------------------------------------------ */
/*  JsSIP dynamic loader                                               */
/* ------------------------------------------------------------------ */

let _jssipPromise = null;
function loadJsSIP() {
  if (window.JsSIP) return Promise.resolve(window.JsSIP);
  if (_jssipPromise) return _jssipPromise;
  _jssipPromise = new Promise((resolve, reject) => {
    const s = document.createElement('script');
    s.src = JSSIP_CDN;
    s.async = true;
    s.onload = () => (window.JsSIP ? resolve(window.JsSIP) : reject(new Error('JsSIP not found after load')));
    s.onerror = () => reject(new Error('Failed to load JsSIP from CDN'));
    document.head.appendChild(s);
  });
  return _jssipPromise;
}

/* ------------------------------------------------------------------ */
/*  Utility helpers                                                    */
/* ------------------------------------------------------------------ */

function fmtTime(sec) {
  const m = String(Math.floor(sec / 60)).padStart(2, '0');
  const s = String(sec % 60).padStart(2, '0');
  return `${m}:${s}`;
}

/* ------------------------------------------------------------------ */
/*  Styles                                                             */
/* ------------------------------------------------------------------ */

const CARD_CSS = `
:host { display: block; }

ha-card {
  padding: 16px;
  overflow: hidden;
  font-family: var(--paper-font-body1_-_font-family, Roboto, sans-serif);
}

/* ---- Status bar ---- */
.status-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
  font-size: 13px;
  color: var(--secondary-text-color);
}
.status-dot {
  width: 8px; height: 8px; border-radius: 50%;
  display: inline-block; margin-right: 6px;
}
.status-dot.online  { background: #4CAF50; }
.status-dot.offline { background: #9E9E9E; }
.status-dot.busy    { background: #FF9800; }
.status-name {
  font-weight: 500;
  color: var(--primary-text-color);
}

/* ---- Display field ---- */
.display {
  text-align: center;
  font-size: 28px;
  font-weight: 300;
  letter-spacing: 2px;
  min-height: 42px;
  line-height: 42px;
  margin-bottom: 8px;
  color: var(--primary-text-color);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.display.small { font-size: 16px; letter-spacing: 0; }

/* ---- Dialpad ---- */
.dialpad {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
  max-width: 280px;
  margin: 0 auto 12px;
}
.key {
  display: flex; flex-direction: column;
  align-items: center; justify-content: center;
  height: 56px;
  border-radius: 50%;
  background: var(--secondary-background-color, #f5f5f5);
  border: none;
  cursor: pointer;
  user-select: none;
  -webkit-tap-highlight-color: transparent;
  transition: background 0.15s, transform 0.1s;
  color: var(--primary-text-color);
}
.key:active {
  transform: scale(0.92);
  background: var(--divider-color, #e0e0e0);
}
.key .digit { font-size: 22px; font-weight: 400; line-height: 1; }
.key .sub   { font-size: 9px; color: var(--secondary-text-color); margin-top: 1px; letter-spacing: 1px; }

/* ---- Action buttons ---- */
.actions {
  display: flex;
  justify-content: center;
  gap: 16px;
  margin-top: 4px;
}
.btn {
  display: flex; align-items: center; justify-content: center;
  border: none; border-radius: 50%; cursor: pointer;
  width: 56px; height: 56px;
  transition: transform 0.1s, box-shadow 0.2s;
  color: #fff;
  font-size: 22px;
}
.btn:active { transform: scale(0.92); }
.btn-call   { background: #4CAF50; box-shadow: 0 2px 8px rgba(76,175,80,.35); }
.btn-hangup { background: #F44336; box-shadow: 0 2px 8px rgba(244,67,54,.35); }
.btn-answer { background: #4CAF50; box-shadow: 0 2px 8px rgba(76,175,80,.35); }
.btn-reject { background: #F44336; }
.btn-small  { width: 48px; height: 48px; font-size: 18px; border-radius: 14px; }

/* ---- In-call controls ---- */
.call-controls {
  display: flex;
  justify-content: center;
  gap: 12px;
  margin-bottom: 16px;
}
.ctrl {
  display: flex; flex-direction: column;
  align-items: center; justify-content: center;
  width: 64px; height: 64px;
  border-radius: 16px;
  background: var(--secondary-background-color, #f5f5f5);
  border: none; cursor: pointer;
  color: var(--primary-text-color);
  font-size: 11px; font-weight: 500;
  transition: background 0.15s;
}
.ctrl:active { background: var(--divider-color, #e0e0e0); }
.ctrl.active {
  background: var(--primary-color);
  color: var(--text-primary-color, #fff);
}
.ctrl-icon { font-size: 20px; margin-bottom: 2px; }

/* ---- Calling / ringing overlay ---- */
.call-info {
  text-align: center;
  padding: 24px 0 16px;
}
.call-info .label { font-size: 14px; color: var(--secondary-text-color); margin-bottom: 4px; }
.call-info .target { font-size: 28px; font-weight: 400; color: var(--primary-text-color); }
.call-info .timer  { font-size: 16px; color: var(--secondary-text-color); margin-top: 4px; font-variant-numeric: tabular-nums; }

/* ---- Pulse animation ---- */
@keyframes pulse { 0%,100% { opacity:1; } 50% { opacity:.5; } }
.pulse { animation: pulse 1.5s ease-in-out infinite; }

/* ---- Incoming ring ---- */
.incoming {
  text-align: center;
  padding: 24px 0;
}
.incoming .from { font-size: 22px; font-weight: 400; color: var(--primary-text-color); margin-bottom: 4px; }
.incoming .label { font-size: 14px; color: var(--secondary-text-color); margin-bottom: 20px; }

/* ---- Error banner ---- */
.error-banner {
  background: rgba(244,67,54,.1);
  color: #D32F2F;
  border-radius: 8px;
  padding: 10px 14px;
  font-size: 13px;
  text-align: center;
  margin-bottom: 12px;
}

/* ---- Backspace ---- */
.backspace {
  position: absolute;
  right: 8px; top: 50%; transform: translateY(-50%);
  background: none; border: none; cursor: pointer;
  color: var(--secondary-text-color);
  font-size: 18px; padding: 8px;
}
.display-wrap { position: relative; }

/* ---- Hidden audio ---- */
audio { display: none; }
`;

/* ------------------------------------------------------------------ */
/*  The Card                                                           */
/* ------------------------------------------------------------------ */

class HaVoipCard extends HTMLElement {

  constructor() {
    super();
    this.attachShadow({ mode: 'open' });
    this._config = {};
    this._hass = null;
    this._ua = null;
    this._session = null;
    this._state = S.LOADING;
    this._display = '';
    this._callTimer = 0;
    this._callInterval = null;
    this._muted = false;
    this._held = false;
    this._showDtmf = false;
    this._error = '';
    this._remoteStream = null;
    this._initialized = false;
  }

  /* ---- HA lifecycle ---- */

  setConfig(config) {
    if (!config.extension) throw new Error('Please define "extension" in card config');
    if (!config.password)  throw new Error('Please define "password" in card config');
    this._config = {
      extension: String(config.extension),
      password:  config.password,
      name:      config.name || config.extension,
      server:    config.server || '',
      ws_port:   config.ws_port || DEFAULT_WS_PORT,
    };
  }

  set hass(hass) {
    this._hass = hass;
    if (!this._config.server && hass) {
      // Auto-detect server from HA connection URL
      try {
        const url = new URL(hass.auth.data.hassUrl || window.location.href);
        this._config.server = url.hostname;
      } catch { /* keep empty, will fail later */ }
    }
    if (!this._initialized) {
      this._initialized = true;
      this._boot();
    }
  }

  static getConfigElement() { return undefined; }
  static getStubConfig()    { return { extension: '100', password: '', name: 'User' }; }
  getCardSize()             { return 6; }

  /* ---- Boot sequence ---- */

  async _boot() {
    this._render();
    try {
      await loadJsSIP();
      this._initSip();
    } catch (e) {
      this._state = S.ERROR;
      this._error = e.message;
      this._render();
    }
  }

  /* ---- SIP initialisation ---- */

  _initSip() {
    const cfg = this._config;
    if (!cfg.server) {
      this._state = S.ERROR;
      this._error = 'SIP server address not configured and could not be auto-detected.';
      this._render();
      return;
    }

    const wsProto = window.location.protocol === 'https:' ? 'wss' : 'ws';
    const wsUrl = `${wsProto}://${cfg.server}:${cfg.ws_port}`;

    const socket = new JsSIP.WebSocketInterface(wsUrl);
    this._ua = new JsSIP.UA({
      sockets:      [socket],
      uri:          `sip:${cfg.extension}@${cfg.server}`,
      password:     cfg.password,
      display_name: cfg.name,
      register:     true,
      session_timers: false,
      user_agent:   `HA-VoIP/${CARD_VERSION}`,
    });

    this._ua.on('connected',          ()  => { this._error = ''; });
    this._ua.on('disconnected',       ()  => this._onUnregistered());
    this._ua.on('registered',         ()  => this._onRegistered());
    this._ua.on('unregistered',       ()  => this._onUnregistered());
    this._ua.on('registrationFailed', (e) => this._onRegFailed(e));
    this._ua.on('newRTCSession',      (e) => this._onNewSession(e));

    this._ua.start();
    this._state = S.UNREGISTERED;
    this._render();
  }

  /* ---- SIP event handlers ---- */

  _onRegistered() {
    this._state = S.REGISTERED;
    this._error = '';
    this._render();
  }

  _onUnregistered() {
    // Don't overwrite active call state
    if (this._state === S.IN_CALL || this._state === S.CALLING || this._state === S.RINGING) return;
    this._state = S.UNREGISTERED;
    this._render();
  }

  _onRegFailed(e) {
    this._state = S.ERROR;
    this._error = `Registration failed: ${e.cause || 'unknown'}`;
    this._render();
  }

  _onNewSession(e) {
    const session = e.session;

    if (session.direction === 'incoming') {
      // Incoming call
      if (this._session) {
        session.terminate({ status_code: 486 }); // Busy Here
        return;
      }
      this._session = session;
      this._state = S.RINGING;
      this._peerNumber = session.remote_identity.uri.user || 'Unknown';
      this._peerName = session.remote_identity.display_name || this._peerNumber;
      this._bindSessionEvents(session);
      this._render();
    }
    // Outgoing sessions are bound in _makeCall
  }

  _bindSessionEvents(session) {
    session.on('accepted',  () => this._onCallAccepted(session));
    session.on('confirmed', () => {});
    session.on('ended',     () => this._onCallEnded());
    session.on('failed',    (e) => this._onCallFailed(e));

    session.on('peerconnection', (e) => {
      e.peerconnection.addEventListener('track', (ev) => {
        if (ev.streams && ev.streams[0]) {
          this._remoteStream = ev.streams[0];
          const audio = this.shadowRoot.querySelector('#remoteAudio');
          if (audio) {
            audio.srcObject = this._remoteStream;
            audio.play().catch(() => {});
          }
        }
      });
    });
  }

  _onCallAccepted() {
    this._state = S.IN_CALL;
    this._callTimer = 0;
    this._muted = false;
    this._held = false;
    this._showDtmf = false;
    this._callInterval = setInterval(() => {
      this._callTimer++;
      const el = this.shadowRoot.querySelector('.timer');
      if (el) el.textContent = fmtTime(this._callTimer);
    }, 1000);
    this._render();
  }

  _onCallEnded() {
    this._cleanup();
    this._state = this._ua && this._ua.isRegistered() ? S.REGISTERED : S.UNREGISTERED;
    this._render();
  }

  _onCallFailed(e) {
    const cause = e && e.cause ? e.cause : 'unknown';
    if (cause !== 'Canceled' && cause !== 'Terminated') {
      this._error = `Call failed: ${cause}`;
    }
    this._cleanup();
    this._state = this._ua && this._ua.isRegistered() ? S.REGISTERED : S.UNREGISTERED;
    this._render();
  }

  _cleanup() {
    if (this._callInterval) { clearInterval(this._callInterval); this._callInterval = null; }
    this._session = null;
    this._callTimer = 0;
    this._muted = false;
    this._held = false;
    this._showDtmf = false;
    this._remoteStream = null;
  }

  /* ---- Call actions ---- */

  _makeCall() {
    const target = this._display.trim();
    if (!target || !this._ua) return;

    const cfg = this._config;
    const session = this._ua.call(`sip:${target}@${cfg.server}`, {
      mediaConstraints: { audio: true, video: false },
      rtcOfferConstraints: { offerToReceiveAudio: true },
      sessionTimersExpires: 120,
    });

    this._session = session;
    this._peerNumber = target;
    this._peerName = target;
    this._state = S.CALLING;
    this._bindSessionEvents(session);
    this._render();
  }

  _answer() {
    if (!this._session) return;
    this._session.answer({
      mediaConstraints: { audio: true, video: false },
    });
  }

  _hangup() {
    if (this._session) {
      try { this._session.terminate(); } catch { /* already ended */ }
    }
    this._cleanup();
    this._state = this._ua && this._ua.isRegistered() ? S.REGISTERED : S.UNREGISTERED;
    this._render();
  }

  _toggleMute() {
    if (!this._session) return;
    this._muted ? this._session.unmute({ audio: true }) : this._session.mute({ audio: true });
    this._muted = !this._muted;
    this._render();
  }

  _toggleHold() {
    if (!this._session) return;
    this._held ? this._session.unhold() : this._session.hold();
    this._held = !this._held;
    this._render();
  }

  _sendDtmf(digit) {
    if (this._session) {
      this._session.sendDTMF(digit, { duration: 100, interToneGap: 70 });
    }
  }

  _pressKey(digit) {
    if (this._state === S.IN_CALL && this._showDtmf) {
      this._sendDtmf(digit);
    } else {
      this._display += digit;
      this._render();
    }
  }

  _backspace() {
    this._display = this._display.slice(0, -1);
    this._render();
  }

  /* ---- Teardown ---- */

  disconnectedCallback() {
    if (this._ua) {
      try { this._ua.stop(); } catch { /* ignore */ }
      this._ua = null;
    }
    this._cleanup();
  }

  /* ---- Rendering ---- */

  _render() {
    const root = this.shadowRoot;

    // Skeleton on first render
    if (!root.querySelector('ha-card')) {
      root.innerHTML = `
        <style>${CARD_CSS}</style>
        <ha-card><div id="content"></div></ha-card>
        <audio id="remoteAudio" autoplay playsinline></audio>
      `;
    }

    const c = root.querySelector('#content');

    // Re-attach remote stream on re-render
    if (this._remoteStream) {
      const audio = root.querySelector('#remoteAudio');
      if (audio && audio.srcObject !== this._remoteStream) {
        audio.srcObject = this._remoteStream;
        audio.play().catch(() => {});
      }
    }

    switch (this._state) {
      case S.LOADING:      c.innerHTML = this._htmlLoading();      break;
      case S.ERROR:        c.innerHTML = this._htmlError();        break;
      case S.UNREGISTERED: c.innerHTML = this._htmlIdle(false);    this._bindDialpad(); break;
      case S.REGISTERED:   c.innerHTML = this._htmlIdle(true);     this._bindDialpad(); break;
      case S.CALLING:      c.innerHTML = this._htmlCalling();      this._bindActions(); break;
      case S.RINGING:      c.innerHTML = this._htmlRinging();      this._bindActions(); break;
      case S.IN_CALL:      c.innerHTML = this._htmlInCall();       this._bindInCall();  break;
    }
  }

  /* ---- HTML fragments ---- */

  _htmlLoading() {
    return `<div style="text-align:center;padding:32px 0;color:var(--secondary-text-color);">Loading VoIP…</div>`;
  }

  _htmlError() {
    return `
      <div class="error-banner">${this._esc(this._error)}</div>
      ${this._htmlIdle(false)}
    `;
  }

  _htmlStatusBar(online) {
    const dot = online ? 'online' : 'offline';
    const label = online ? 'Registered' : 'Disconnected';
    return `
      <div class="status-bar">
        <span><span class="status-dot ${dot}"></span>${label}</span>
        <span class="status-name">${this._esc(this._config.name)} · ${this._config.extension}</span>
      </div>
    `;
  }

  _htmlDialpad() {
    const keys = [
      ['1',''],['2','ABC'],['3','DEF'],
      ['4','GHI'],['5','JKL'],['6','MNO'],
      ['7','PQRS'],['8','TUV'],['9','WXYZ'],
      ['*',''],['0','+'],['#',''],
    ];
    return `<div class="dialpad">${keys.map(([d,s]) =>
      `<button class="key" data-digit="${d}"><span class="digit">${d}</span><span class="sub">${s}</span></button>`
    ).join('')}</div>`;
  }

  _htmlIdle(registered) {
    const hasDigits = this._display.length > 0;
    return `
      ${this._htmlStatusBar(registered)}
      ${this._error ? `<div class="error-banner">${this._esc(this._error)}</div>` : ''}
      <div class="display-wrap">
        <div class="display">${this._esc(this._display) || '&nbsp;'}</div>
        ${hasDigits ? '<button class="backspace" data-action="backspace">&#9003;</button>' : ''}
      </div>
      ${this._htmlDialpad()}
      <div class="actions">
        <button class="btn btn-call" data-action="call" ${!registered || !hasDigits ? 'disabled style="opacity:.4;cursor:default"' : ''}>&#128222;</button>
      </div>
    `;
  }

  _htmlCalling() {
    return `
      ${this._htmlStatusBar(true)}
      <div class="call-info">
        <div class="label pulse">Calling…</div>
        <div class="target">${this._esc(this._peerName)}</div>
      </div>
      <div class="actions">
        <button class="btn btn-hangup" data-action="hangup">&#9746;</button>
      </div>
    `;
  }

  _htmlRinging() {
    return `
      ${this._htmlStatusBar(true)}
      <div class="incoming">
        <div class="label pulse">Incoming Call</div>
        <div class="from">${this._esc(this._peerName)}</div>
      </div>
      <div class="actions">
        <button class="btn btn-answer" data-action="answer">&#128222;</button>
        <button class="btn btn-reject" data-action="hangup">&#9746;</button>
      </div>
    `;
  }

  _htmlInCall() {
    return `
      ${this._htmlStatusBar(true)}
      <div class="call-info">
        <div class="label">${this._held ? 'On Hold' : 'In Call'}</div>
        <div class="target">${this._esc(this._peerName)}</div>
        <div class="timer">${fmtTime(this._callTimer)}</div>
      </div>
      <div class="call-controls">
        <button class="ctrl ${this._muted ? 'active' : ''}" data-action="mute">
          <span class="ctrl-icon">${this._muted ? '&#128263;' : '&#127908;'}</span>
          ${this._muted ? 'Unmute' : 'Mute'}
        </button>
        <button class="ctrl ${this._held ? 'active' : ''}" data-action="hold">
          <span class="ctrl-icon">${this._held ? '&#9654;' : '&#9208;'}</span>
          ${this._held ? 'Resume' : 'Hold'}
        </button>
        <button class="ctrl ${this._showDtmf ? 'active' : ''}" data-action="dtmf-toggle">
          <span class="ctrl-icon">&#9000;</span>
          Keypad
        </button>
      </div>
      ${this._showDtmf ? this._htmlDialpad() : ''}
      <div class="actions">
        <button class="btn btn-hangup" data-action="hangup">&#9746;</button>
      </div>
    `;
  }

  /* ---- Event binding ---- */

  _bindDialpad() {
    const root = this.shadowRoot;
    root.querySelectorAll('.key').forEach(btn => {
      btn.onclick = () => this._pressKey(btn.dataset.digit);
    });
    const callBtn = root.querySelector('[data-action="call"]');
    if (callBtn) callBtn.onclick = () => this._makeCall();
    const bs = root.querySelector('[data-action="backspace"]');
    if (bs) bs.onclick = () => this._backspace();
  }

  _bindActions() {
    const root = this.shadowRoot;
    const a = (name, fn) => { const el = root.querySelector(`[data-action="${name}"]`); if (el) el.onclick = fn; };
    a('hangup', () => this._hangup());
    a('answer', () => this._answer());
  }

  _bindInCall() {
    const root = this.shadowRoot;
    const a = (name, fn) => { const el = root.querySelector(`[data-action="${name}"]`); if (el) el.onclick = fn; };
    a('mute',        () => this._toggleMute());
    a('hold',        () => this._toggleHold());
    a('hangup',      () => this._hangup());
    a('dtmf-toggle', () => { this._showDtmf = !this._showDtmf; this._render(); });

    root.querySelectorAll('.key').forEach(btn => {
      btn.onclick = () => this._pressKey(btn.dataset.digit);
    });
  }

  /* ---- Helpers ---- */

  _esc(str) {
    const el = document.createElement('span');
    el.textContent = str || '';
    return el.innerHTML;
  }
}

/* ------------------------------------------------------------------ */
/*  Registration                                                       */
/* ------------------------------------------------------------------ */

customElements.define('ha-voip-card', HaVoipCard);

window.customCards = window.customCards || [];
window.customCards.push({
  type: 'ha-voip-card',
  name: 'HA VoIP Phone',
  description: 'Browser-based SIP phone card for HA VoIP integration',
  preview: true,
});

console.info(`%c HA-VOIP-CARD %c v${CARD_VERSION} `, 'background:#4CAF50;color:#fff;font-weight:700;', 'background:#333;color:#fff;');
