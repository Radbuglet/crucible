import {NonNever} from "../helpers/typing";

export type ToListenerFn<F> = F extends (...args: infer A) => void ? (...args: A) => void : never;  // TODO: Preserve generics
export type ToListener<I> = NonNever<{
    [K in keyof I]: ToListenerFn<I[K]>
}>;

export const MultiDispatch = new class {
    genProxy<I>(handlers: Iterable<I>): ToListener<I> {
        const proxy = new Proxy<any>({}, {
            // FIXME: Prove validity using the type system.
            get: (target: any, p: PropertyKey): any => {
                let fn = target[p];
                if (fn === undefined) {
                    fn = (...args: unknown[]) => {
                        for (const handler of handlers) {
                            (handler as any)[p](...args);
                        }
                    };
                    target[p] = fn;
                }
                return fn;
            },

            // TODO: Implement these once we can query the type of `I` at runtime. We should also prevent some unintuitive runtime operations.
            has(): boolean {
                throw "Cannot check for property presence in a MultiDispatch proxy!";
            },
            ownKeys(): PropertyKey[] {
                throw "Cannot iterate through the keys of a MultiDispatch proxy!";
            }
        });

        // Type safety: `ToListener<I>` is programmatically implemented by the proxy.
        return proxy as any;
    }
}();

export class HandlerList<I> implements Iterable<I> {
    private readonly targets = new Set<I>();
    private proxy_cache: ToListener<I> | null = null;

    register(handler: I) {
        this.targets.add(handler);
    }

    unregister(handler: I) {
        this.targets.delete(handler);
    }

    get proxy(): ToListener<I> {
        if (this.proxy_cache === null) {
            this.proxy_cache = MultiDispatch.genProxy(this);
        }
        return this.proxy_cache;
    }

    [Symbol.iterator](): Iterator<I> {
        return this.targets[Symbol.iterator]();
    }
}
