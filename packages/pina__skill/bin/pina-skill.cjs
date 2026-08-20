#!/usr/bin/env node
"use strict";

const { cpSync, existsSync, mkdirSync } = require("node:fs");
const { homedir } = require("node:os");
const path = require("node:path");

const packageRoot = path.resolve(__dirname, "..");
const skillEntries = Object.freeze(["SKILL.md", "agents", "references"]);

function defaultDestination(environment = process.env) {
	const codexHome = environment.CODEX_HOME || path.join(homedir(), ".codex");
	return path.join(codexHome, "skills", "pina");
}

function installSkill(destination) {
	const resolvedDestination = path.resolve(destination);
	if (existsSync(resolvedDestination)) {
		throw new Error(
			`Refusing to replace existing skill directory: ${resolvedDestination}. ` +
				"Remove or move it after reviewing your local changes, then run the installer again.",
		);
	}

	mkdirSync(resolvedDestination, { recursive: true });
	for (const entry of skillEntries) {
		cpSync(
			path.join(packageRoot, entry),
			path.join(resolvedDestination, entry),
			{
				recursive: true,
			},
		);
	}

	return resolvedDestination;
}

function helpText() {
	return `Install or locate the Pina agent skill.

Usage:
  pina-skill --install [DIR]
  pina-skill --print-path
  pina-skill --print-install
  pina-skill --help

Options:
  --install [DIR]  Copy the skill to DIR. Defaults to the Codex skill directory.
  --print-path     Print the packaged skill source directory.
  --print-install  Print manual installation commands.
  -h, --help       Show this help.

The installer never overwrites an existing skill directory.`;
}

function main(arguments_ = process.argv.slice(2)) {
	const command = arguments_[0];
	if (command === "--print-path") {
		console.log(packageRoot);
		return 0;
	}

	if (command === "--print-install") {
		const destination = defaultDestination();
		console.log(`Packaged skill: ${packageRoot}`);
		console.log(`Default destination: ${destination}`);
		console.log("Install with: pina-skill --install");
		return 0;
	}

	if (command === "--install") {
		const requestedDestination = arguments_[1];
		if (
			arguments_.length > 2 ||
			(requestedDestination !== undefined &&
				requestedDestination.startsWith("--"))
		) {
			console.error("`--install` accepts at most one destination directory.");
			return 2;
		}

		const destination = requestedDestination ?? defaultDestination();
		console.log(`Installed Pina skill at ${installSkill(destination)}`);
		return 0;
	}

	if (command === undefined || command === "--help" || command === "-h") {
		console.log(helpText());
		return 0;
	}

	console.error(`Unknown argument: ${command}`);
	console.error("Run `pina-skill --help` for usage.");
	return 2;
}

module.exports = {
	defaultDestination,
	helpText,
	installSkill,
	main,
	packageRoot,
};

if (require.main === module) {
	try {
		process.exitCode = main();
	} catch (error) {
		console.error(error instanceof Error ? error.message : String(error));
		process.exitCode = 1;
	}
}
