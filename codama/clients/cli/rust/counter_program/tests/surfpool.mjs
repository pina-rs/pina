// Boots an offline surfnet, deploys the counter program at its declared ID,
// funds the CLI's payer, and prints the RPC URL on stdout once that RPC
// answers, for the Rust e2e test to consume.
import { Surfnet } from "@solana/surfpool";

const [soPath, programId, payer] = process.argv.slice(2);

// Clock block production confirms what the CLI submits; Surfpool 1.6's
// default transaction-triggered mode never confirms on an offline instance
// (https://github.com/solana-foundation/surfpool/issues/814).
const surfnet = Surfnet.startWithConfig({
	offline: true,
	blockProductionMode: "clock",
});

// The native instance shuts down when V8 collects this handle. Nothing after
// startup reads it, so without an owner a garbage collection could stop the
// RPC between READY and the CLI's first request. The SIGTERM handler owns the
// handle for the life of the process and stops the instance cleanly.
process.on("SIGTERM", () => {
	surfnet.stop();
	process.exit(0);
});

surfnet.deploy({ programId, soPath });
// The CLI signs with its own keypair, which an offline surfnet has never
// seen, so fund it before the test runs any command.
surfnet.fundSol(payer, 10_000_000_000);

// READY promises an RPC that answers, so ask it over HTTP the way the CLI
// will reach it. Startup already waited for the RPC, so one bounded request
// either confirms it or fails the bootstrap loudly.
const response = await fetch(surfnet.rpcUrl, {
	method: "POST",
	headers: { "content-type": "application/json" },
	body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "getHealth" }),
	signal: AbortSignal.timeout(10_000),
});
const health = await response.json();
if (health.result !== "ok") {
	throw new Error(`surfnet RPC is not healthy: ${JSON.stringify(health)}`);
}

console.log(`READY ${surfnet.rpcUrl}`);
setInterval(() => {}, 60_000);
