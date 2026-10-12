import * as Effect from "effect/Effect";
import * as Option from "effect/Option";

declare function decode<A>(): (input: string) => Effect.Effect<A>;

const readFile = Effect.fail<{ readonly _tag: "NotFound" }>({ _tag: "NotFound" });

export const readJson = <A>() =>
  Effect.gen(function* () {
    return yield* readFile.pipe(
      Effect.catchTags({ NotFound: () => Effect.succeed("{}") }),
      Effect.flatMap(decode<A>()),
    );
  });


declare class Tool {
  constructor(properties: { readonly annotations: { readonly title?: string } });
}

export const makeTool = () =>
  Effect.gen(function* () {
    return new Tool({
      annotations: {
        ...Option.some("test").pipe(
          Option.map((title) => ({ title })),
          Option.getOrUndefined,
        ),
      },
    });
  });
