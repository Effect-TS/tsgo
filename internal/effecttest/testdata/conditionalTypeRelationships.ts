// @filename: conditionalTypeRelationships.ts
// Reduced from microsoft/TypeScript PR64307's conditionalTypes1.ts and inferTypes1.ts.
// The identical callback signatures are the minimal TS2719 regression from issue 64673.
declare let callback: <A>(item: A, receive: (value: A extends string ? A : 0) => void) => void;
export const sameCallback: <A>(item: A, receive: (value: A extends string ? A : 0) => void) => void = callback;

export function conditionalRelations<T>(
  x: T extends { value: infer U } ? { t: T; u: U } : never,
  y: T extends { value: infer U } ? { t: T; u: U } : never,
) {
  x = y;
  y = x;
}

// Identity instantiation must normalize distributed references inside objects,
// function parameters and nested conditionals, not just the outer check type.
declare function inferObject<X, T>(value: X extends unknown ? { value: T } : never): T;
export function inferObjectRelation<U>(value: U extends unknown ? { value: U } : never, ordinary: U) {
  const inferred = inferObject(value);
  const accept: typeof inferred = ordinary;
  return accept;
}

declare function inferCallback<X, T>(value: X extends unknown ? (arg: T) => T : never): T;
export function inferCallbackRelation<U>(value: U extends unknown ? (arg: U) => U : never, ordinary: U) {
  const inferred = inferCallback(value);
  const accept: typeof inferred = ordinary;
  return accept;
}

export function nestedConditionalRelation<T>(
  x: T extends object ? { inner: [T] extends [string] ? { t: T } : never } : never,
  y: [T] extends [string] ? { t: T } : never,
) {
  x.inner = y;
  y = x.inner;
}

export function falseBranchRelation<T>(x: T extends { value: infer U } ? never : { t: T }) {
  const make = <V>(v: V): T extends { value: infer U } ? never : { t: T; v: V } => {
    throw new Error("type-only fixture");
  };
  x = make(1);
}

export function extendsRelation<T>(x: T extends { self: T } ? { t: T } : never) {
  const make = <V>(v: V): T extends { self: T } ? { t: T; v: V } : never => {
    throw new Error("type-only fixture");
  };
  x = make(1);
}

import { PgInsert, type ReturningQuery } from "./returning.js";
export const selected: ReturningQuery<{ id: number }> = new PgInsert().returning({ id: 1 });
export const selectedRows: { id: number }[] = selected.execute();
export const unselected: { rowCount: number } = new PgInsert().execute();

// @filename: returning.d.ts
// Self-contained Drizzle-style declarations, not a substitute for Drizzle's suite.
// Keep these in a .d.ts unit so declaration checking covers extends and implements.
export interface SQLWrapper {
  getSQL(): string;
}

export interface ReturningQuery<TSelection> {
  execute(): TSelection extends undefined ? { rowCount: number } : TSelection[];
  returning<T extends Record<string, unknown> = Record<string, unknown>>(fields?: T): ReturningQuery<T>;
}

export declare abstract class QueryBase<TSelection> implements ReturningQuery<TSelection> {
  abstract execute(): TSelection extends undefined ? { rowCount: number } : TSelection[];
  abstract returning<T extends Record<string, unknown> = Record<string, unknown>>(fields?: T): ReturningQuery<T>;
}

export declare class PgInsert<TSelection = undefined> extends QueryBase<TSelection> implements ReturningQuery<TSelection>, SQLWrapper {
  execute(): TSelection extends undefined ? { rowCount: number } : TSelection[];
  returning<T extends Record<string, unknown> = Record<string, unknown>>(fields?: T): PgInsert<T>;
  getSQL(): string;
}

// Expected TS2430 on BrokenReturning. This also catches accidental skipLibCheck.
export interface BrokenReturning extends ReturningQuery<{ id: number }> {
  execute(): string;
}

// @filename: invalid.ts
// Expected TS2322 on badAssignment. The patch must not accept invalid assignments.
export const badAssignment: number = "not a number";

// Expected TS2322 on badNarrowed. Normalization must preserve distributivity.
export function rejectNonDistributed<T>(ordinary: T) {
  const badNarrowed: T extends string ? T : never = ordinary;
  return badNarrowed;
}

// @filename: effectControl.ts
// @effect-diagnostics *:off
// @effect-diagnostics effectInFailure:CONTROL
import { Effect } from "effect";

// Expected TS377054 on the whole Effect.try call only when the rule is enabled.
export const effectControl = Effect.try({
  try: () => 1,
  catch: () => Effect.logError("boom"),
});
