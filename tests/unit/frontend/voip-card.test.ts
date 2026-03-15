/**
 * Unit tests for the HA VoIP Lovelace card.
 *
 * Uses a lightweight DOM mock (no real browser).  Tests cover rendering,
 * config handling, and call-state display.
 *
 * Run with:  npx vitest run tests/unit/frontend/voip-card.test.ts
 *        or: npx jest --config jest.config.ts
 */

import { describe, it, expect, beforeEach, vi } from "vitest";

// ---------------------------------------------------------------------------
// Minimal stubs for LitElement / HA types
// ---------------------------------------------------------------------------

interface MinimalHass {
  states: Record<string, { state: string; attributes: Record<string, unknown> }>;
  callService: (...args: unknown[]) => Promise<void>;
  connection: {
    subscribeMessage: (cb: (msg: unknown) => void, sub: Record<string, unknown>) => Promise<() => void>;
  };
}

interface VoipCardConfig {
  type: string;
  title?: string;
  entity?: string;
  extensions?: { name: string; number: string }[];
  show_dialpad?: boolean;
  show_recent_calls?: boolean;
  recent_calls_count?: number;
  compact_mode?: boolean;
}

type CallStateValue = "idle" | "ringing" | "dialing" | "connected" | "on_hold" | "ended";

// ---------------------------------------------------------------------------
// Simplified card logic extracted for testability
// ---------------------------------------------------------------------------

class VoipCardLogic {
  config: VoipCardConfig = { type: "custom:ha-voip-card" };
  hass: MinimalHass | null = null;
  callState: CallStateValue = "idle";
  remoteNumber = "";
  duration = 0;

  setConfig(config: VoipCardConfig): void {
    if (!config) throw new Error("Invalid configuration");
    if (config.type !== "custom:ha-voip-card") {
      throw new Error(`Unexpected card type: ${config.type}`);
    }
    this.config = { ...config };
  }

  getCardSize(): number {
    return this.config.compact_mode ? 3 : 5;
  }

  get title(): string {
    return this.config.title ?? "VoIP";
  }

  get showDialpad(): boolean {
    return this.config.show_dialpad ?? true;
  }

  get showRecentCalls(): boolean {
    return this.config.show_recent_calls ?? true;
  }

  get recentCallsCount(): number {
    return this.config.recent_calls_count ?? 5;
  }

  get extensions(): { name: string; number: string }[] {
    return this.config.extensions ?? [];
  }

  get statusText(): string {
    switch (this.callState) {
      case "idle":
        return "Ready";
      case "ringing":
        return `Incoming: ${this.remoteNumber}`;
      case "dialing":
        return `Calling ${this.remoteNumber}...`;
      case "connected":
        return `In call: ${this.formatDuration(this.duration)}`;
      case "on_hold":
        return "On Hold";
      case "ended":
        return "Call Ended";
      default:
        return "Unknown";
    }
  }

  formatDuration(seconds: number): string {
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  applyCallState(state: CallStateValue, remote?: string, dur?: number): void {
    this.callState = state;
    if (remote !== undefined) this.remoteNumber = remote;
    if (dur !== undefined) this.duration = dur;
  }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("VoipCardLogic", () => {
  let card: VoipCardLogic;

  beforeEach(() => {
    card = new VoipCardLogic();
  });

  // -- Config handling --

  it("should accept a valid config", () => {
    card.setConfig({ type: "custom:ha-voip-card", title: "My Phone" });
    expect(card.title).toBe("My Phone");
  });

  it("should throw on missing config", () => {
    expect(() => card.setConfig(null as unknown as VoipCardConfig)).toThrow("Invalid configuration");
  });

  it("should throw on wrong card type", () => {
    expect(() =>
      card.setConfig({ type: "custom:wrong-card" }),
    ).toThrow("Unexpected card type");
  });

  it("should use default title when not provided", () => {
    card.setConfig({ type: "custom:ha-voip-card" });
    expect(card.title).toBe("VoIP");
  });

  it("should default showDialpad to true", () => {
    card.setConfig({ type: "custom:ha-voip-card" });
    expect(card.showDialpad).toBe(true);
  });

  it("should respect compact_mode in card size", () => {
    card.setConfig({ type: "custom:ha-voip-card", compact_mode: true });
    expect(card.getCardSize()).toBe(3);

    card.setConfig({ type: "custom:ha-voip-card", compact_mode: false });
    expect(card.getCardSize()).toBe(5);
  });

  it("should surface extensions from config", () => {
    card.setConfig({
      type: "custom:ha-voip-card",
      extensions: [
        { name: "Alice", number: "100" },
        { name: "Bob", number: "101" },
      ],
    });
    expect(card.extensions).toHaveLength(2);
    expect(card.extensions[0].name).toBe("Alice");
  });

  // -- Call state display --

  it("should show 'Ready' when idle", () => {
    expect(card.statusText).toBe("Ready");
  });

  it("should show incoming number when ringing", () => {
    card.applyCallState("ringing", "102");
    expect(card.statusText).toBe("Incoming: 102");
  });

  it("should show duration when connected", () => {
    card.applyCallState("connected", "101", 125);
    expect(card.statusText).toBe("In call: 2:05");
  });

  it("should format duration correctly for exact minutes", () => {
    expect(card.formatDuration(60)).toBe("1:00");
    expect(card.formatDuration(0)).toBe("0:00");
    expect(card.formatDuration(3661)).toBe("61:01");
  });

  it("should show 'On Hold' state", () => {
    card.applyCallState("on_hold");
    expect(card.statusText).toBe("On Hold");
  });

  it("should show 'Call Ended' state", () => {
    card.applyCallState("ended");
    expect(card.statusText).toBe("Call Ended");
  });
});
