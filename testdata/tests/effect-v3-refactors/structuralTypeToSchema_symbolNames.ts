// refactor: 15:18-15:26
// @effect-v3
import * as Schema from "effect/Schema"

type User = {
  id: number
  name: string
}

interface Todo {
  id: number
  description: string
}

export interface AppState {
  users: User
  tasks: Array<[Todo, User]>
}
