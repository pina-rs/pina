import type { Node, Visitor } from "codama";
import {
	assertIsNode,
	rootNodeVisitor,
	setFixedAccountSizesVisitor,
	visit,
} from "codama";

export function defaultVisitor(): Visitor<Node | null, "rootNode"> {
	return rootNodeVisitor((currentRoot) => {
		const nextRoot = visit(currentRoot, setFixedAccountSizesVisitor());
		assertIsNode(nextRoot, "rootNode");
		return nextRoot;
	});
}
