import type { Node, RootNode } from "codama";
import { assertIsNode, createFromRoot, visit } from "codama";

import { defaultVisitor } from "./defaultVisitor";

export { defaultVisitor };

export function rootNodeFromPina(idl: unknown): RootNode {
	const root = visit(
		rootNodeFromPinaWithoutDefaultVisitor(idl),
		defaultVisitor(),
	);
	assertIsNode(root, "rootNode");
	return root;
}

export function rootNodeFromPinaWithoutDefaultVisitor(idl: unknown): RootNode {
	if (typeof idl === "string") {
		let parsedIdl: unknown;
		try {
			parsedIdl = JSON.parse(idl) as unknown;
		} catch (error) {
			throw new Error(
				`Invalid Pina IDL JSON: ${(error as Error).message}`,
			);
		}
		return rootNodeFromPinaWithoutDefaultVisitor(parsedIdl);
	}

	if (typeof idl !== "object" || idl === null) {
		throw new Error("Expected a Pina IDL object or JSON string.");
	}

	assertIsNode(idl as Node, "rootNode");
	return createFromRoot(idl as RootNode).getRoot();
}
