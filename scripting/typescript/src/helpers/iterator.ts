/// A type that is *guaranteed* to be non-iterable.
export type NonIterable<T> = (T & { [Symbol.iterator]?: undefined });

/// A type that is either iterable or a non-iterable "unit" value.
export type IterableOrUnit<T> = NonIterable<T> | Iterable<T>;

export function unitToIter<T>(from: T): Iterable<T> {
    return [from];
}

export function iterOrUnitToIter<T>(from: IterableOrUnit<T>): Iterable<T> {
    return from[Symbol.iterator] !== undefined
        // Type safety: we already checked that `target[Symbol.iterator]` is not `undefined`. Through elimination, this
        // means that the right side of the union is the value's actual type.
        ?  from as Iterable<T>

        // Type safety: we already checked that `target[Symbol.iterator]` is `undefined`, meaning that the left side of
        // the union is the value's actual type.
        : unitToIter(from as NonIterable<T>);
}
