// refactor: 5:44-5:45, 14:41-14:42
// @effect-v3
import * as Effect from "effect/Effect"

const asyncFunctionDeclaration = async function() {
  const response = await fetch("test")
  if (response.ok) {
    const y = await response.json()
    return y
  }
  return null
}

const asyncArrowFunctionExpression = async () => {
  const response = await fetch("test")
  if (response.ok) {
    const y = await response.json()
    return y
  }
  return null
}
