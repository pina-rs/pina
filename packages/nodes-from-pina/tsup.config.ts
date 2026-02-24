import { defineConfig } from "tsup";

export default defineConfig({
	clean: false,
	dts: false,
	entry: ["src/index.ts"],
	format: ["cjs", "esm"],
	sourcemap: true,
	target: "es2022",
});
