/* ──────────────────────────────────────────────────────────────────────────────
 * styles.ts — Shared CSS styles for HA VoIP components
 * Uses HA CSS custom properties for full theme integration.
 * ────────────────────────────────────────────────────────────────────────── */
import { css } from "lit";

/* ── Reset & common card styling ─────────────────────────────────────────── */
export const cardStyles = css`
  :host {
    --voip-primary: var(--primary-color, #03a9f4);
    --voip-primary-text: var(--primary-text-color, #212121);
    --voip-secondary-text: var(--secondary-text-color, #727272);
    --voip-disabled: var(--disabled-text-color, #bdbdbd);
    --voip-divider: var(--divider-color, rgba(0, 0, 0, 0.12));
    --voip-card-bg: var(--card-background-color, #fff);
    --voip-surface: var(--ha-card-background, var(--voip-card-bg));
    --voip-error: var(--error-color, #db4437);
    --voip-success: var(--success-color, #43a047);
    --voip-warning: var(--warning-color, #ffa726);
    --voip-info: var(--info-color, #039be5);
    --voip-radius: var(--ha-card-border-radius, 12px);
    --voip-shadow: var(
      --ha-card-box-shadow,
      0 2px 2px 0 rgba(0, 0, 0, 0.14),
      0 1px 5px 0 rgba(0, 0, 0, 0.12),
      0 3px 1px -2px rgba(0, 0, 0, 0.2)
    );
    --voip-btn-size: 56px;
    --voip-btn-size-sm: 44px;

    display: block;
    font-family: var(--paper-font-body1_-_font-family, "Roboto", sans-serif);
    color: var(--voip-primary-text);
  }

  *,
  *::before,
  *::after {
    box-sizing: border-box;
  }

  ha-card {
    overflow: hidden;
    border-radius: var(--voip-radius);
  }
`;

/* ── Buttons ─────────────────────────────────────────────────────────────── */
export const buttonStyles = css`
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: 50%;
    cursor: pointer;
    transition: background-color 0.2s ease, transform 0.1s ease,
      box-shadow 0.2s ease;
    user-select: none;
    -webkit-tap-highlight-color: transparent;
    outline: none;
    font-size: 0;
    padding: 0;
  }

  .btn:focus-visible {
    box-shadow: 0 0 0 3px var(--voip-primary);
  }

  .btn:active {
    transform: scale(0.93);
  }

  .btn--lg {
    width: var(--voip-btn-size);
    height: var(--voip-btn-size);
  }

  .btn--md {
    width: var(--voip-btn-size-sm);
    height: var(--voip-btn-size-sm);
  }

  .btn--sm {
    width: 36px;
    height: 36px;
  }

  .btn--call {
    background-color: var(--voip-success);
    color: #fff;
  }

  .btn--call:hover {
    background-color: #388e3c;
  }

  .btn--hangup {
    background-color: var(--voip-error);
    color: #fff;
  }

  .btn--hangup:hover {
    background-color: #c62828;
  }

  .btn--action {
    background-color: var(--voip-surface);
    color: var(--voip-primary-text);
    border: 1px solid var(--voip-divider);
  }

  .btn--action:hover {
    background-color: var(--voip-divider);
  }

  .btn--action.active {
    background-color: var(--voip-primary);
    color: #fff;
    border-color: var(--voip-primary);
  }

  .btn--icon {
    background: none;
    color: var(--voip-secondary-text);
    border: none;
  }

  .btn--icon:hover {
    color: var(--voip-primary-text);
    background-color: rgba(0, 0, 0, 0.06);
  }
`;

/* ── Dialpad grid ────────────────────────────────────────────────────────── */
export const dialpadStyles = css`
  .dialpad-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
    padding: 16px;
    max-width: 280px;
    margin: 0 auto;
  }

  .dialpad-key {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    width: 64px;
    height: 64px;
    margin: 0 auto;
    border: none;
    border-radius: 50%;
    background-color: var(--voip-surface);
    border: 1px solid var(--voip-divider);
    cursor: pointer;
    transition: background-color 0.15s ease, transform 0.1s ease;
    user-select: none;
    -webkit-tap-highlight-color: transparent;
    font-family: inherit;
    outline: none;
  }

  .dialpad-key:focus-visible {
    box-shadow: 0 0 0 3px var(--voip-primary);
  }

  .dialpad-key:hover {
    background-color: var(--voip-divider);
  }

  .dialpad-key:active {
    transform: scale(0.92);
    background-color: var(--voip-primary);
    color: #fff;
  }

  .dialpad-key__digit {
    font-size: 24px;
    font-weight: 500;
    line-height: 1;
    color: var(--voip-primary-text);
  }

  .dialpad-key__letters {
    font-size: 9px;
    letter-spacing: 2px;
    text-transform: uppercase;
    color: var(--voip-secondary-text);
    margin-top: 2px;
  }

  .dialpad-key:active .dialpad-key__digit,
  .dialpad-key:active .dialpad-key__letters {
    color: #fff;
  }
`;

