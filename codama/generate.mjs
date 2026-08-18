import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const workspaceRoot = fileURLToPath(new URL("..", import.meta.url));

// Keep this historical entry point, but route every generation workflow
// through Pina's single renderer pipeline. Besides preventing drift between
// scripts, the CLI adds the zeropod validation boundary to stock Codama's
// generated JavaScript codecs.
execFileSync(
	"cargo",
	["run", "-p", "pina_cli", "--", "codama", "generate", "--npx", "node"],
	{
		cwd: workspaceRoot,
		stdio: "inherit",
	},
);
