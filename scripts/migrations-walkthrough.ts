#!/usr/bin/env node
// End-to-end migration walkthrough: one program, ten deployments.
//
// This is the executable narrative of a project that fully embraces Pina
// migrations. Each step modifies the program source, runs the real Pina CLI
// (`migrations make`, `check`, `deploy`), rebuilds the SBF artifact, loads it
// onto an isolated Surfpool network, regenerates every client (TypeScript,
// Rust, Dart), and proves that clients — including clients generated at
// earlier steps — keep working against the newest deployment.
//
// Run it from the repository root inside `devenv shell`:
//
//   pnpm exec tsx scripts/migrations-walkthrough.ts
//   pnpm exec tsx scripts/migrations-walkthrough.ts --from-step 6 --to-step 8
//   pnpm exec tsx scripts/migrations-walkthrough.ts --keep
//
// The walkthrough is intentionally not part of `cargo test`: a full run
// builds ten SBF artifacts and regenerates clients ten times.

import { spawnSync } from "node:child_process";
import {
	cpSync,
	existsSync,
	mkdirSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from "node:fs";
import { basename, resolve } from "node:path";
import process from "node:process";

import {
	AccountRole,
	address,
	appendTransactionMessageInstruction,
	createKeyPairSignerFromBytes,
	createSolanaRpc,
	createTransactionMessage,
	getAddressEncoder,
	getBase64EncodedWireTransaction,
	getSignatureFromTransaction,
	type Instruction,
	type ReadonlyUint8Array,
	sendTransactionWithoutConfirmingFactory,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
	signTransactionMessageWithSigners,
} from "@solana/kit";

import { Surfnet } from "@solana/surfpool";

const REPO = resolve(import.meta.dirname, "..");
const WORK = resolve(REPO, "target/migrations-walkthrough");
const PROGRAM_NAME = "walkthrough_program";
const PROGRAM_DIR = resolve(WORK, "examples", PROGRAM_NAME);
const IDLS_DIR = resolve(WORK, "idls");
const CLIENTS = resolve(WORK, "clients");
const CLIENT_HISTORY = resolve(WORK, "client-history");
const DEPLOY_DIR = resolve(WORK, "deploy");
const argv = process.argv.slice(2);
const flag = (name: string) => argv.includes(`--${name}`);
const KEEP = flag("keep");

let failures = 0;

function say(message: string): void {
	console.log(`\n\u001B[36m${message}\u001B[0m`);
}

function fail(message: string): void {
	failures += 1;
	console.error(`\u001B[31mFAIL\u001B[0m ${message}`);
	if (!KEEP) {
		throw new Error(message);
	}
}

const value = (name: string) => {
	const index = argv.indexOf(`--${name}`);
	if (index === -1) {
		return undefined;
	}
	const operand = argv[index + 1];
	const parsed = Number(operand);
	if (operand === undefined || !Number.isFinite(parsed)) {
		fail(`--${name} requires a finite numeric operand, got ${String(operand)}`);
		return undefined;
	}
	return parsed;
};
const FROM_STEP = value("from-step") ?? 1;
const TO_STEP = value("to-step") ?? 10;

function expect(condition: boolean, message: string): void {
	if (condition) {
		console.log(`\u001B[32mPASS\u001B[0m ${message}`);
	} else {
		fail(message);
	}
}

interface RunResult {
	status: number;
	stdout: string;
	stderr: string;
}

function run(
	program: string,
	args: string[],
	options: { cwd?: string; env?: Record<string, string> } = {},
): RunResult {
	const result = spawnSync(program, args, {
		cwd: options.cwd ?? REPO,
		encoding: "utf8",
		env: { ...process.env, ...options.env },
		maxBuffer: 64 * 1024 * 1024,
	});
	return {
		status: result.status ?? -1,
		stdout: result.stdout ?? "",
		stderr: result.stderr ?? "",
	};
}

function mustRun(
	program: string,
	args: string[],
	options: { cwd?: string; env?: Record<string, string> } = {},
	label: string,
): RunResult {
	const result = run(program, args, options);
	if (result.status !== 0) {
		console.error(result.stdout);
		console.error(result.stderr);
		throw new Error(`${label} failed with status ${result.status}`);
	}
	return result;
}

function pina(args: string[], cwd?: string): RunResult {
	return run(
		"cargo",
		[
			"run",
			"-q",
			"--manifest-path",
			`${REPO}/Cargo.toml`,
			"-p",
			"pina_cli",
			"--locked",
			"--",
			...args,
		],
		{ cwd },
	);
}

function mustPina(args: string[], cwd: string, label: string): string {
	const result = pina(args, cwd);
	if (result.status !== 0) {
		console.error(result.stdout);
		console.error(result.stderr);
		throw new Error(`${label} failed`);
	}
	if (process.env.WALKTHROUGH_VERBOSE) {
		console.log(result.stdout);
		console.error(result.stderr);
	}
	return result.stdout + result.stderr;
}

function writeProgramSource(source: string): void {
	writeFileSync(resolve(PROGRAM_DIR, "src/lib.rs"), source);
}

function scaffoldProgram(programId: string): void {
	mkdirSync(resolve(PROGRAM_DIR, "src"), { recursive: true });
	mkdirSync(resolve(PROGRAM_DIR, "migrations"), { recursive: true });
	writeFileSync(
		resolve(PROGRAM_DIR, "Cargo.toml"),
		[
			"[package]",
			`name = "${PROGRAM_NAME}"`,
			'version = "0.0.0"',
			'edition = "2024"',
			"publish = false",
			"",
			"[lib]",
			'crate-type = ["cdylib", "lib"]',
			"",
			"[workspace]",

			"[features]",
			'default = ["bpf-entrypoint"]',
			"bpf-entrypoint = []",
			"",
			"[dependencies]",
			`pina = { path = ${JSON.stringify(`${REPO}/crates/pina`)}, features = [` +
			'"account-resize", "compact", "derive", "validation"] }',
			"",
		].join("\n"),
	);
	writeFileSync(
		resolve(PROGRAM_DIR, "pina.toml"),
		[
			"[project]",
			'program = "."',
			"",
			"[migrations]",
			'version-type = "u8"',
			"",
		].join("\n"),
	);
	writeProgramSource(source_v0(programId));
}

function source_v0(programId: string): string {
	return `#![cfg_attr(not(feature = "fuzzing"), no_std)]

use pina::*;

declare_id!("${programId}");

const MAX_INLINE_MIGRATION_LAMPORTS: u64 = 20_000;

#[discriminator]
pub enum WalkInstruction {
	Update = 0,
}

#[discriminator]
pub enum WalkAccount {
	Profile = 1,
	Journal = 2,
}

#[discriminator]
pub enum WalkEvent {
	ProfileChanged = 3,
}

#[account(discriminator = WalkAccount::Profile, migrations)]
pub struct Profile {
	pub authority: Address,
	pub score: u64,
}

#[account(discriminator = WalkAccount::Journal, compact, migrations)]
pub struct Journal {
	pub title: String<12>,
}

#[instruction(discriminator = WalkInstruction::Update, migrations)]
pub struct UpdateInstruction {
	pub score: u64,
}

#[event(discriminator = WalkEvent::ProfileChanged, migrations)]
pub struct ProfileChanged {
	pub score: u64,
}

#[derive(Accounts)]
pub struct UpdateAccounts<'a> {
	#[pina(validate(signer))]
	pub authority: &'a AccountView,
	pub profile: Option<&'a mut AccountView>,
	pub journal: Option<&'a mut AccountView>,
	#[pina(validate(signer))]
	pub migration_payer: Option<&'a mut AccountView>,
	pub system_program: Option<&'a AccountView>,
}

impl<'a> ProcessAccountInfos<'a> for UpdateAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		UpdateInstruction::with_current_instruction_data(data, |current| {
			let instruction = UpdateInstruction::try_from_bytes(current)?;
			let _ = instruction;
			if self.profile.is_none()
				&& self.journal.is_none()
				&& self.migration_payer.is_none()
				&& self.system_program.is_none()
			{
				return Ok(());
			}
			let (Some(profile), payer, Some(system_program)) =
				(self.profile, self.migration_payer, self.system_program)
			else {
				return Err(ProgramError::NotEnoughAccountKeys);
			};
			system_program.assert_address(&system::ID)?;
			let payer = payer.map(|account| &*account);
			MigrateAccount {
				account: profile,
				payer,
				program_id: &ID,
				max_lamports: MAX_INLINE_MIGRATION_LAMPORTS,
			}
			.invoke::<Profile>()?;
			let mut profile = profile.as_account_mut::<Profile>(&ID)?;
			profile.score.set(instruction.score.get());
			Ok(())
		})
	}
}

pub struct WalkProgram;

impl CpiProgramId for WalkProgram {
	const ID: Address = ID;
}

#[cfg(feature = "bpf-entrypoint")]
pub mod entrypoint {
	use super::*;

	nostd_entrypoint!(process_instruction);

	#[inline]
	pub fn process_instruction(
		program_id: &Address,
		accounts: &mut [AccountView],
		data: &[u8],
	) -> ProgramResult {
		let instruction: WalkInstruction = parse_instruction(program_id, &ID, data)?;
		match instruction {
			WalkInstruction::Update => {
				UpdateAccounts::try_from((program_id, accounts))?.process(data)
			}
		}
	}
}
`;
}

// ---------------------------------------------------------------------------
// Surfpool network + client submission helpers.
// ---------------------------------------------------------------------------

type SurfnetInstance = ReturnType<typeof Surfnet.start>;

interface Network {
	surfnet: SurfnetInstance;
	rpcUrl: string;
	payer: Awaited<ReturnType<typeof createKeyPairSignerFromBytes>>;
	submit: (instruction: Instruction) => Promise<string>;
	accountData: (address: string) => Promise<Uint8Array | undefined>;
}

async function startNetwork(): Promise<Network> {
	const surfnet = Surfnet.start();
	const payer = await createKeyPairSignerFromBytes(surfnet.payerSecretKey);

	const rpc = createSolanaRpc(surfnet.rpcUrl);
	const send = sendTransactionWithoutConfirmingFactory({ rpc });
	const submit = async (instruction: Instruction): Promise<string> => {
		for (let attempt = 0;; attempt += 1) {
			try {
				return await submitOnce(instruction);
			} catch (error) {
				if (attempt >= 2 || !(error instanceof TypeError)) {
					throw error;
				}
				console.error(
					`submit attempt ${attempt + 1} failed transiently; retrying`,
				);
				await new Promise((resolve) => setTimeout(resolve, 2000));
			}
		}
	};

	const submitOnce = async (instruction: Instruction): Promise<string> => {
		const { value: latestBlockhash } = await rpc
			.getLatestBlockhash()
			.send();
		const message = setTransactionMessageLifetimeUsingBlockhash(
			latestBlockhash,
			appendTransactionMessageInstruction(
				instruction,
				setTransactionMessageFeePayerSigner(
					payer,
					createTransactionMessage({ version: 0 }),
				),
			),
		);
		const signed = await signTransactionMessageWithSigners(message);
		const simulated = await rpc.simulateTransaction(
			getBase64EncodedWireTransaction(signed),
			{ encoding: "base64", sigVerify: true },
		).send();
		if (simulated.value.err !== null) {
			throw new Error(
				`transaction failed: ${String(simulated.value.err)}\n${
					(simulated.value.logs ?? []).join("\n")
				}`,
			);
		}
		const signature = getSignatureFromTransaction(signed);
		await send(signed, { commitment: "confirmed" });
		const statuses = await rpc
			.getSignatureStatuses([signature], {
				searchTransactionHistory: true,
			})
			.send();
		const status = statuses.value[0];
		if (!status || status.err !== null) {
			throw new Error(
				`submitted transaction ${signature} failed: ${
					JSON.stringify(status?.err)
				}`,
			);
		}
		return signature;
	};
	const accountData = async (addressText: string): Promise<
		Uint8Array | undefined
	> => {
		const { value: account } = await rpc
			.getAccountInfo(address(addressText), { encoding: "base64" })
			.send();
		if (!account) {
			return undefined;
		}
		const [encoded, encoding] = account.data;
		if (encoding !== "base64") {
			throw new Error(`unexpected account encoding ${encoding}`);
		}
		return new Uint8Array(Buffer.from(encoded, "base64"));
	};

	return { surfnet, rpcUrl: surfnet.rpcUrl, payer, submit, accountData };
}

interface LoadedClient {
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	update: any;
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	profile: any;
}

/** Capture the logical profile state for network-restart recovery. */
/** Borrow the captured profile bytes as a DataView for assertions. */
function profileView(data: Uint8Array | undefined): DataView {
	if (!data) {
		throw new Error("profile account missing after submission");
	}
	return new DataView(data.buffer, data.byteOffset, data.byteLength);
}

function remember(context: StepContext, data: Uint8Array | undefined): void {
	if (!data) {
		return;
	}
	context.profileBytes = new Uint8Array(data);
}

async function loadClient(clientRoot: string): Promise<LoadedClient> {
	const update = await import(
		`${clientRoot}/src/generated/instructions/update.ts`
	);
	const profile = await import(
		`${clientRoot}/src/generated/accounts/profile.ts`
	);
	return { update, profile };
}

function clientInstruction(
	client: LoadedClient,
	network: Network,
	profileAddress: string,
	score: number,
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
): any {
	return client.update.getUpdateInstruction({
		authority: network.payer,
		profile: profileAddress,
		migrationPayer: network.payer,
		systemProgram: "11111111111111111111111111111111",
		score: BigInt(score),
	});
}

function instructionAccounts(instruction: Instruction): number {
	return instruction.accounts?.length ?? 0;
}

// ---------------------------------------------------------------------------
// Deployment plumbing shared by every step.
// ---------------------------------------------------------------------------

interface Deployed {
	rpcUrl: string;
}

function buildSbf(): string {
	mustRun(
		"cargo",
		[
			"build-sbf",
			"--manifest-path",
			resolve(PROGRAM_DIR, "Cargo.toml"),
			"--sbf-out-dir",
			DEPLOY_DIR,
		],
		{},
		"build-sbf",
	);
	const artifact = resolve(DEPLOY_DIR, `${PROGRAM_NAME}.so`);
	if (!existsSync(artifact)) {
		throw new Error(`SBF artifact missing: ${artifact}`);
	}
	return artifact;
}

function generateClients(step: number): string {
	const jsRoot = resolve(CLIENTS, "js", PROGRAM_NAME);
	rmSync(resolve(CLIENTS, "js"), { recursive: true, force: true });
	mustRun(
		"cargo",
		[
			"run",
			"-q",
			"-p",
			"pina_cli",
			"--locked",
			"--",
			"codama",
			"generate",
			"--npx",
			"node",
			"--examples-dir",
			resolve(WORK, "examples"),
			"--idls-dir",
			IDLS_DIR,
			"--rust-out",
			resolve(CLIENTS, "rust"),
			"--cpi-out",
			resolve(CLIENTS, "cpi"),
			"--js-out",
			resolve(CLIENTS, "js"),
			"--dart-out",
			resolve(CLIENTS, "dart"),
		],
		{},
		"codama generate",
	);
	const historyRoot = resolve(CLIENT_HISTORY, `step-${step}`, "js");
	mkdirSync(resolve(historyRoot, PROGRAM_NAME), { recursive: true });
	cpSync(jsRoot, resolve(historyRoot, PROGRAM_NAME), {
		recursive: true,
	});
	return jsRoot;
}

async function deploy(
	context: StepContext,
	artifact: string,
): Promise<Deployed> {
	try {
		context.network.surfnet.deploy({
			programId: context.programId,
			soPath: artifact,
		});
	} catch {
		// Surfpool's engine occasionally dies under the memory pressure of an
		// SBF build. Restart the network, reload the newest artifact, and
		// restore the logical account state so the narrative continues.
		console.error("Surfpool network died; restarting and restoring state");
		context.network = await startNetwork();
		context.network.surfnet.deploy({
			programId: context.programId,
			soPath: artifact,
		});
		context.network.surfnet.setAccount(
			context.profileAddress,
			context.profileLamports,
			context.profileBytes,
			context.programId,
		);
	}
	return { rpcUrl: context.network.rpcUrl };
}

function recordDeployment(artifact: string, rpcUrl: string): string {
	return mustPina(
		[
			"deploy",
			"--project",
			PROGRAM_DIR,
			"--program",
			artifact,
			"--program-keypair",
			resolve(DEPLOY_DIR, "walkthrough-keypair.json"),
			"--upgrade-authority",
			resolve(DEPLOY_DIR, "walkthrough-authority.json"),
			"--payer",
			resolve(DEPLOY_DIR, "walkthrough-authority.json"),
			"--cluster",
			rpcUrl,
			"--record-publication",
			"--remote-command",
			"true",
			"--yes",
		],
		PROGRAM_DIR,
		"record deployment",
	);
}

function migrationStatus(): string {
	return mustPina(
		["migrations", "status", "--project", PROGRAM_DIR],
		PROGRAM_DIR,
		"migration status",
	);
}

// ---------------------------------------------------------------------------
// Steps.
// ---------------------------------------------------------------------------

interface StepContext {
	network: Network;
	programId: string;
	authority: string;
	profileAddress: string;
	journalAddress: string;
	profileBytes: Uint8Array;
	profileLamports: number;
}

async function step1_genesis(context: StepContext): Promise<void> {
	say("Step 1 — initial deployment: every contract is migratable from day one");
	const output = mustPina(
		["migrations", "make", "--project", PROGRAM_DIR, "--no-interactive"],
		PROGRAM_DIR,
		"initial make",
	);
	expect(
		output.includes("Created account:1:01@0") &&
			output.includes("Created account:1:02@0") &&
			output.includes("Created instruction:1:00@0") &&
			output.includes("Created event:1:03@0"),
		"make captures Profile, Journal, Update, and ProfileChanged at version 0",
	);
	const artifact = buildSbf();
	await deploy(context, artifact);
	const recorded = recordDeployment(artifact, context.network.rpcUrl);
	expect(
		recorded.includes("Deployment complete"),
		"deployment records the first publication receipt",
	);
	generateClients(1);
	const client = await loadClient(resolve(CLIENTS, "js", PROGRAM_NAME));
	const instruction = clientInstruction(
		client,
		context.network,
		context.profileAddress,
		42,
	);
	expect(
		instruction.data[0] === 0 && instruction.data[1] === 0,
		"the v0 client writes the version-0 envelope without caller input",
	);
	await context.network.submit(instruction);
	const data = await context.network.accountData(context.profileAddress);
	remember(context, data);
	// Discriminator (1) + version (1) + authority (32) + score (8).
	expect(
		data?.length === 42 && data[1] === 0,
		"the on-chain profile carries version 0 after the first submission",
	);
}

async function step2_add_field(context: StepContext): Promise<void> {
	say("Step 2 — add a field: automatic zero-initialized migration");
	const source = readFileSync(
		resolve(PROGRAM_DIR, "src/lib.rs"),
		"utf8",
	);
	writeProgramSource(
		source.replace(
			"pub struct Profile {\n\tpub authority: Address,\n\tpub score: u64,\n}",
			"pub struct Profile {\n\tpub authority: Address,\n\tpub score: u64,\n\tpub active: bool,\n}",
		),
	);
	const output = mustPina(
		["migrations", "make", "--project", PROGRAM_DIR, "--no-interactive"],
		PROGRAM_DIR,
		"make after adding a field",
	);
	expect(
		output.includes("Advanced account:1:01@1"),
		"adding `active` advances Profile to version 1",
	);
	const artifact = buildSbf();
	await deploy(context, artifact);
	recordDeployment(artifact, context.network.rpcUrl);
	generateClients(2);

	// The account still holds v0 bytes: the current client must refuse to
	// decode them through the v1 layout instead of silently misparsing.
	const currentClient = await loadClient(
		resolve(CLIENT_HISTORY, "step-2", "js", PROGRAM_NAME),
	);
	const staleBytes = await context.network.accountData(context.profileAddress);
	let staleRejection: string | undefined;
	try {
		currentClient.profile
			.getProfileDecoder()
			.read(staleBytes as ReadonlyUint8Array, 0);
	} catch (error) {
		staleRejection = String(error);
	}
	expect(
		staleRejection?.includes("stale migration version") === true,
		`the current client rejects a stale account with an actionable error (got: ${staleRejection})`,
	);

	const oldClient = await loadClient(
		resolve(CLIENT_HISTORY, "step-1", "js", PROGRAM_NAME),
	);
	await context.network.submit(
		clientInstruction(
			oldClient,
			context.network,
			context.profileAddress,
			50,
		),
	);
	const data = await context.network.accountData(context.profileAddress);
	remember(context, data);
	expect(
		data?.length === 43 && data[1] === 1 && data[42] === 0,
		"the step-1 client still submits; the account migrated to v1 with `active` zeroed",
	);
}

async function step3_rename(context: StepContext): Promise<void> {
	say("Step 3 — rename a field through the disambiguation flow");
	const source = readFileSync(
		resolve(PROGRAM_DIR, "src/lib.rs"),
		"utf8",
	);
	writeProgramSource(
		source
			.replace("pub score: u64,", "pub points: u64,")
			.replace("instruction.score.set(", "instruction.points.set(")
			.replace("profile.score.set(", "profile.points.set("),
	);
	const question = pina([
		"migrations",
		"make",
		"--project",
		PROGRAM_DIR,
		"--no-interactive",
	]);
	expect(
		question.status !== 0 &&
			question.stderr.includes("was `score` renamed to `points`") &&
			question.stderr.includes("--rename score:points"),
		"unanswered renames fail with the exact command that answers them",
	);
	const json = pina([
		"migrations",
		"make",
		"--project",
		PROGRAM_DIR,
		"--no-interactive",
		"--json",
	]);
	expect(
		json.stdout.includes('"from": "score"') &&
			json.stdout.includes('"to": "points"'),
		"--json emits machine-readable questions for agents",
	);
	const output = mustPina(
		[
			"migrations",
			"make",
			"--project",
			PROGRAM_DIR,
			"--no-interactive",
			"--rename",
			"score:points",
		],
		PROGRAM_DIR,
		"make with rename answer",
	);
	expect(
		output.includes("Advanced account:1:01@2"),
		"the answered rename advances Profile to version 2",
	);
	const transition = readFileSync(
		resolve(
			PROGRAM_DIR,
			"migrations/transitions/account_1_01/v1_to_v2.rs",
		),
		"utf8",
	);
	expect(
		transition.includes("@generated"),
		"the rename generates an automatic byte-preserving transition",
	);
	const artifact = buildSbf();
	await deploy(context, artifact);
	recordDeployment(artifact, context.network.rpcUrl);
	generateClients(3);

	const oldClient = await loadClient(
		resolve(CLIENT_HISTORY, "step-1", "js", PROGRAM_NAME),
	);
	await context.network.submit(
		clientInstruction(
			oldClient,
			context.network,
			context.profileAddress,
			77,
		),
	);
	const data = await context.network.accountData(context.profileAddress);
	remember(context, data);
	expect(
		data?.length === 43 && data[1] === 2 &&
			Number(profileView(data).getBigUint64(34, true)) === 77,
		"old clients keep submitting; the renamed field preserves its data",
	);
}

async function step4_type_change(context: StepContext): Promise<void> {
	say("Step 4 — change a field type through a manual transition");
	const source = readFileSync(
		resolve(PROGRAM_DIR, "src/lib.rs"),
		"utf8",
	);
	writeProgramSource(
		source
			.replace("pub points: u64,", "pub points: u32,")
			.replace(
				"profile.points.set(instruction.score.get());",
				[
					"profile.points.set(",
					"				u32::try_from(instruction.score.get()).unwrap_or(u32::MAX),",
					"			);",
				].join("\n"),
			),
	);
	const output = mustPina(
		["migrations", "make", "--project", PROGRAM_DIR, "--no-interactive"],
		PROGRAM_DIR,
		"make after type change",
	);
	expect(
		output.includes("Manual migration required") &&
			output.includes("v2_to_v3.rs"),
		"type changes fall back to a manual TODO transition",
	);
	const checked = pina([
		"migrations",
		"check",
		"--project",
		PROGRAM_DIR,
	]);
	expect(
		checked.status !== 0,
		"check refuses the unfinished manual transition",
	);
	// Agent fills the TODO: narrow u64 -> u32, saturating at u32::MAX.
	const transitionPath = resolve(
		PROGRAM_DIR,
		"migrations/transitions/account_1_01/v2_to_v3.rs",
	);
	const filled = readFileSync(transitionPath, "utf8")
		.replace(
			"// TODO(pina-manual-migration): the source shape is preflighted; this conversion must be total and fully initialize destination.\n",
			"// Narrows the u64 points value into u32, saturating at u32::MAX.\n",
		)
		.replace(
			"pub(crate) fn migrate(data: &mut [u8]) {\n\tif data.len() < WORKING_SIZE {\n\t\treturn;\n\t}\n\tlet _ = data;\n}",
			[
				"pub(crate) fn migrate(data: &mut [u8]) {",
				"\tif data.len() < WORKING_SIZE {",
				"\t\treturn;",
				"\t}",
				"\tlet wide = u64::from_le_bytes([",
				"\t\tdata[34], data[35], data[36], data[37], data[38], data[39], data[40],",
				"\t\tdata[41],",
				"\t]);",
				"\tlet narrowed = u32::try_from(wide).unwrap_or(u32::MAX);",
				"\tdata[34..38].copy_from_slice(&narrowed.to_le_bytes());",
				"\tdata[38..43].fill(0);",
				"}",
			].join("\n"),
		);
	writeFileSync(transitionPath, filled);
	const finalized = mustPina(
		["migrations", "make", "--project", PROGRAM_DIR, "--no-interactive"],
		PROGRAM_DIR,
		"make records the implemented transition",
	);
	expect(
		finalized.includes("Updated draft account:1:01@3"),
		"re-running make refreshes the draft with the manual body's recorded hash",
	);
	const artifact = buildSbf();
	await deploy(context, artifact);
	recordDeployment(artifact, context.network.rpcUrl);
	generateClients(4);
	const client = await loadClient(resolve(CLIENTS, "js", PROGRAM_NAME));
	await context.network.submit(
		clientInstruction(client, context.network, context.profileAddress, 78),
	);
	const data = await context.network.accountData(context.profileAddress);
	remember(context, data);
	expect(
		data?.length === 39 && data[1] === 3 &&
			profileView(data).getUint32(34, true) === 78,
		"the stored value narrows into the u32 field during migration",
	);
}

async function step5_compact_growth(context: StepContext): Promise<void> {
	say("Step 5 — grow the compact account with a manual relayout");
	const source = readFileSync(
		resolve(PROGRAM_DIR, "src/lib.rs"),
		"utf8",
	);
	writeProgramSource(
		source.replace(
			"pub struct Journal {\n\tpub title: String<12>,\n}",
			"pub struct Journal {\n\tpub title: String<12>,\n\tpub tags: Vec<u16, 3>,\n}",
		),
	);
	const output = mustPina(
		["migrations", "make", "--project", PROGRAM_DIR, "--no-interactive"],
		PROGRAM_DIR,
		"make after compact growth",
	);
	expect(
		output.includes("Manual migration required") &&
			output.includes("account_1_02"),
		"compact growth creates a manual sizing transition",
	);
	const transitionPath = resolve(
		PROGRAM_DIR,
		"migrations/transitions/account_1_02/v0_to_v1.rs",
	);
	const filled = readFileSync(transitionPath, "utf8")
		.replace(
			"// TODO(pina-manual-migration): the source shape is preflighted; this conversion must be total and fully initialize destination.\n",
			"// Appends a zeroed tags tail after the title.\n",
		)
		.replace(
			"pub(crate) fn target_size(data: &[u8]) -> Option<usize> {\n\tlet _ = data;\n\tNone\n}",
			[
				"pub(crate) fn target_size(data: &[u8]) -> Option<usize> {",
				"\tif data.len() < 3 {",
				"\t\treturn None;",
				"\t}",
				"\tlet title_len = usize::from(data[2]);",
				"\tSome(3 + title_len + 2)",
				"}",
			].join("\n"),
		)
		.replace(
			"pub(crate) fn migrate(data: &mut [u8]) {\n\tlet _ = data;\n}",
			[
				"pub(crate) fn migrate(data: &mut [u8]) {",
				"\tif data.len() < 3 {",
				"\t\treturn;",
				"\t}",
				"\tlet title_len = usize::from(data[2]);",
				"\tif data.len() < 3 + title_len + 2 {",
				"\t\treturn;",
				"\t}",
				"\tdata[3 + title_len..3 + title_len + 2].fill(0);",
				"}",
			].join("\n"),
		);
	writeFileSync(transitionPath, filled);
	mustPina(
		["migrations", "make", "--project", PROGRAM_DIR, "--no-interactive"],
		PROGRAM_DIR,
		"make records the compact transition",
	);
	const artifact = buildSbf();
	await deploy(context, artifact);
	recordDeployment(artifact, context.network.rpcUrl);
	generateClients(5);
	const status = migrationStatus();
	expect(
		status.includes("Journal v1"),
		"Journal advances to v1 with the appended tags tail",
	);
}

async function step6_instruction_payload(context: StepContext): Promise<void> {
	say("Step 6 — extend the instruction payload; v0 clients keep working");
	const source = readFileSync(
		resolve(PROGRAM_DIR, "src/lib.rs"),
		"utf8",
	);
	writeProgramSource(
		source.replace(
			"pub struct UpdateInstruction {\n\tpub score: u64,\n}",
			"pub struct UpdateInstruction {\n\tpub score: u64,\n\tpub memo: u16,\n}",
		).replace(
			"let _ = instruction;",
			"let _ = instruction.memo.get();",
		),
	);
	const output = mustPina(
		["migrations", "make", "--project", PROGRAM_DIR, "--no-interactive"],
		PROGRAM_DIR,
		"make after payload growth",
	);
	expect(
		output.includes("Advanced instruction:1:00@1"),
		"adding `memo` advances the instruction to version 1",
	);
	const artifact = buildSbf();
	await deploy(context, artifact);
	recordDeployment(artifact, context.network.rpcUrl);
	generateClients(6);
	const oldClient = await loadClient(
		resolve(CLIENT_HISTORY, "step-1", "js", PROGRAM_NAME),
	);
	await context.network.submit(
		clientInstruction(
			oldClient,
			context.network,
			context.profileAddress,
			90,
		),
	);
	const data = await context.network.accountData(context.profileAddress);
	remember(context, data);
	expect(
		data?.[1] === 3 &&
			profileView(data).getUint32(34, true) === 90,
		"the v0 instruction payload normalizes and the handler still runs",
	);
}

async function step7_optional_accounts(context: StepContext): Promise<void> {
	say("Step 7 — append optional accounts to the process contract");
	const source = readFileSync(
		resolve(PROGRAM_DIR, "src/lib.rs"),
		"utf8",
	);
	writeProgramSource(
		source.replace(
			"pub system_program: Option<&'a AccountView>,\n}",
			"pub system_program: Option<&'a AccountView>,\n\tpub referrer: Option<&'a AccountView>,\n}",
		).replace(
			"system_program.assert_address(&system::ID)?;",
			"system_program.assert_address(&system::ID)?;\n\t\t\tlet _ = self.referrer;",
		),
	);
	const output = mustPina(
		["migrations", "make", "--project", PROGRAM_DIR, "--no-interactive"],
		PROGRAM_DIR,
		"make after process growth",
	);
	expect(
		output.includes("Advanced instruction:1:00@2"),
		"appending an optional account advances the instruction to version 2",
	);
	const artifact = buildSbf();
	await deploy(context, artifact);
	recordDeployment(artifact, context.network.rpcUrl);
	generateClients(7);
	const oldClient = await loadClient(
		resolve(CLIENT_HISTORY, "step-1", "js", PROGRAM_NAME),
	);
	await context.network.submit(
		clientInstruction(
			oldClient,
			context.network,
			context.profileAddress,
			95,
		),
	);
	const data = await context.network.accountData(context.profileAddress);
	remember(context, data);
	expect(
		profileView(data).getUint32(34, true) === 95,
		"old clients omit the appended optional account and still succeed",
	);
}

async function step8_event(context: StepContext): Promise<void> {
	say("Step 8 — extend the event; historical log bytes stay decodable");
	const source = readFileSync(
		resolve(PROGRAM_DIR, "src/lib.rs"),
		"utf8",
	);
	writeProgramSource(
		source.replace(
			"pub struct ProfileChanged {\n\tpub score: u64,\n}",
			"pub struct ProfileChanged {\n\tpub score: u64,\n\tpub reason: u8,\n}",
		),
	);
	const output = mustPina(
		["migrations", "make", "--project", PROGRAM_DIR, "--no-interactive"],
		PROGRAM_DIR,
		"make after event growth",
	);
	expect(
		output.includes("Advanced event:1:03@1"),
		"adding `reason` advances the event to version 1",
	);
	const artifact = buildSbf();
	await deploy(context, artifact);
	recordDeployment(artifact, context.network.rpcUrl);
	generateClients(8);
	// The Codama IDL intentionally renders only accounts, instructions, and
	// defined types; event history stays in the migration manifest. The
	// appended field is proven by the generated transition, which re-encodes
	// old log bytes into the new shape off-chain.
	const transition = readFileSync(
		resolve(
			PROGRAM_DIR,
			"migrations/transitions/event_1_03/v0_to_v1.rs",
		),
		"utf8",
	);
	expect(
		transition.includes("@generated"),
		"appending an event field generates an automatic projection transition",
	);
}

async function step9_removal(context: StepContext): Promise<void> {
	say("Step 9 — remove a field with an explicit data-loss acknowledgement");
	const source = readFileSync(
		resolve(PROGRAM_DIR, "src/lib.rs"),
		"utf8",
	);
	writeProgramSource(
		source.replace("\tpub active: bool,\n", ""),
	);
	const question = pina([
		"migrations",
		"make",
		"--project",
		PROGRAM_DIR,
		"--no-interactive",
	]);
	expect(
		question.status !== 0 &&
			question.stderr.includes("--assume-removed active"),
		"removals demand an explicit data-loss acknowledgement",
	);
	const output = mustPina(
		[
			"migrations",
			"make",
			"--project",
			PROGRAM_DIR,
			"--no-interactive",
			"--assume-removed",
			"active",
		],
		PROGRAM_DIR,
		"make with removal answer",
	);
	expect(
		output.includes("Advanced account:1:01@4"),
		"the acknowledged removal advances Profile to version 4",
	);
	expect(
		output.includes("field `active`"),
		"the data-loss warning names the discarded field",
	);
	const artifact = buildSbf();
	await deploy(context, artifact);
	recordDeployment(artifact, context.network.rpcUrl);
	generateClients(9);
}

/** Compile-check the generated Rust client and analyze the Dart client. */
function verifyGeneratedClients(): void {
	const rustClient = resolve(CLIENTS, "rust", PROGRAM_NAME);
	// Generated client crates inherit the Pina workspace's dependency table.
	// Convert those inheritance entries to the versions the repository pins
	// so the client compiles as a standalone crate.
	const rustManifest = readFileSync(resolve(rustClient, "Cargo.toml"), "utf8");
	const rootManifest = readFileSync(resolve(REPO, "Cargo.toml"), "utf8");
	const dependencyBlock = rootManifest.slice(
		rootManifest.indexOf("[workspace.dependencies]"),
	);
	const pinned = rustManifest.replace(
		/(\w[\w-]*) = \{ workspace = true, ([^}]*) \}/g,
		(_match, name: string, rest: string) => {
			if (name === "pina") {
				return `{ path = ${
					JSON.stringify(`${REPO}/crates/pina`)
				}, version = "0.15.0", features = ["compact"] }`
					.replace("{", `{pina = `)
					.replace(/^\{pina = /, "pina = {");
			}
			const line = dependencyBlock
				.split("\n")
				.find((candidate) => candidate.startsWith(`${name} = `));
			if (!line) {
				throw new Error(
					`dependency ${name} missing from the workspace table`,
				);
			}
			const version = line.match(/version = "([^"]+)"/)?.[1];
			if (!version) {
				throw new Error(`dependency ${name} has no pinned version`);
			}
			// The generated entry decides feature flags; the workspace only
			// contributes the version.
			return `${name} = { version = "${version}", ${rest.trim()} }`;
		},
	);
	writeFileSync(
		resolve(rustClient, "Cargo.toml"),
		`${pinned}\n[workspace]\n`,
	);
	const rustCheck = run("cargo", ["check", "--quiet"], {
		cwd: rustClient,
		env: { CARGO_TARGET_DIR: resolve(WORK, "rust-client-target") },
	});
	expect(
		rustCheck.status === 0,
		"the generated Rust client compiles against the final ABI",
	);
	if (run("which", ["dart"]).status === 0) {
		const dartClient = resolve(CLIENTS, "dart");
		mustRun("dart", ["pub", "get"], { cwd: dartClient }, "dart pub get");
		const dartAnalyze = run("dart", ["analyze", "--no-fatal-warnings"], {
			cwd: dartClient,
		});
		expect(
			dartAnalyze.status === 0,
			"the generated Dart client analyzes cleanly against the final ABI",
		);
	}
	const profileAccount = readFileSync(
		resolve(rustClient, "src/generated/accounts/profile.rs"),
		"utf8",
	);
	expect(
		profileAccount.includes("PROFILE_MIGRATION_VERSION"),
		"the Rust client embeds the current migration version constant",
	);
}

