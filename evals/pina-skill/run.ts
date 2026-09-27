// Command-line runner for the Pina skill evaluation harness.
//
//   pnpm exec tsx evals/pina-skill/run.ts --list
//   pnpm exec tsx evals/pina-skill/run.ts --scenario add-field --variant baseline
//   pnpm exec tsx evals/pina-skill/run.ts --all --variant baseline --repeats 3
//
// Results land in `results/` as JSON plus a Markdown scorecard.

import {
	existsSync,
	mkdirSync,
	readdirSync,
	readFileSync,
	writeFileSync,
} from "node:fs";
import { basename, join } from "node:path";
import process from "node:process";

import { runAgent, runWorkdir } from "./lib/agent.ts";
import { allPassed, gradeAll } from "./lib/grade.ts";
import {
	copyFixture,
	ensureParent,
	exec,
	HARNESS_ROOT,
	resolvePinaCli,
	RESULT_DIR,
	SCENARIO_DIR,
	SKILL_VARIANT_DIR,
	WORK_DIR,
} from "./lib/paths.ts";
import type { Report, RunResult, Scenario } from "./lib/types.ts";

interface Options {
	scenarios: string[];
	variants: string[];
	repeats: number;
	model: string;
	timeoutSeconds: number;
	list: boolean;
	instructionVariant?: string;
	keep: boolean;
	/// Re-grade saved workdirs instead of driving the agent again.
	regrade: boolean;
	/// How the skill reaches the agent under test.
	skillSource: "installed" | "cli";
}

const DEFAULT_TOOLS = [
	"Read",
	"Write",
	"Edit",
	"Bash",
	"Glob",
	"Grep",
	"Skill",
	"TodoWrite",
];

function parseArgs(argv: string[]): Options {
	const value = (flag: string): string | undefined => {
		const index = argv.indexOf(flag);
		return index === -1 ? undefined : argv[index + 1];
	};
	const many = (flag: string): string[] => {
		const collected: string[] = [];
		for (let index = 0; index < argv.length; index += 1) {
			if (argv[index] === flag && argv[index + 1]) {
				collected.push(argv[index + 1] as string);
			}
		}
		return collected;
	};

	return {
		scenarios: many("--scenario"),
		variants: many("--variant"),
		repeats: Number(value("--repeats") ?? "1"),
		model: value("--model") ?? process.env["PINA_EVAL_MODEL"] ?? "sonnet",
		timeoutSeconds: Number(value("--timeout") ?? "900"),
		list: argv.includes("--list"),
		instructionVariant: value("--instruction-variant"),
		keep: argv.includes("--keep"),
		regrade: argv.includes("--regrade"),
		skillSource: argv.includes("--skill-source") &&
				process.argv[process.argv.indexOf("--skill-source") + 1] === "cli"
			? "cli"
			: "installed", // the flag is the last token, so indexOf(+1) cannot cross into another flag
	};
}

function loadScenarios(): Scenario[] {
	if (!existsSync(SCENARIO_DIR)) {
		return [];
	}
	const scenarios: Scenario[] = [];
	for (const entry of readdirSync(SCENARIO_DIR).sort()) {
		if (!entry.endsWith(".json")) {
			continue;
		}
		const path = join(SCENARIO_DIR, entry);
		const parsed = JSON.parse(readFileSync(path, "utf8")) as Scenario;
		scenarios.push(parsed);
	}
	return scenarios;
}

function loadVariants(): string[] {
	if (!existsSync(SKILL_VARIANT_DIR)) {
		return [];
	}
	return readdirSync(SKILL_VARIANT_DIR, { withFileTypes: true })
		.filter((entry) => entry.isDirectory())
		.map((entry) => entry.name)
		.sort();
}

/// Path to a run workdir, without clearing it when re-grading.
function runWorkdirFor(
	regrade: boolean,
	scenarioId: string,
	variant: string,
	repeat: number,
): string {
	const path = join(WORK_DIR, "runs", `${scenarioId}__${variant}__${repeat}`);
	if (regrade) {
		if (!existsSync(path)) {
			throw new Error(`No saved workdir to re-grade: ${path}`);
		}
		return path;
	}
	return runWorkdir(scenarioId, variant, repeat);
}

