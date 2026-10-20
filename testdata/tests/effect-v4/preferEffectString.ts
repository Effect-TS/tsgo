// @effect-diagnostics *:off
// @effect-diagnostics preferEffectString:warning
import * as Str from "effect/String"

declare const value: string
value.at(0)
value.charAt(0)
value.charCodeAt(0)
value.codePointAt(0)
value.concat("!")
value.endsWith("o")
value.includes("e")
value.indexOf("l")
value.lastIndexOf("l")
value.localeCompare("world")
value.match(/l/)
value.matchAll(/l/g)
value.normalize()
value.padEnd(8)
value.padStart(8)
value.repeat(2)
value.replace("l", "r")
value.replaceAll("l", "r")
value.search(/l/)
value.slice(1)
value.split("")
value.startsWith("h")
value.substring(1)
value.toLocaleLowerCase()
value.toLocaleUpperCase()
value.toLowerCase()
value.toUpperCase()
value.trim()
value.trimStart()
value.trimEnd()
value.trimLeft()
value.trimRight()

value["trim"]()
value[`trim`]()
;(value.trim)()
value.trim?.()
declare const optional: string | undefined
optional?.trim()
optional?.["trim"]()
declare const literal: "hello"
literal.trim()
declare const template: `hello ${string}`
template.trim()
declare const branded: string & { _brand: "X" }
branded.trim()
declare const boxed: String
boxed.trim()
function constrained<T extends string>(generic: T) { return generic.trim() }

declare const mixed: string | Array<string>
mixed.includes("h")
mixed.slice(1)
declare const anyValue: any
anyValue.trim()
const uncalled = value.trim
;[value].map(value.trim)
String.prototype.trim.call(value)
value.substr(1)
value.toString()
value.anchor("id")
value.isWellFormed()
value.toWellFormed()
value.valueOf()
value[Symbol.iterator]()
value.length
String.fromCharCode(65)
String.fromCodePoint(65)
String.raw`hello`
declare const key: "trim"
value[key]()
declare const custom: { trim(): string }
custom.trim()
class Custom extends String { trim() { return "" } }
new Custom().trim()
class Inherited extends String { test() { return super.trim() } }
declare let writable: Partial<Pick<String, "trim">>
writable.trim = () => ""
delete (writable.trim)
const { trim } = value
Str.trim(value)
