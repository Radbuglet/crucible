type IterableUnit<T> = (T & { [Symbol.iterator]?: undefined });
export type IterableOrUnit<T> = IterableUnit<T> | Iterable<T>;

export function unitToIter<T>(value: T): Iterable<T> {
    return [value];
}

export function iterOrUnitToIter<T>(value: IterableOrUnit<T>): Iterable<T> {
    return value[Symbol.iterator] !== undefined
        // Type safety: we already checked that `target[Symbol.iterator]` is not `undefined`. Through elimination, this
        // means that the right side of the union is the value's actual type.
        ?  value as Iterable<T>

        // Type safety: we already checked that `target[Symbol.iterator]` is `undefined`, meaning that the left side of
        // the union is the value's actual type.
        : unitToIter(value as IterableUnit<T>);
}
