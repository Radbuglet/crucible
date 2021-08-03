import {Node} from "./node";
import {IterableOrUnit, iterOrUnitToIter} from "../helpers/iterator";

export interface IRawKey {
    getRaw(target: Entity): unknown;
}

export interface IWriteKey<T> extends IRawKey {  // `T` is contravariant
    set(target: Entity, value: T): void;
    remove(target: Entity): void;
}

export interface IReadKey<T> extends IRawKey {  // `T` is covariant
    get(target: Entity): T | undefined;
}

export class Key<T> implements IReadKey<T>, IWriteKey<T> {  // `T` is invariant
    private readonly symbol: symbol;

    constructor(debug_name?: string) {
        this.symbol = Symbol(debug_name);
    }

    set(target: Entity, value: T) {
        // Type safety: symbol is only accessed through this safe interface.
        (target as any)[this.symbol] = value;
    }

    remove(target: Entity) {
        // Type safety: `target` can be accessed like an object
        delete (target as any)[this.symbol];
    }

    getRaw(target: Entity): unknown {
        // Type safety: `target` can be accessed like an object
        return (target as any)[this.symbol];
    }

    get(target: Entity): T | undefined {
        // Type safety: symbol is only accessed through this safe interface.
        return (target as any)[this.symbol];
    }

    toString(): string {
        return `Key(${this.symbol.toString()})`;
    }
}

export class Entity extends Node {
    // === Core accessors === //

    add<T>(keys: IterableOrUnit<IWriteKey<T>>, comp: T) {
        for (const key of iterOrUnitToIter(keys)) {
            console.assert(
                !this.has(key),
                this, `: attempted to add() to the already-mapped key ${key.toString()}.`
            );
            key.set(this, comp);
        }
    }

    set<T>(keys: IterableOrUnit<IWriteKey<T>>, comp: T) {
        for (const key of iterOrUnitToIter(keys)) {
            key.set(this, comp);
        }
    }

    remove(keys: IterableOrUnit<IWriteKey<unknown>>) {
        for (const key of iterOrUnitToIter(keys)) {
            key.remove(this);
        }
    }

    tryGet<T>(key: IReadKey<T>): T | undefined {
        return key.get(this);
    }

    get<T>(key: IReadKey<T>): T {
        const comp = this.tryGet(key);
        console.assert(comp !== undefined, this, `: Failed to fetch component under key ${key.toString()}`);
        return comp!;
    }

    has(key: IRawKey): boolean {
        return key.getRaw(this) !== undefined;
    }

    // === Tree accessors === //

    addNode<T extends Node>(keys: IterableOrUnit<IWriteKey<T>>, comp: T) {
        comp.withParent(this);
        this.add(keys, comp);
    }

    setNode<T extends Node>(keys: IterableOrUnit<IWriteKey<T>>, comp: T) {
        comp.withParent(this);
        this.set(keys, comp);
    }

    removeNode(keys: IterableOrUnit<IWriteKey<unknown>>) {
        for (const key of iterOrUnitToIter(keys)) {
            const comp = key.getRaw(this);
            if (comp instanceof Node) {
                comp.orphan();
            } else {
                console.warn(this, ": removeNode removed a component mapping to the non-node value", comp);
            }
            key.remove(this);
        }
    }

    // === Deep querying === //

    static tryDeepGet<T>(node: Node, key: IReadKey<T>): T | null {
        for (const ancestor of node.getStrictAncestors()) {
            if (!(ancestor instanceof Entity)) continue;

            const comp = ancestor.tryGet(key);
            if (comp !== undefined)
                return comp;
        }
        return null;
    }
}