/// Rebuild the assistant-authored transcript from a saved run, so a re-grade
/// compares against the same text the original grade saw.
function readTranscript(
	scenarioId: string,
	variant: string,
	repeat: number,
): string {
	const path = join(
		RESULT_DIR,
		"transcripts",
		`${scenarioId}__${variant}__${repeat}`,
		"transcript.jsonl",
	);
	if (!existsSync(path)) {
		return "";
	}
	const parts: string[] = [];
	for (const line of readFileSync(path, "utf8").split("\n")) {
		if (line.trim() === "") {
			continue;
		}
		let parsed: Record<string, unknown>;
		try {
			parsed = JSON.parse(line) as Record<string, unknown>;
		} catch {
			continue;
		}
		if (parsed["type"] !== "assistant") {
			continue;
		}
		const content = (parsed["message"] as Record<string, unknown> | undefined)
			?.[
				"content"
			];
		if (!Array.isArray(content)) {
			continue;
		}
		for (const block of content) {
			const entry = block as Record<string, unknown>;
			if (entry["type"] === "text" && typeof entry["text"] === "string") {
				parts.push(entry["text"]);
			}
			if (entry["type"] === "tool_use") {
				const name = typeof entry["name"] === "string" ? entry["name"] : "";
				parts.push(
					name
						? `${name} ${JSON.stringify(entry["input"] ?? "")}`
						: JSON.stringify(entry["input"] ?? ""),
				);
			}
		}
	}
	return parts.join("\n");
}

async function main(): Promise<void> {
	const options = parseArgs(process.argv.slice(2));
	const scenarios = loadScenarios();
	const variants = loadVariants();

	if (options.list) {
		console.log("Scenarios:");
		for (const scenario of scenarios) {
			console.log(`  ${scenario.id.padEnd(28)} ${scenario.title}`);
		}
		console.log("\nSkill variants:");
		for (const variant of variants) {
			console.log(`  ${variant}`);
		}
		return;
	}

	const selectedScenarios = options.scenarios.length > 0
		? scenarios.filter((scenario) => options.scenarios.includes(scenario.id))
		: scenarios;
	const selectedVariants = options.variants.length > 0
		? options.variants
		: variants.length > 0
		? variants
		: ["baseline"];

	if (selectedScenarios.length === 0) {
		throw new Error("No scenarios selected.");
	}

	const pinaCli = resolvePinaCli();
	console.log(`pina CLI: ${pinaCli}`);
	const version = exec(`${JSON.stringify(pinaCli)} --version`, {
		cwd: HARNESS_ROOT,
		timeoutSeconds: 60,
	});
	console.log(`pina version: ${version.stdout.trim()}`);

	mkdirSync(RESULT_DIR, { recursive: true });
	const startedAt = new Date().toISOString();
	const runs: RunResult[] = [];

	for (const scenario of selectedScenarios) {
		for (const variant of selectedVariants) {
			for (let repeat = 1; repeat <= options.repeats; repeat += 1) {
				const source = scenario.skillSource ?? options.skillSource;
				const key = `${variant}${source === "cli" ? "-cli" : ""}`;
				const label = `${scenario.id} / ${key} / run ${repeat}`;
				console.log(`\n=== ${label} ===`);

				const workdir = runWorkdirFor(
					options.regrade,
					scenario.id,
					key,
					repeat,
				);
				try {
					if (!options.regrade) {
						copyFixture(scenario.fixture, workdir);

						for (
							const [path, contents] of Object.entries(
								scenario.setupFiles ?? {},
							)
						) {
							const target = join(workdir, path);
							ensureParent(target);
							writeFileSync(target, contents);
						}
						for (const command of scenario.setupCommands ?? []) {
							const result = exec(command, {
								cwd: workdir,
								timeoutSeconds: 300,
							});
							if (result.exit !== 0) {
								throw new Error(
									`setup command failed (${result.exit}): ${command}\n${result.stderr}`,
								);
							}
						}
					}

					const agentStarted = Date.now();
					const agent = options.regrade || scenario.agent === false
						? {
							transcript: options.regrade
								? readTranscript(scenario.id, key, repeat)
								: "",
							usage: undefined,
							timedOut: false,
							exit: 0,
						}
						: await runAgent({
							workdir,
							skillVariant: variant,
							skillSource: source,
							prompt: options.instructionVariant
								? (scenario.variants?.[options.instructionVariant] ??
									scenario.prompt)
								: scenario.prompt,
							model: options.model,
							timeoutSeconds: options.timeoutSeconds,
							allowedTools: DEFAULT_TOOLS,
							pinaCli,
							transcriptDir: join(
								RESULT_DIR,
								"transcripts",
								`${scenario.id}__${key}__${repeat}`,
							),
						});
					const agentDurationMs = Date.now() - agentStarted;

					const gradeStarted = Date.now();
					const checks = gradeAll(scenario.checks, {
						workdir,
						transcript: agent.transcript,
						pinaCli,
					});
					const gradeDurationMs = Date.now() - gradeStarted;

					const passed = allPassed(checks);
					for (const result of checks) {
						const mark = result.passed ? "PASS" : "FAIL";
						console.log(
							`  [${mark}] ${result.check.id}: ${result.detail.split("\n")[0]}`,
						);
						if (!result.passed) {
							console.log(`         why: ${result.check.why}`);
							const extra = result.detail.split("\n").slice(1).join("\n");
							if (extra.trim()) {
								console.log(
									extra
										.split("\n")
										.map((line) => `         ${line}`)
										.join("\n"),
								);
							}
						}
					}
					console.log(
						`  => ${passed ? "PASS" : "FAIL"} (${
							(agentDurationMs / 1000).toFixed(1)
						}s)`,
					);

					runs.push({
						scenario: scenario.id,
						title: scenario.title,
						variant,
						skillSource: source,
						model: options.model,
						instructionVariant: options.instructionVariant,
						repeat,
						passed,
						checks,
						agentDurationMs,
						gradeDurationMs,
						transcriptPath: join(
							"transcripts",
							`${scenario.id}__${variant}__${repeat}`,
							"transcript.jsonl",
						),
						usage: agent.usage,
						error: agent.timedOut
							? `agent timed out after ${options.timeoutSeconds}s`
							: agent.exit !== 0
							? `agent exited ${agent.exit}`
							: undefined,
					});
				} catch (error) {
					console.error(`  harness error: ${String(error)}`);
					runs.push({
						scenario: scenario.id,
						title: scenario.title,
						variant,
						skillSource: source,
						model: options.model,
						instructionVariant: options.instructionVariant,
						repeat,
						passed: false,
						checks: [],
						agentDurationMs: 0,
						gradeDurationMs: 0,
						transcriptPath: "",
						error: String(error),
					});
				}
			}
		}
	}

	const report: Report = {
		startedAt,
		finishedAt: new Date().toISOString(),
		model: options.model,
		repeats: options.repeats,
		runs,
	};

	const stamp = startedAt.replace(/[:.]/g, "-");
	const jsonPath = join(RESULT_DIR, `report-${stamp}.json`);
	writeFileSync(jsonPath, `${JSON.stringify(report, null, "\t")}\n`);
	writeFileSync(
		join(RESULT_DIR, "latest.json"),
		`${JSON.stringify(report, null, "\t")}\n`,
	);
	const markdown = renderMarkdown(report);
	writeFileSync(join(RESULT_DIR, `report-${stamp}.md`), markdown);
	writeFileSync(join(RESULT_DIR, "latest.md"), markdown);

	console.log(`\n${markdown}`);
	console.log(`\nReport: ${jsonPath}`);
}

