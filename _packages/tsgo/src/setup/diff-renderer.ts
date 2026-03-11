import * as Console from "effect/Console"
import * as Effect from "effect/Effect"
import * as Option from "effect/Option"
import type * as ts from "typescript"
import type { ComputeChangesResult } from "./changes.js"
import type { Assessment } from "./types.js"

// ANSI color helpers using raw escape codes
const green = (str: string): string => `\x1b[32m${str}\x1b[0m`
const red = (str: string): string => `\x1b[31m${str}\x1b[0m`
const gray = (str: string): string => `\x1b[90m${str}\x1b[0m`
const cyan = (str: string): string => `\x1b[36m${str}\x1b[0m`
const bold = (str: string): string => `\x1b[1m${str}\x1b[0m`
const yellow = (str: string): string => `\x1b[33m${str}\x1b[0m`

/**
 * Get lines from source file text
 */
function getLines(text: string): ReadonlyArray<string> {
  return text.split("\n")
}

/**
 * Render a single line with consistent formatting
 * @param lineNum - Line number (1-based) or undefined for lines without numbers (changes)
 * @param symbol - Symbol to display: "|" for unchanged, "-" for deletion, "+" for addition
 * @param text - The actual line text
 * @param colorFn - The color function to apply
 */
function renderLine(
  lineNum: number | undefined,
  symbol: "|" | "-" | "+",
  text: string,
  colorFn: (s: string) => string
): string {
  const lineNumPart = lineNum !== undefined
    ? String(lineNum).padStart(4, " ")
    : "    "

  return colorFn(`${lineNumPart} ${symbol} ${text}`)
}

/**
 * Render a single text change with context (1 line before and after)
 */
export function renderTextChange(
  sourceFile: ts.SourceFile,
  textChange: ts.TextChange
): ReadonlyArray<string> {
  const startPos = textChange.span.start
  const endPos = textChange.span.start + textChange.span.length

  const startLineAndChar = sourceFile.getLineAndCharacterOfPosition(startPos)
  const endLineAndChar = sourceFile.getLineAndCharacterOfPosition(endPos)

  const startLine = startLineAndChar.line
  const endLine = endLineAndChar.line
  const startCol = startLineAndChar.character
  const endCol = endLineAndChar.character

  const lines: Array<string> = []
  const allLines = getLines(sourceFile.text)

  // Show 1 line before the change (if exists)
  if (startLine > 0) {
    const contextBefore = allLines[startLine - 1]
    lines.push(renderLine(startLine, "|", contextBefore, gray))
  }

  // ============================================================================
  // Render deleted text
  // ============================================================================

  // Handle the first line of deletion (might be partial)
  if (startLine <= endLine) {
    const firstLineText = allLines[startLine]
    const keptBeforeDeletion = firstLineText.slice(0, startCol)

    // Only show the kept part if it contains non-whitespace characters
    const hasNonWhitespaceKept = keptBeforeDeletion.trim().length > 0
    if (hasNonWhitespaceKept) {
      lines.push(renderLine(startLine + 1, "|", keptBeforeDeletion, gray))
    }

    // Show the deleted part of the first line
    const deletedOnFirstLine = startLine === endLine
      ? firstLineText.slice(startCol, endCol)
      : firstLineText.slice(startCol)

    if (deletedOnFirstLine.length > 0) {
      const spacePadding = hasNonWhitespaceKept ? " ".repeat(keptBeforeDeletion.length) : ""
      lines.push(renderLine(undefined, "-", `${spacePadding}${deletedOnFirstLine}`, red))
    }
  }

  // Show fully deleted lines (middle lines between start and end)
  for (let i = startLine + 1; i < endLine; i++) {
    const lineText = allLines[i]
    if (lineText !== undefined) {
      lines.push(renderLine(undefined, "-", lineText, red))
    }
  }

  // Handle the last line of deletion (might be partial, and different from first line)
  if (endLine > startLine) {
    const lastLineText = allLines[endLine]
    const deletedOnLastLine = lastLineText.slice(0, endCol)

    if (deletedOnLastLine.length > 0) {
      lines.push(renderLine(undefined, "-", deletedOnLastLine, red))
    }
  }

  // ============================================================================
  // Render added text
  // ============================================================================

  if (textChange.newText.length > 0) {
    const newTextLines = textChange.newText.split("\n")

    const firstLineText = allLines[startLine]
    const keptBeforeDeletion = firstLineText.slice(0, startCol)
    const hasNonWhitespaceKept = keptBeforeDeletion.trim().length > 0
    const spacePadding = hasNonWhitespaceKept ? " ".repeat(keptBeforeDeletion.length) : ""

    for (let i = 0; i < newTextLines.length; i++) {
      const newLine = newTextLines[i]

      // Skip empty last line from split (trailing newline case)
      if (i === newTextLines.length - 1 && newLine.length === 0 && newTextLines.length > 1) {
        continue
      }

      const padding = (i === 0 && hasNonWhitespaceKept) ? spacePadding : ""
      lines.push(renderLine(undefined, "+", `${padding}${newLine}`, green))
    }
  }

  // ============================================================================
  // Render kept part after deletion
  // ============================================================================

  let alignmentForKeptPart = 0

  if (textChange.newText.length > 0) {
    const newTextLines = textChange.newText.split("\n")
    const lastNewLine = newTextLines[newTextLines.length - 1]

    if (lastNewLine.length === 0 && newTextLines.length > 1) {
      alignmentForKeptPart = 0
    } else {
      const firstLineText = allLines[startLine]
      const keptBeforeDeletion = firstLineText.slice(0, startCol)
      const hasNonWhitespaceKept = keptBeforeDeletion.trim().length > 0

      if (hasNonWhitespaceKept) {
        if (newTextLines.length === 1) {
          alignmentForKeptPart = keptBeforeDeletion.length + lastNewLine.length
        } else {
          alignmentForKeptPart = lastNewLine.length
        }
      } else {
        alignmentForKeptPart = lastNewLine.length
      }
    }
  } else {
    alignmentForKeptPart = endCol
  }

  if (endLine > startLine) {
    const lastLineText = allLines[endLine]
    const keptAfterDeletion = lastLineText.slice(endCol)
    if (keptAfterDeletion.trim().length > 0) {
      const alignment = " ".repeat(alignmentForKeptPart)
      lines.push(renderLine(endLine + 1, "|", `${alignment}${keptAfterDeletion}`, gray))
    }
  } else if (startLine === endLine) {
    const firstLineText = allLines[startLine]
    const keptAfterDeletion = firstLineText.slice(endCol)
    if (keptAfterDeletion.trim().length > 0) {
      const alignment = " ".repeat(alignmentForKeptPart)
      lines.push(renderLine(startLine + 1, "|", `${alignment}${keptAfterDeletion}`, gray))
    }
  }

  // Show 1 line after the change (if exists)
  if (endLine + 1 < allLines.length) {
    const contextAfter = allLines[endLine + 1]
    lines.push(renderLine(endLine + 2, "|", contextAfter, gray))
  }

  return lines
}