async function step10_full_ladder(context: StepContext): Promise<void> {
	say(
		"Step 10 — a day-one account climbs the entire ladder in one transaction",
	);
	// Install a pristine version-0 profile, exactly as the step-1 deployment
	// would have written it.
	const authorityBytes = new Uint8Array(32).fill(9);
	const v0 = new Uint8Array(42);
	v0[0] = 1; // WalkAccount::Profile
	v0[1] = 0; // version 0
	v0.set(authorityBytes, 2);
	new DataView(v0.buffer).setBigUint64(34, 123n, true);
	context.network.surfnet.setAccount(
		context.profileAddress,
		5_000_000,
		v0,
		context.programId,
	);
	const client = await loadClient(resolve(CLIENTS, "js", PROGRAM_NAME));
	await context.network.submit(
		clientInstruction(
			client,
			context.network,
			context.profileAddress,
			124,
		),
	);
	const data = await context.network.accountData(context.profileAddress);
	remember(context, data);
	expect(
		data?.[1] === 4 &&
			profileView(data).getUint32(34, true) === 124,
		"the v0 account walks v0 to v4 atomically and lands on the new value",
	);
	const status = migrationStatus();
	expect(
		status.includes("Migration history is consistent"),
		"the final history is consistent across every contract",
	);
	verifyGeneratedClients();

	const publications = readFileSync(
		resolve(PROGRAM_DIR, "migrations/publications.json"),
		"utf8",
	);
	const receipts = (JSON.parse(publications).receipts as unknown[]).length;
	expect(
		receipts === 9,
		`every deployment (steps 1-9) recorded an immutable receipt (found ${receipts})`,
	);
}