function renderMarkdown(report: Report): string {
	const lines: string[] = ["# Pina skill evaluation", ""];
	lines.push(`- Model: \`${report.model}\``);
	lines.push(`- Started: ${report.startedAt}`);
	lines.push(`- Finished: ${report.finishedAt}`);
	lines.push(`- Repeats per cell: ${report.repeats}`);
	lines.push("");

	const variants = [...new Set(report.runs.map((run) => run.variant))];
	const scenarios = [...new Set(report.runs.map((run) => run.scenario))];

	lines.push("## Pass rate by scenario and skill variant");
	lines.push("");
	lines.push(`| Scenario | ${variants.join(" | ")} |`);
	lines.push(`| --- | ${variants.map(() => "---").join(" | ")} |`);
	for (const scenario of scenarios) {
		const cells = variants.map((variant) => {
			const cellRuns = report.runs.filter(
				(run) => run.scenario === scenario && run.variant === variant,
			);
			if (cellRuns.length === 0) {
				return "-";
			}
			const passed = cellRuns.filter((run) => run.passed).length;
			return `${passed}/${cellRuns.length}`;
		});
		lines.push(`| ${scenario} | ${cells.join(" | ")} |`);
	}
	lines.push("");

	lines.push("## Failing checks");
	lines.push("");
	const failures = report.runs.flatMap((run) =>
		run.checks
			.filter((check) => !check.passed)
			.map((check) => ({ run, check }))
	);
	if (failures.length === 0) {
		lines.push("None.");
	} else {
		lines.push("| Scenario | Variant | Check | Detail |");
		lines.push("| --- | --- | --- | --- |");
		for (const { run, check } of failures) {
			const detail = check.detail.split("\n")[0]?.replaceAll("|", "\\|") ?? "";
			lines.push(
				`| ${run.scenario} | ${run.variant} | ${check.check.id} | ${detail} |`,
			);
		}
	}
	lines.push("");

	const tokenTotal = report.runs.reduce(
		(sum, run) =>
			sum + (run.usage?.inputTokens ?? 0) + (run.usage?.outputTokens ?? 0),
		0,
	);
	const costTotal = report.runs.reduce(
		(sum, run) => sum + (run.usage?.costUsd ?? 0),
		0,
	);
	lines.push("## Cost");
	lines.push("");
	lines.push(`- Runs: ${report.runs.length}`);
	lines.push(`- Input+output tokens: ${tokenTotal}`);
	lines.push(`- Reported cost: $${costTotal.toFixed(2)}`);
	lines.push("");

	return lines.join("\n");
}

main().catch((error) => {
	console.error(error);
	process.exitCode = 1;
});
