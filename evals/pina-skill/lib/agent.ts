// Agent invocation for the evaluation harness.
//
// The runner drives a headless coding agent in a throwaway workdir and returns
// its transcript. Two properties matter more than anything else here:
//
// 1. The skill under test must be the only `pina` skill visible. A globally
//    installed skill of the same name would otherwise silently grade the wrong
//    content, because the agent would read it instead of the variant.
//
// 2. The run must be reproducible. `--setting-sources project` keeps the
//    operator's personal settings, hooks, and plugins out of the run, and
//    `--strict-mcp-config` keeps their MCP servers out.

import { spawn } from "node:child_process";
import {
	cpSync,
	existsSync,
	mkdirSync,
	rmSync,
	symlinkSync,
	writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import process from "node:process";

import { SKILL_VARIANT_DIR, WORK_DIR } from "./paths.ts";
import type { RunUsage } from "./types.ts";

export interface AgentRunOptions {
	/// Directory the agent works in. It is the project root the agent sees.
	workdir: string;
	/// Skill variant directory name under `skill-variants/`, or an absolute path.
	skillVariant: string;
	/// How the skill reaches the agent.
	///
	/// `installed` copies the variant into the workdir as a project skill, the
	/// normal path. `cli` installs nothing: the agent starts with only the
	/// toolkit and has to find the guidance itself, which is the path
	/// `pina skill read` and `pina skill install` exist to serve.
	skillSource: "installed" | "cli";
	/// The task handed to the agent.
	prompt: string;
	model: string;
	/// Wall-clock limit for the agent.
	timeoutSeconds: number;
	/// Tools the agent may use without prompting.
	allowedTools: string[];
	/// Extra environment variables for the agent process.
	env?: Record<string, string>;
	/// Directory to write `transcript.jsonl` into.
	transcriptDir: string;
	/// Optional system prompt appended to the runtime's default.
	appendSystemPrompt?: string;
	/// Absolute path to the `pina` CLI that must be first on `PATH`.
	///
	/// An operator's `PATH` commonly holds an older released `pina` whose
	/// subcommands differ (`make` instead of `create`). Grading a run that used
	/// the wrong CLI measures the environment, not the skill, so the workspace
	/// binary is always prepended.
	pinaCli: string;
}

export interface AgentRunResult {
	/// Flattened assistant text plus tool inputs, used for transcript grading.
	transcript: string;
	/// Final assistant message only.
	finalMessage: string;
	usage: RunUsage;
	exit: number;
	timedOut: boolean;
	stderr: string;
}

/// Prepare skill discovery for a run.
///
/// `installed` copies the variant into the workdir as a project skill —
/// `--setting-sources project` makes the runtime read skills from
/// `<workdir>/.claude/skills/`, so this copy is what the agent actually reads.
/// `cli` installs nothing on purpose: the run then measures whether the CLI
/// alone is enough for an agent to find and follow the guidance.
function prepareSkill(
	workdir: string,
	skillVariant: string,
	source: "installed" | "cli",
): void {
	if (source === "cli") {
		// The user-level skill must not leak in through some other discovery
		// path, so an explicit empty project skills directory is the contract.
		const skillsDir = join(workdir, ".claude", "skills");
		rmSync(skillsDir, { recursive: true, force: true });
		mkdirSync(skillsDir, { recursive: true });
		return;
	}

	const variantPath = skillVariant.startsWith("/")
		? skillVariant
		: join(SKILL_VARIANT_DIR, skillVariant);
	if (!existsSync(variantPath)) {
		throw new Error(`Skill variant not found: ${variantPath}`);
	}

	const skillsDir = join(workdir, ".claude", "skills");
	rmSync(skillsDir, { recursive: true, force: true });
	mkdirSync(skillsDir, { recursive: true });
	cpSync(variantPath, join(skillsDir, "pina"), { recursive: true });
}

/// Extract assistant-authored content from a `stream-json` transcript line.
///
/// Only records the assistant actually authored count. Skill bodies, tool
/// results, and file contents the agent merely *read* arrive as other record
/// types; including them would let a check match the skill's own prose instead
/// of the agent's work.
function extractFromLine(line: string): { text: string; toolInput: string } {
	let parsed: unknown;
	try {
		parsed = JSON.parse(line);
	} catch {
		return { text: "", toolInput: "" };
	}

	const record = parsed as Record<string, unknown>;
	if (record["type"] !== "assistant") {
		return { text: "", toolInput: "" };
	}

	const message = record["message"] as Record<string, unknown> | undefined;
	const content = message?.["content"];
	if (!Array.isArray(content)) {
		return { text: "", toolInput: "" };
	}

	const texts: string[] = [];
	const toolInputs: string[] = [];
	for (const block of content) {
		const entry = block as Record<string, unknown>;
		if (entry["type"] === "text" && typeof entry["text"] === "string") {
			texts.push(entry["text"]);
		}
		if (entry["type"] === "tool_use") {
			const input = entry["input"];
			const serialized = typeof input === "string"
				? input
				: JSON.stringify(input ?? "");
			// The tool name is part of what the assistant authored: a check
			// that hunts for an Edit of a generated file needs the name to
			// match on.
			const name = typeof entry["name"] === "string" ? entry["name"] : "";
			toolInputs.push(name ? `${name} ${serialized}` : serialized);
		}
	}

	return { text: texts.join("\n"), toolInput: toolInputs.join("\n") };
}

/// Run one headless agent session and return its transcript.
export async function runAgent(
	options: AgentRunOptions,
): Promise<AgentRunResult> {
	prepareSkill(options.workdir, options.skillVariant, options.skillSource);

	// A private HOME keeps the runtime's own caches out of the operator's
	// profile. Credentials still come from the real HOME via the CLI's own
	// keychain lookup, so HOME is deliberately left untouched.
	mkdirSync(options.transcriptDir, { recursive: true });

	const args = [
		"-p",
		options.prompt,
		"--model",
		options.model,
		"--output-format",
		"stream-json",
		"--verbose",
		"--permission-mode",
		"bypassPermissions",
		"--setting-sources",
		"project",
		"--strict-mcp-config",
		"--allowedTools",
		...options.allowedTools,
	];
	if (options.appendSystemPrompt) {
		args.push("--append-system-prompt", options.appendSystemPrompt);
	}

	const binDir = dirname(options.pinaCli);
	const child = spawn("claude", args, {
		cwd: options.workdir,
		env: {
			...process.env,
			...options.env,
			PATH: `${binDir}:${process.env["PATH"] ?? ""}`,
		},
		stdio: ["ignore", "pipe", "pipe"],
	});

	let stdout = "";
	let stderr = "";
	let timedOut = false;

	const timer = setTimeout(() => {
		timedOut = true;
		child.kill("SIGKILL");
	}, options.timeoutSeconds * 1000);

	child.stdout.on("data", (chunk: Buffer) => {
		stdout += chunk.toString("utf8");
	});
	child.stderr.on("data", (chunk: Buffer) => {
		stderr += chunk.toString("utf8");
	});

	const exit = await new Promise<number>((resolveExit) => {
		child.on("close", (code) => resolveExit(code ?? 1));
		child.on("error", () => resolveExit(1));
	});
	clearTimeout(timer);

	writeFileSync(join(options.transcriptDir, "transcript.jsonl"), stdout);
	writeFileSync(join(options.transcriptDir, "stderr.txt"), stderr);

	// A runtime that refuses to start (no credentials, or bypassed permissions
	// under root) produces no transcript at all. Grading that as an ordinary
	// failed run would report the skill as broken when no agent ever ran.
	if (!timedOut && exit !== 0 && stdout.trim() === "") {
		throw new Error(
			`the agent runtime exited with status ${exit} before producing a transcript: ${stderr.trim()}`,
		);
	}

	const transcriptParts: string[] = [];
	const finalParts: string[] = [];
	const usage: RunUsage = {
		inputTokens: 0,
		outputTokens: 0,
		cacheReadTokens: 0,
		cacheCreationTokens: 0,
		costUsd: 0,
		turns: 0,
	};

	for (const line of stdout.split("\n")) {
		if (line.trim() === "") {
			continue;
		}
		const { text, toolInput } = extractFromLine(line);
		if (text) {
			transcriptParts.push(text);
			finalParts.push(text);
		}
		if (toolInput) {
			transcriptParts.push(toolInput);
		}

		// The runtime reports cumulative usage per assistant turn and a final
		// total in the result record. Take the maximum so a summary line never
		// under-counts.
		try {
			const parsed = JSON.parse(line) as Record<string, unknown>;
			if (parsed["type"] === "result") {
				const reported = parsed["usage"] as Record<string, number> | undefined;
				if (reported) {
					usage.inputTokens = Math.max(
						usage.inputTokens,
						reported["input_tokens"] ?? 0,
					);
					usage.outputTokens = Math.max(
						usage.outputTokens,
						reported["output_tokens"] ?? 0,
					);
					usage.cacheReadTokens = Math.max(
						usage.cacheReadTokens,
						reported["cache_read_input_tokens"] ?? 0,
					);
					usage.cacheCreationTokens = Math.max(
						usage.cacheCreationTokens,
						reported["cache_creation_input_tokens"] ?? 0,
					);
				}
				if (typeof parsed["total_cost_usd"] === "number") {
					usage.costUsd = parsed["total_cost_usd"];
				}
				if (typeof parsed["num_turns"] === "number") {
					usage.turns = parsed["num_turns"];
				}
			}
			if (parsed["type"] === "assistant") {
				usage.turns += 1;
			}
		} catch {
			// Non-JSON lines are already handled by the extractor above.
		}
	}

	return {
		transcript: transcriptParts.join("\n"),
		finalMessage: finalParts.join("\n"),
		usage,
		exit,
		timedOut,
		stderr,
	};
}

/// Create a per-run scratch directory.
export function runWorkdir(
	scenarioId: string,
	variant: string,
	repeat: number,
): string {
	const path = join(WORK_DIR, "runs", `${scenarioId}__${variant}__${repeat}`);
	rmSync(path, { recursive: true, force: true });
	mkdirSync(path, { recursive: true });
	return path;
}