const STEPS: Array<[number, string, (context: StepContext) => Promise<void>]> =
	[
		[1, "genesis", step1_genesis],
		[2, "add field", step2_add_field],
		[3, "rename", step3_rename],
		[4, "type change", step4_type_change],
		[5, "compact growth", step5_compact_growth],
		[6, "instruction payload", step6_instruction_payload],
		[7, "optional accounts", step7_optional_accounts],
		[8, "event projection", step8_event],
		[9, "explicit removal", step9_removal],
		[10, "full ladder", step10_full_ladder],
	];

async function main(): Promise<void> {
	console.log(
		`Pina migrations walkthrough — steps ${FROM_STEP}..${TO_STEP}`,
	);
	if (!KEEP) {
		rmSync(WORK, { recursive: true, force: true });
	}
	mkdirSync(DEPLOY_DIR, { recursive: true });
	mkdirSync(CLIENT_HISTORY, { recursive: true });

	const keypair = resolve(DEPLOY_DIR, "walkthrough-keypair.json");
	const authority = resolve(DEPLOY_DIR, "walkthrough-authority.json");
	const pubkeyOf = (keypairPath: string) => {
		const result = mustRun(
			"solana-keygen",
			["pubkey", keypairPath],
			{},
			`derive pubkey for ${keypairPath}`,
		);
		const pubkey = result.stdout.trim();
		if (pubkey.length === 0) {
			throw new Error(
				`solana-keygen emitted an empty pubkey for ${keypairPath}`,
			);
		}
		return pubkey;
	};
	if (!existsSync(keypair)) {
		mustRun(
			"solana-keygen",
			[
				"new",
				"--no-bip39-passphrase",
				"-o",
				keypair,
				"--force",
			],
			{},
			"generate walkthrough keypair",
		);
	}
	if (!existsSync(authority)) {
		mustRun(
			"solana-keygen",
			[
				"new",
				"--no-bip39-passphrase",
				"-o",
				authority,
				"--force",
			],
			{},
			"generate walkthrough authority",
		);
	}
	const programId = pubkeyOf(keypair);
	const authorityId = pubkeyOf(authority);
	console.log(`program:   ${programId}`);
	console.log(`authority: ${authorityId}`);

	if (!existsSync(resolve(PROGRAM_DIR, "src/lib.rs"))) {
		scaffoldProgram(programId);
	}

	const network = await startNetwork();
	const profileAddress = Surfnet.newKeypair().publicKey;
	const journalAddress = Surfnet.newKeypair().publicKey;
	network.surfnet.fundSol(String(profileAddress), 1_000_000);
	network.surfnet.fundSol(String(journalAddress), 1_000_000);
	// Seed a version-0 profile exactly as the first deployment's clients
	// would have written it, so later steps demonstrate real on-chain
	// migration rather than fresh account creation.
	const seed = new Uint8Array(42);
	seed[0] = 1;
	seed[1] = 0;
	const authorityBytes = new Uint8Array(
		await getAddressEncoder().encode(network.payer.address),
	);
	seed.set(authorityBytes, 2);
	new DataView(seed.buffer).setBigUint64(34, 11n, true);
	network.surfnet.setAccount(
		String(profileAddress),
		5_000_000,
		seed,
		programId,
	);
	network.surfnet.fundSol(String(journalAddress), 1_000_000);

	const context: StepContext = {
		network,
		programId,
		authority: authorityId,
		profileAddress: String(profileAddress),
		journalAddress: String(journalAddress),
		profileBytes: seed,
		profileLamports: 5_000_000,
	};

	// A resumed run starts a fresh network: deploy the current artifact and
	// restore the captured account state before the requested steps run.
	if (FROM_STEP > 1) {
		const artifact = resolve(DEPLOY_DIR, `${PROGRAM_NAME}.so`);
		if (existsSync(artifact)) {
			await deploy(context, artifact);
			context.network.surfnet.setAccount(
				context.profileAddress,
				context.profileLamports,
				context.profileBytes,
				context.programId,
			);
		} else {
			throw new Error(
				`--from-step ${FROM_STEP} needs a previous run's artifact; run the full walkthrough first or pass --from-step 1`,
			);
		}
	}

	const startedAt = Date.now();
	for (const [number, , step] of STEPS) {
		if (number < FROM_STEP || number > TO_STEP) {
			continue;
		}
		await step(context);
	}
	const elapsed = Math.round((Date.now() - startedAt) / 1000);
	console.log(
		`\nwalkthrough finished in ${elapsed}s with ${failures} failure(s)`,
	);
	if (failures > 0) {
		process.exitCode = 1;
	}
}

await main();
