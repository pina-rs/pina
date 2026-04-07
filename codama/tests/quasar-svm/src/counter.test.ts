import { QuasarSvm } from "@blueshift-gg/quasar-svm/kit";
import {
	type Address,
	generateKeyPairSigner,
	getAddressEncoder,
	getProgramDerivedAddress,
	getUtf8Encoder,
} from "@solana/kit";
import { describe, expect, test } from "vitest";
import { decodeCounterState } from "../../../clients/js/counter_program/src/generated/accounts";
import {
	getIncrementInstruction,
	getInitializeInstruction,
} from "../../../clients/js/counter_program/src/generated/instructions";
import { COUNTER_PROGRAM_PROGRAM_ADDRESS } from "../../../clients/js/counter_program/src/generated/programs";
import { createFundedSignerAccount, expectSome, loadProgram } from "./helpers";

const PROGRAM_NAME = "counter_program";

async function deriveCounterPda(authority: Address) {
	return await getProgramDerivedAddress({
		programAddress: COUNTER_PROGRAM_PROGRAM_ADDRESS,
		seeds: [
			getUtf8Encoder().encode("counter"),
			getAddressEncoder().encode(authority),
		],
	});
}

describe("counter_program quasar e2e", () => {
	test("initialize and increment a counter", async () => {
		using svm = new QuasarSvm();
		if (!loadProgram(svm, COUNTER_PROGRAM_PROGRAM_ADDRESS, PROGRAM_NAME)) {
			console.log(
				`[SKIP] ${PROGRAM_NAME}.so not found. Build SBF binaries first.`,
			);
			return;
		}

		const authority = await generateKeyPairSigner();
		const authorityAccount = createFundedSignerAccount(authority);
		const [counterPda, bump] = await deriveCounterPda(authority.address);

		const initializeResult = svm.processInstruction(
			getInitializeInstruction({ authority, counter: counterPda, bump }),
			[authorityAccount],
		);
		initializeResult.assertSuccess();

		const initializedCounter = decodeCounterState(
			expectSome(
				initializeResult.account(counterPda),
				"counter PDA should exist after initialize",
			),
		);
		expect(initializedCounter.data.bump).toBe(bump);
		expect(initializedCounter.data.count).toBe(0n);

		const incrementResult = svm.processInstruction(
			getIncrementInstruction({ authority, counter: counterPda }),
			[
				authorityAccount,
				expectSome(
					initializeResult.account(counterPda),
					"counter PDA should exist before increment",
				),
			],
		);
		incrementResult.assertSuccess();

		const incrementedCounter = decodeCounterState(
			expectSome(
				incrementResult.account(counterPda),
				"counter PDA should exist after increment",
			),
		);
		expect(incrementedCounter.data.count).toBe(1n);
	});
});