/**
 * Render all text changes for a file
 */
export function renderFileChanges(
  sourceFile: ts.SourceFile,
  textChanges: ReadonlyArray<ts.TextChange>
): ReadonlyArray<string> {
  const lines: Array<string> = []

  // Sort changes by position
  const sortedChanges = [...textChanges].sort((a, b) => a.span.start - b.span.start)

  for (let i = 0; i < sortedChanges.length; i++) {
    const change = sortedChanges[i]
    const changeLines = renderTextChange(sourceFile, change)

    for (const line of changeLines) {
      lines.push(line)
    }

    // Add separator between changes if there are multiple
    if (i < sortedChanges.length - 1) {
      lines.push("")
    }
  }

  return lines
}

/**
 * Render code actions with colored diffs using raw ANSI escape codes
 */
export const renderCodeActions = (
  result: ComputeChangesResult,
  assessmentState: Assessment.State
): Effect.Effect<void> =>
  Effect.gen(function*() {
    // Check if there are no changes
    if (result.codeActions.length === 0) {
      yield* Console.log(green("No changes needed - your configuration is already up to date!"))
      return
    }

    // Collect all source files from assessment state
    const sourceFiles: Array<ts.SourceFile> = [
      assessmentState.packageJson.sourceFile,
      assessmentState.tsconfig.sourceFile
    ]
    if (Option.isSome(assessmentState.vscodeSettings)) {
      sourceFiles.push(assessmentState.vscodeSettings.value.sourceFile)
    }

    // Render each code action with diffs
    for (const codeAction of result.codeActions) {
      for (const fileChange of codeAction.changes) {
        // Find the source file that matches the file name
        const sourceFile = sourceFiles.find((sf) => sf.fileName === fileChange.fileName)

        // Render description and file name
        yield* Console.log("")
        yield* Console.log(bold(codeAction.description))
        yield* Console.log(cyan(fileChange.fileName))
        yield* Console.log("")

        if (sourceFile) {
          const diffLines = renderFileChanges(sourceFile, fileChange.textChanges)
          for (const line of diffLines) {
            yield* Console.log(line)
          }
        } else {
          yield* Console.log("  (file will be modified)")
        }
      }
    }

    // Display user messages
    if (result.messages.length > 0) {
      yield* Console.log("")
      for (const message of result.messages) {
        if (message.includes("WARNING")) {
          yield* Console.log(yellow(message))
        } else {
          yield* Console.log(message)
        }
      }
    }
  })
