/**
 * Synchronous child-process transport adapted from TypeScript-Go's SyncRpcChannel.
 *
 * Copyright (c) Microsoft Corporation. Licensed under the Apache License 2.0.
 * Source: typescript-go/_packages/native-preview/src/api/syncChannel.ts
 *
 * The process, pipe, buffering, retry, and cleanup code intentionally tracks the
 * upstream implementation. The MessagePack RPC layer is replaced with the
 * Effect Oxlint protocol's Content-Length JSON framing.
 */

import { type ChildProcess, spawn } from "node:child_process"
import { closeSync, openSync, readSync, writeSync } from "node:fs"
import type { Readable, Writable } from "node:stream"

interface StdioHandle {
  fd: number
  setBlocking?: (value: boolean) => void
}

interface StdoutWithHandle extends Readable {
  _handle: StdioHandle
  unref: () => void
}

interface StdinWithHandle extends Writable {
  _handle: StdioHandle
  unref: () => void
}

const sleepBuf = new Int32Array(new SharedArrayBuffer(4))
const liveChildren = new Set<ChildProcess>()

process.on("exit", () => {
  for (const child of liveChildren) {
    try {
      child.kill()
    } catch {
      // The process may already have exited.
    }
  }
  liveChildren.clear()
})

/** This class is single-threaded and must not be shared across worker threads. */
export class SyncChannel {
  private child: ChildProcess
  private readFd: number
  private writeFd: number
  private pipeFd: number | undefined
  private readBuf = Buffer.allocUnsafe(65536)
  private readBufPos = 0
  private readBufLen = 0

  constructor(executable: string, args: Array<string>) {
    if (process.platform === "win32") {
      const pipePath = `\\\\.\\pipe\\effect-tsgo-oxlint-sync-${process.pid}-${Date.now()}`
      this.child = spawn(executable, [...args, "--pipe", pipePath], {
        stdio: ["ignore", "ignore", "inherit"],
      })

      let fd: number | undefined
      for (let i = 0; i < 500; i++) {
        try {
          fd = openSync(pipePath, "r+")
          break
        } catch {
          if (this.child.exitCode !== null) {
            throw new Error(`Child process exited with code ${this.child.exitCode} before pipe was ready`)
          }
          Atomics.wait(sleepBuf, 0, 0, 10)
        }
      }
      if (fd === undefined) {
        this.child.kill()
        throw new Error("SyncChannel: timed out connecting to named pipe")
      }
      this.readFd = fd
      this.writeFd = fd
      this.pipeFd = fd
    } else {
      this.child = spawn(executable, args, {
        stdio: ["pipe", "pipe", "inherit"],
      })

      const stdout = this.child.stdout as StdoutWithHandle
      const stdin = this.child.stdin as StdinWithHandle
      this.readFd = stdout._handle.fd
      this.writeFd = stdin._handle.fd

      if (typeof this.readFd !== "number" || this.readFd < 0 || typeof this.writeFd !== "number" || this.writeFd < 0) {
        stdout.destroy()
        stdin.destroy()
        this.child.kill()
        throw new Error("SyncChannel: could not obtain pipe file descriptors")
      }

      stdout._handle.setBlocking?.(true)
      stdin._handle.setBlocking?.(true)
      stdout.pause()
      stdout.unref()
      stdin.unref()
    }

    liveChildren.add(this.child)
    this.child.unref()
  }

  request(payload: string): string {
    this.ensureOpen()
    const length = Buffer.byteLength(payload, "utf8")
    this.writeAllBuf(Buffer.from(`Content-Length: ${length}\r\n\r\n${payload}`))
    return this.readFrame().toString("utf8")
  }

  close(): void {
    try {
      liveChildren.delete(this.child)
      if (this.pipeFd !== undefined) {
        closeSync(this.pipeFd)
        this.pipeFd = undefined
      }
      this.child.stdout?.destroy()
      this.child.stdin?.destroy()
      this.child.kill()
      this.readFd = -1
      this.writeFd = -1
    } catch {
      // The process may already have exited.
    }
  }

  private ensureOpen(): void {
    if (this.readFd < 0) throw new Error("SyncChannel is closed")
  }

  private readFrame(): Buffer {
    let header = ""
    while (!header.endsWith("\r\n\r\n")) {
      header += String.fromCharCode(this.readByte())
    }
    const match = /(?:^|\r\n)Content-Length:\s*(\d+)\r\n/i.exec(header)
    if (!match) throw new Error("Effect Oxlint response is missing Content-Length")
    return this.readExact(Number(match[1]))
  }

  private eofError(): Error {
    const code = this.child.exitCode
    const signal = this.child.signalCode
    const detail = signal ? `killed by signal ${signal}` : code !== null ? `exited with code ${code}` : "unknown reason"
    return new Error(`Unexpected EOF while reading from child process (${detail})`)
  }

  private readByte(): number {
    if (this.readBufPos >= this.readBufLen) this.fillReadBuffer()
    return this.readBuf[this.readBufPos++]
  }

  private readExact(length: number): Buffer {
    const buffer = Buffer.allocUnsafeSlow(length)
    this.readExactInto(buffer, length)
    return buffer
  }

  private fillReadBuffer(): void {
    this.readBufPos = 0
    this.readBufLen = 0
    for (;;) {
      try {
        const read = readSync(this.readFd, this.readBuf, 0, this.readBuf.length, null)
        if (read === 0) throw this.eofError()
        this.readBufLen = read
        return
      } catch (error: unknown) {
        if (!isRetryable(error)) throw error
        Atomics.wait(sleepBuf, 0, 0, 1)
      }
    }
  }

  private readExactInto(buffer: Buffer, length: number): void {
    let position = 0
    while (position < length) {
      const available = this.readBufLen - this.readBufPos
      if (available > 0) {
        const toCopy = Math.min(available, length - position)
        this.readBuf.copy(buffer, position, this.readBufPos, this.readBufPos + toCopy)
        this.readBufPos += toCopy
        position += toCopy
      } else if (length - position >= this.readBuf.length) {
        try {
          const read = readSync(this.readFd, buffer, position, length - position, null)
          if (read === 0) throw this.eofError()
          position += read
        } catch (error: unknown) {
          if (!isRetryable(error)) throw error
          Atomics.wait(sleepBuf, 0, 0, 1)
        }
      } else {
        this.fillReadBuffer()
      }
    }
  }

  private writeAllBuf(data: Buffer | Uint8Array, length?: number): void {
    const total = length ?? data.length
    let position = 0
    while (position < total) {
      try {
        position += writeSync(this.writeFd, data, position, total - position)
      } catch (error: unknown) {
        if (!isRetryable(error)) throw error
        Atomics.wait(sleepBuf, 0, 0, 1)
      }
    }
  }
}

function isRetryable(error: unknown): boolean {
  return error instanceof Error && "code" in error && (error.code === "EAGAIN" || error.code === "EWOULDBLOCK")
}
