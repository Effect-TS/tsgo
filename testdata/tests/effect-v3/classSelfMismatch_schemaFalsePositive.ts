// @effect-v3
const Persistable = {
  Class: <A>() =>
    (
      _name: string,
      _config: {
        primaryKey: (_req: A) => string
      }
    ) =>
      class {}
}

export class TTLRequest extends Persistable.Class<{
  payload: { id: number }
}>()("TTLRequest", {
  primaryKey: (req) => `TTLRequest:${req.payload.id}`
}) {}
