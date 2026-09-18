// Boots an offline surfnet, deploys the counter program at its declared ID,
// and prints the RPC URL on stdout for the Rust e2e test to consume.
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "../../../../..");
const require2 = createRequire(resolve(repo, "package.json"));
const { Surfnet } = await import(
	resolve(repo, "node_modules/@solana/surfpool/dist/index.mjs")
).catch(
	() => import("@solana/surfpool"),
);

const soPath = process.argv[2];
const programId = process.argv[3];
const surfnet = Surfnet.start({ offline: true });
await surfnet.deploy({ programId, soPath });
console.log(`READY ${surfnet.rpcUrl ?? "http://127.0.0.1:8899"}`);
process.on("SIGTERM", () => process.exit(0));
setInterval(() => {}, 60_000);
