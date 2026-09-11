/**
 * Client-side migration version contract for the generated JavaScript
 * client.
 *
 * The program normalizes stale data inside the transaction that touches it,
 * but a client reading an account over RPC bypasses the program entirely.
 * The generated decoder must therefore reject any migration version other
 * than its own before interpreting fields, mirroring the Dart and Rust
 * clients. Without this check a stale account with a compatible byte length
 * would silently decode through the wrong field layout.
 */

import { describe, expect, it } from "vitest";

import { getStateDecoder } from "../../../clients/js/migrations_program/src/generated/accounts/state";

const STATE_CURRENT_VERSION = 2;
// discriminator (1) + migration version (1) + authority (32) + value (8)
// + enabled (1) + revision (1).
const STATE_V2_SIZE = 44;

function stateBytes(version: number): Uint8Array {
	const bytes = new Uint8Array(STATE_V2_SIZE);
	bytes[0] = 1; // STATE_DISCRIMINATOR
	bytes[1] = version;
	// authority stays the zero address; value stays 0; enabled = 1.
	bytes[STATE_V2_SIZE - 2] = 1;
	return bytes;
}

describe("generated migration version decoder", () => {
	it("decodes data written at the client's current version", () => {
		const [state] = getStateDecoder().read(
			stateBytes(STATE_CURRENT_VERSION),
			0,
		);

		expect(state.migrationVersion).toBe(STATE_CURRENT_VERSION);
		expect(state.enabled).toBe(true);
	});

	it("rejects a stale version with an actionable error", () => {
		let rejection: unknown;
		try {
			getStateDecoder().read(stateBytes(STATE_CURRENT_VERSION - 1), 0);
		} catch (error) {
			rejection = error;
		}

		expect(rejection).toBeInstanceOf(RangeError);
		expect(String(rejection)).toContain("migration version mismatch");
		expect(String(rejection)).toContain("expected 2, received 1");
		expect(String(rejection)).toContain("migrate it by sending a transaction");
	});

	it("rejects a future version with an upgrade hint", () => {
		let rejection: unknown;
		try {
			getStateDecoder().read(stateBytes(STATE_CURRENT_VERSION + 1), 0);
		} catch (error) {
			rejection = error;
		}

		expect(rejection).toBeInstanceOf(RangeError);
		expect(String(rejection)).toContain("migration version mismatch");
		expect(String(rejection)).toContain("expected 2, received 3");
		expect(String(rejection)).toContain("upgrade this client");
	});
});
