// TODO: TypeScript utils, implement
// TODO: Should we integrate with the alive system?

export const MultiDispatch = new class {
    genProxy<I>(targets: Iterable<I>): I {
        throw "Not implemented";
    }

    *dispatchIter<I>(targets: Iterable<I>) {
        throw "Not implemented";
    }

    dispatch<I>(targets: Iterable<I>) {
        throw "Not implemented";
    }
}();

export class HandlerList<I> {
    register() {
        throw "Not implemented";
    }

    unregister() {
        throw "Not implemented";
    }

    getProxy() {
        throw "Not implemented";
    }

    *dispatchIter() {
        throw "Not implemented";
    }

    dispatch() {
        throw "Not implemented";
    }
}