/* ── Call controls bar ───────────────────────────────────────────────────── */
export const callControlStyles = css`
  .call-controls {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 16px;
    padding: 16px;
  }

  .call-controls__label {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    color: var(--voip-secondary-text);
  }

  .call-controls__label .btn--action.active + span {
    color: var(--voip-primary);
  }
`;

/* ── Status indicators ───────────────────────────────────────────────────── */
export const statusStyles = css`
  .status-dot {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    margin-right: 6px;
    flex-shrink: 0;
  }

  .status-dot--available {
    background-color: var(--voip-success);
  }

  .status-dot--busy,
  .status-dot--ringing {
    background-color: var(--voip-warning);
  }

  .status-dot--offline {
    background-color: var(--voip-disabled);
  }

  .status-dot--dnd {
    background-color: var(--voip-error);
  }

  .badge {
    display: inline-flex;
    align-items: center;
    padding: 2px 8px;
    border-radius: 12px;
    font-size: 12px;
    font-weight: 500;
    line-height: 1.5;
  }

  .badge--idle {
    background-color: rgba(0, 0, 0, 0.06);
    color: var(--voip-secondary-text);
  }

  .badge--ringing {
    background-color: rgba(255, 167, 38, 0.15);
    color: #e65100;
    animation: pulse 1.5s infinite;
  }

  .badge--connected {
    background-color: rgba(67, 160, 71, 0.15);
    color: #2e7d32;
  }

  .badge--on_hold {
    background-color: rgba(3, 155, 229, 0.15);
    color: #01579b;
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.5;
    }
  }
`;

/* ── Call history list ───────────────────────────────────────────────────── */
export const historyStyles = css`
  .history-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .history-item {
    display: flex;
    align-items: center;
    padding: 10px 16px;
    border-bottom: 1px solid var(--voip-divider);
    gap: 12px;
    cursor: pointer;
    transition: background-color 0.15s;
  }

  .history-item:last-child {
    border-bottom: none;
  }

  .history-item:hover {
    background-color: rgba(0, 0, 0, 0.04);
  }

  .history-item__icon {
    flex-shrink: 0;
    width: 20px;
    text-align: center;
  }

  .history-item__icon--inbound {
    color: var(--voip-success);
  }

  .history-item__icon--outbound {
    color: var(--voip-primary);
  }

  .history-item__icon--missed {
    color: var(--voip-error);
  }

  .history-item__info {
    flex: 1;
    min-width: 0;
  }

  .history-item__name {
    font-size: 14px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .history-item__number {
    font-size: 12px;
    color: var(--voip-secondary-text);
  }

  .history-item__meta {
    text-align: right;
    flex-shrink: 0;
  }

  .history-item__time {
    font-size: 12px;
    color: var(--voip-secondary-text);
  }

  .history-item__duration {
    font-size: 11px;
    color: var(--voip-disabled);
  }
`;

/* ── Popup / dialog overlay ──────────────────────────────────────────────── */
export const popupStyles = css`
  .popup-overlay {
    position: fixed;
    inset: 0;
    z-index: 1000;
    display: flex;
    align-items: center;
    justify-content: center;
    background-color: rgba(0, 0, 0, 0.6);
    animation: fadeIn 0.2s ease;
  }

  .popup-card {
    background-color: var(--voip-surface);
    border-radius: var(--voip-radius);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
    max-width: 400px;
    width: 90vw;
    max-height: 90vh;
    overflow-y: auto;
    animation: slideUp 0.25s ease;
  }

  .popup-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid var(--voip-divider);
  }

  .popup-body {
    padding: 20px;
  }

  .popup-footer {
    display: flex;
    justify-content: center;
    gap: 16px;
    padding: 16px 20px;
    border-top: 1px solid var(--voip-divider);
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  @keyframes slideUp {
    from {
      opacity: 0;
      transform: translateY(30px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
`;

/* ── Wizard / onboarding steps ───────────────────────────────────────────── */
export const wizardStyles = css`
  .wizard {
    padding: 20px;
  }

  .wizard-progress {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    margin-bottom: 24px;
  }

  .wizard-progress__step {
    width: 32px;
    height: 4px;
    border-radius: 2px;
    background-color: var(--voip-divider);
    transition: background-color 0.3s ease;
  }

  .wizard-progress__step--active {
    background-color: var(--voip-primary);
  }

  .wizard-progress__step--completed {
    background-color: var(--voip-success);
  }

  .wizard-title {
    font-size: 20px;
    font-weight: 500;
    margin: 0 0 8px;
    color: var(--voip-primary-text);
  }

  .wizard-subtitle {
    font-size: 14px;
    color: var(--voip-secondary-text);
    margin: 0 0 20px;
    line-height: 1.5;
  }

  .wizard-actions {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: 24px;
    padding-top: 16px;
    border-top: 1px solid var(--voip-divider);
  }

  .wizard-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 20px;
    border: none;
    border-radius: 8px;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    transition: background-color 0.2s, transform 0.1s;
    font-family: inherit;
    outline: none;
  }

  .wizard-btn:focus-visible {
    box-shadow: 0 0 0 3px var(--voip-primary);
  }

  .wizard-btn:active {
    transform: scale(0.97);
  }

  .wizard-btn--primary {
    background-color: var(--voip-primary);
    color: #fff;
  }

  .wizard-btn--primary:hover {
    filter: brightness(1.1);
  }

  .wizard-btn--secondary {
    background: none;
    color: var(--voip-secondary-text);
  }

  .wizard-btn--secondary:hover {
    background-color: rgba(0, 0, 0, 0.06);
  }

  .wizard-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
`;

