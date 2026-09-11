// @filename: tsconfig.json
{
  "compilerOptions": {
    "types": ["node"],
    "plugins": [
      {
        "name": "@effect/language-service",
        "ignoreEffectErrorsInTscExitCode": true,
        "skipDisabledOptimization": true
      }
    ]
  }
}

// @filename: nodeBuiltinImport.ts
// @effect-v3
// @effect-diagnostics nodeBuiltinImport:warning
import * as Effect from "effect/Effect"

// Flagged: ES module imports for covered modules
import fs from "fs"
import * as fs2 from "node:fs"
import { readFile } from "fs/promises"
import { readFile as readFile2 } from "node:fs/promises"
import { join } from "path"
import path from "node:path"
import { join as join2 } from "path/posix"
import { join as join3 } from "node:path/posix"
import { join as join4 } from "path/win32"
import { join as join5 } from "node:path/win32"
import { exec } from "child_process"
import { spawn } from "node:child_process"
import http from "http"
import https from "node:https"
import { Console } from "console"
import { Console as NodeConsole } from "node:console"
import { setTimeout } from "timers"
import { setTimeout as nodeSetTimeout } from "node:timers"
import { setTimeout as delay } from "timers/promises"
import { setTimeout as nodeDelay } from "node:timers/promises"
import { Readable } from "stream"
import { Readable as NodeReadable } from "node:stream"
import { pipeline } from "stream/promises"
import { pipeline as nodePipeline } from "node:stream/promises"
import { ReadableStream } from "stream/web"
import { ReadableStream as NodeReadableStream } from "node:stream/web"

// Flagged: side-effect import
import "node:fs"

// Flagged: top-level require
const fs3 = require("node:fs")
const consoleModule = require("console")
const timersModule = require("node:timers/promises")
const streamModule = require("stream/web")

// Not flagged: crypto has no secure Effect v3 equivalent
import * as crypto from "crypto"
import { randomUUID } from "node:crypto"
const cryptoModule = require("node:crypto")

// Not flagged: only explicitly covered subpaths match
import { text } from "node:stream/consumers"
const unknownTimersSubpath = require("timers/promises/extra")

// Not flagged: Effect-native imports
// @ts-expect-error - @effect/platform not installed in harness
import { FileSystem } from "@effect/platform"

// Not flagged: third-party modules with similar names
// @ts-expect-error - fs-extra not installed in harness
import fsExtra from "fs-extra"
