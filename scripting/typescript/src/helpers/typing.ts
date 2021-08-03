/// Produces a dummy type that is invariant over `T`.
export type Invariant<T> = (_: T) => T;

/// Produces `true` if the type of `A` is fully equal to the type of `B` and `false` otherwise.
export type TypesEqual<A, B> = Invariant<A> extends Invariant<B> ? true : false;

/// A ternary that picks the `truthy` value IFF `Cond` is a literal `true` and the `falsy` value otherwise.
export type Ternary<Cond, Truthy, Falsy> = Cond extends true ? Truthy : Falsy;

/// Returns the union of all keys in `T` that are non-never, or `never` otherwise.
export type NonNeverKeys<T> = { [K in keyof T]: Ternary<TypesEqual<T[K], never>, never, K> }[keyof T];

/// Returns an object where `never` properties (i.e. properties that can never be accessed) are removed.
export type NonNever<T> = Pick<T, NonNeverKeys<T>>;