/* ── Diagnostics table ───────────────────────────────────────────────────── */
export const diagnosticsStyles = css`
  .diag-table {
    width: 100%;
    border-collapse: collapse;
  }

  .diag-row {
    display: flex;
    align-items: center;
    padding: 12px 16px;
    border-bottom: 1px solid var(--voip-divider);
    gap: 12px;
  }

  .diag-row:last-child {
    border-bottom: none;
  }

  .diag-icon {
    flex-shrink: 0;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
  }

  .diag-icon--pass {
    background-color: rgba(67, 160, 71, 0.15);
    color: var(--voip-success);
  }

  .diag-icon--fail {
    background-color: rgba(219, 68, 55, 0.15);
    color: var(--voip-error);
  }

  .diag-icon--warning {
    background-color: rgba(255, 167, 38, 0.15);
    color: var(--voip-warning);
  }

  .diag-icon--running {
    background-color: rgba(3, 155, 229, 0.15);
    color: var(--voip-info);
    animation: spin 1s linear infinite;
  }

  .diag-icon--pending {
    background-color: rgba(0, 0, 0, 0.06);
    color: var(--voip-disabled);
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .diag-info {
    flex: 1;
    min-width: 0;
  }

  .diag-name {
    font-size: 14px;
    font-weight: 500;
  }

  .diag-message {
    font-size: 12px;
    color: var(--voip-secondary-text);
    margin-top: 2px;
  }

  .diag-time {
    font-size: 12px;
    color: var(--voip-disabled);
    flex-shrink: 0;
  }
`;

/* ── Responsive helpers ──────────────────────────────────────────────────── */
export const responsiveStyles = css`
  @media (max-width: 600px) {
    :host {
      --voip-btn-size: 48px;
      --voip-btn-size-sm: 40px;
    }

    .dialpad-key {
      width: 56px;
      height: 56px;
    }

    .dialpad-key__digit {
      font-size: 20px;
    }

    .dialpad-grid {
      gap: 8px;
      padding: 12px;
    }

    .call-controls {
      gap: 10px;
      padding: 12px;
    }

    .popup-card {
      max-width: 100%;
      width: 100vw;
      max-height: 100vh;
      border-radius: 0;
    }
  }

  @media (max-width: 380px) {
    .dialpad-key {
      width: 48px;
      height: 48px;
    }

    .dialpad-key__digit {
      font-size: 18px;
    }

    .dialpad-key__letters {
      display: none;
    }
  }
`;

/* ── Form elements (for onboarding / config) ─────────────────────────────── */
export const formStyles = css`
  .form-group {
    margin-bottom: 16px;
  }

  .form-label {
    display: block;
    font-size: 13px;
    font-weight: 500;
    color: var(--voip-secondary-text);
    margin-bottom: 6px;
  }

  .form-input {
    width: 100%;
    padding: 10px 12px;
    border: 1px solid var(--voip-divider);
    border-radius: 8px;
    font-size: 14px;
    font-family: inherit;
    color: var(--voip-primary-text);
    background-color: var(--voip-surface);
    outline: none;
    transition: border-color 0.2s;
  }

  .form-input:focus {
    border-color: var(--voip-primary);
    box-shadow: 0 0 0 2px rgba(3, 169, 244, 0.2);
  }

  .form-input::placeholder {
    color: var(--voip-disabled);
  }

  .form-select {
    width: 100%;
    padding: 10px 12px;
    border: 1px solid var(--voip-divider);
    border-radius: 8px;
    font-size: 14px;
    font-family: inherit;
    color: var(--voip-primary-text);
    background-color: var(--voip-surface);
    outline: none;
    cursor: pointer;
    appearance: none;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 12 12'%3E%3Cpath fill='%23727272' d='M6 8L1 3h10z'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 12px center;
    padding-right: 32px;
  }

  .form-select:focus {
    border-color: var(--voip-primary);
    box-shadow: 0 0 0 2px rgba(3, 169, 244, 0.2);
  }

  .form-radio-group {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .form-radio {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 12px;
    border: 1px solid var(--voip-divider);
    border-radius: 8px;
    cursor: pointer;
    transition: border-color 0.2s, background-color 0.2s;
  }

  .form-radio:hover {
    background-color: rgba(0, 0, 0, 0.02);
  }

  .form-radio--selected {
    border-color: var(--voip-primary);
    background-color: rgba(3, 169, 244, 0.06);
  }

  .form-radio input[type="radio"] {
    margin-top: 2px;
    accent-color: var(--voip-primary);
  }

  .form-radio__label {
    font-size: 14px;
    font-weight: 500;
  }

  .form-radio__description {
    font-size: 12px;
    color: var(--voip-secondary-text);
    margin-top: 2px;
    line-height: 1.4;
  }
`;
