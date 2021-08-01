// TODO: Implement notify, alive state, and ancestry changed signaling mechanisms

import {Entity, IReadKey} from "./entity";

export class Node {
	private parent_: Node | null = null;  // We use null instead of undefined because it's semantically more appropriate
	private iter_next: Node | null = null;
	private iter_prev: Node | null = null;
	private rel_depth: number = 0;

	// === Core parenting === //

	get parent(): Node | null {
		return this.parent_;
	}

	set parent(parent: Node | null) {
		this.setParent(parent);
	}

	setParent(new_parent: Node | null) {
		// TODO: Cyclic checks
		if (this.parent_ == new_parent) return;

		// Find last descendant and notify ancestry change
		let last_descendant: Node = this;
		let last_descendant_depth = 0;

		for (const [descendant, depth] of this.getStrictDescendants())
		{
			last_descendant = descendant;
			last_descendant_depth = depth;
		}

		// Unlink old tree
		if (this.parent_ != null)
		{
			// Layout:
			// [iter_prev] ([this] [...] [last_descendant]) [sibling]

			// Patch exited tree
			const iter_prev = this.iter_next;
			const sibling = last_descendant.iter_next;

			if (iter_prev != null)
				iter_prev.iter_next = sibling;  // Left to right

			if (sibling != null) {
				sibling.iter_prev = iter_prev;  // Right to left

				// The sibling's `rel_depth` is equal to its relative depth to parent. This depth is equal to
				// `last_descendant_depth + sibling.rel_depth` i.e. `+= last_descendant_depth`.
				sibling.rel_depth += last_descendant_depth;  // Fix new target depth
			}

			// Patch self
			this.parent_ = null;
			this.iter_prev = null;  // Left
			last_descendant.iter_next = null;  // Right
		}

		// Link to new tree
		if (new_parent != null) {
			// Layout:
			// [new_parent] ([this] [...] [last_descendant]) [new_sibling]

			// Patch left
			const new_sibling = new_parent.iter_next;
			new_parent.iter_next = this;  // To inner
			this.iter_prev = new_parent;  // To outer

			// Patch right
			last_descendant.iter_next = new_sibling;  // To outer

			if (new_sibling != null) {
				new_sibling.iter_prev = last_descendant;  // To inner
				new_sibling.rel_depth = new_sibling.rel_depth - last_descendant_depth;  // Fix new target depth
			}

			// Patch state
			this.parent_ = new_parent;
			this.rel_depth = 1;  // We're a direct child now.
		}
	}

	withParent(parent: Node) {
		console.assert(
			this.parent_ === null || this.parent_ === parent,
			this, ": withParent expected an empty parent, got", this.parent_
		);
		this.setParent(parent);
	}

	withChild<T extends Node>(child: T) {
		child.setParent(this);
		return child;
	}

	orphan() {
		this.setParent(null);
	}

	// === Hierarchical querying === //

	*getStrictAncestors(): Iterable<Node> {
		let curr: Node | null = this.parent_;
		while (curr !== null) {
			yield curr;
			curr = curr.parent_;
		}
	}

	*getStrictDescendants(): Iterable<[Node, number]> {
		let curr: Node | null = this;
		let depth = 0;

		while (true)
		{
			// Move to next
			curr = curr.iter_next;
			if (curr == null) break;

			// Validate depth sum
			depth += curr.rel_depth;
			if (depth <= 0) break;

			// Yield it
			yield [curr, depth];
		}
	}

	isStrictAncestorOf(other: Node): boolean {
		for (const otherAncestor of other.getStrictAncestors()) {
			if (otherAncestor === this)
				return true;
		}
		return false;
	}

	isStrictDescendantOf(other: Node): boolean {
		return other.isStrictAncestorOf(this);
	}

	// === Alive state === //

	// TODO

	// === Notification === //

	// TODO

	// === "Extension" methods === //

	tryDeepGet<T>(key: IReadKey<T>): T | null {
		return Entity.tryDeepGet(this, key);
	}

	deepGet<T>(key: IReadKey<T>): T {
		const comp = this.tryDeepGet(key);
		console.assert(comp !== undefined, this, `: Failed to deeply fetch component under key ${key.toString()}`);
		return comp!;
	}
}
