import assert from "node:assert/strict";
import { test } from "node:test";

import { address, blockhash, generateKeyPairSigner } from "@solana/kit";
import {
	buildTransactionMessage,
	CliError,
	consumptionSummary,
	parseComputeUnitLimit,
} from "../../clients/cli/ts/counter_program/src/context.js";
import {
	getCounterProgramComputeUnitLimit,
	INCREMENT_COMPUTE_UNIT_LIMIT,
	INCREMENT_MEASURED_COMPUTE_UNITS,
	INITIALIZE_COMPUTE_UNIT_LIMIT,
} from "../../clients/js/counter_program/src/generated/computeUnits.js";
import { COUNTER_PROGRAM_PROGRAM_ADDRESS } from "../../clients/js/counter_program/src/generated/programs/counterProgram.js";

const COMPUTE_BUDGET = address("ComputeBudget111111111111111111111111111111");
const SYSTEM = address("11111111111111111111111111111111");
const FORK = address("9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin");

const initialize = {
	programAddress: COUNTER_PROGRAM_PROGRAM_ADDRESS,
	data: new Uint8Array([0, 254]),
};
const increment = {
	programAddress: COUNTER_PROGRAM_PROGRAM_ADDRESS,
	data: new Uint8Array([1]),
};

test("each recorded limit covers its measurement", () => {
	assert.ok(INCREMENT_COMPUTE_UNIT_LIMIT > INCREMENT_MEASURED_COMPUTE_UNITS);
});

test("sums the recorded limits of this program's instructions", () => {
	assert.equal(
		getCounterProgramComputeUnitLimit([increment]),
		INCREMENT_COMPUTE_UNIT_LIMIT,
	);
	assert.equal(
		getCounterProgramComputeUnitLimit([
			initialize,
			{ programAddress: SYSTEM, data: new Uint8Array([2, 0, 0, 0]) },
			increment,
		]),
		INITIALIZE_COMPUTE_UNIT_LIMIT + INCREMENT_COMPUTE_UNIT_LIMIT,
	);
});

test("returns undefined without a measured instruction for this program", () => {
	assert.equal(getCounterProgramComputeUnitLimit([]), undefined);
	assert.equal(
		getCounterProgramComputeUnitLimit([{ programAddress: SYSTEM }]),
		undefined,
	);
	// One unmeasured instruction makes the whole sum unknown.
	assert.equal(
		getCounterProgramComputeUnitLimit([
			increment,
			{
				programAddress: COUNTER_PROGRAM_PROGRAM_ADDRESS,
				data: new Uint8Array([9]),
			},
		]),
		undefined,
	);
	assert.equal(
		getCounterProgramComputeUnitLimit([
			{ programAddress: COUNTER_PROGRAM_PROGRAM_ADDRESS },
		]),
		undefined,
	);
});

test("counts a program deployed at another address when told to", () => {
	const forked = { ...increment, programAddress: FORK };

	assert.equal(getCounterProgramComputeUnitLimit([forked]), undefined);
	assert.equal(
		getCounterProgramComputeUnitLimit([forked], { programAddress: FORK }),
		INCREMENT_COMPUTE_UNIT_LIMIT,
	);
});

test("caps the sum at the transaction maximum", () => {
	assert.equal(
		getCounterProgramComputeUnitLimit(new Array(10_000).fill(increment)),
		1_400_000,
	);
});

test("the CLI prepends exactly one limit instruction when a limit is requested", async () => {
	const payer = await generateKeyPairSigner();
	const lifetime = {
		blockhash: blockhash("11111111111111111111111111111111"),
		lastValidBlockHeight: 0n,
	};

	const limited = buildTransactionMessage(payer, lifetime, increment, 800);
	assert.equal(limited.instructions.length, 2);
	assert.equal(limited.instructions[0]?.programAddress, COMPUTE_BUDGET);
	assert.deepEqual(
		[...(limited.instructions[0]?.data ?? [])],
		[2, 0x20, 0x03, 0, 0],
	);
	assert.equal(limited.instructions[1], increment);

	const unlimited = buildTransactionMessage(
		payer,
		lifetime,
		increment,
		undefined,
	);
	assert.deepEqual(unlimited.instructions, [increment]);
});

test("the CLI accepts only limits a transaction can request", () => {
	assert.equal(parseComputeUnitLimit(undefined), undefined);
	assert.equal(parseComputeUnitLimit("800"), 800);
	assert.equal(parseComputeUnitLimit("1400000"), 1_400_000);

	for (const value of ["0", "1400001", "1.5", "-1", "1e3", "many", ""]) {
		assert.throws(() => parseComputeUnitLimit(value), CliError, value);
	}
});

test("the CLI reports consumption against the requested limit", () => {
	assert.equal(
		consumptionSummary(529, 800),
		"Consumed 529 of 800 requested compute units",
	);
	assert.match(consumptionSummary(379, undefined), /runtime default/u);
});
