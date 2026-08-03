import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import { appendFile } from "node:fs/promises"
import { runCommand, runCommandString } from "./process.ts"

export interface OpenPullRequestOptions {
  readonly base: string
  readonly head?: string
  readonly headPrefix?: string
  readonly title: string
  readonly body: string
  readonly commitMessage: string
  readonly checks: Readonly<Record<string, string>>
}

export interface CompleteCheckOptions {
  readonly checkId: string
  readonly result: "success" | "failure" | "cancelled"
  readonly successMessage: string
  readonly failureMessage: string
  readonly summary: string
}

export class GitHubEnvironmentError extends Data.TaggedError("GitHubEnvironmentError")<{
  readonly variable: string
}> {
  get message(): string {
    return `Missing required GitHub Actions environment variable ${this.variable}`
  }
}

export class CheckOutputError extends Data.TaggedError("CheckOutputError")<{
  readonly output: string
}> {
  get message(): string {
    return `Invalid GitHub Actions output name ${JSON.stringify(this.output)}`
  }
}

export class PullRequestOptionsError extends Data.TaggedError("PullRequestOptionsError")<{
  readonly reason: string
}> {
  get message(): string {
    return `Invalid pull request options: ${this.reason}`
  }
}

const commandString = (command: string, cwd: string, args: ReadonlyArray<string>) =>
  runCommandString(command, cwd, args).pipe(Effect.map((output) => output.trim()))

const requireEnvironment = (variable: string) => {
  const value = process.env[variable]
  return value === undefined || value === ""
    ? Effect.fail(new GitHubEnvironmentError({ variable }))
    : Effect.succeed(value)
}

export const openPullRequestIfChanged = Effect.fnUntraced(function*(
  repositoryRoot: string,
  options: OpenPullRequestOptions
) {
  const repository = yield* requireEnvironment("GITHUB_REPOSITORY")
  if ((options.head === undefined) === (options.headPrefix === undefined)) {
    return yield* new PullRequestOptionsError({ reason: "Specify exactly one of --head or --head-prefix" })
  }
  const branchSelector = options.head === undefined
    ? `startswith(${JSON.stringify(`${options.headPrefix}/`)})`
    : `. == ${JSON.stringify(options.head)}`
  const outputFile = Object.keys(options.checks).length === 0
    ? undefined
    : yield* requireEnvironment("GITHUB_OUTPUT")

  for (const output of Object.keys(options.checks)) {
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(output)) {
      return yield* new CheckOutputError({ output })
    }
  }

  yield* runCommand("git", repositoryRoot, ["fetch", "--depth", "1", "origin", options.base])
  const baseSha = yield* commandString("git", repositoryRoot, ["rev-parse", "FETCH_HEAD"])
  const baseTree = yield* commandString("git", repositoryRoot, ["rev-parse", `${baseSha}^{tree}`])
  yield* runCommand("git", repositoryRoot, ["add", "-A"])
  const generatedTree = yield* commandString("git", repositoryRoot, ["write-tree"])

  const existing = yield* commandString("gh", repositoryRoot, [
    "pr",
    "list",
    "--base",
    options.base,
    "--state",
    "open",
    "--json",
    "number,headRefName",
    "--jq",
    `map(select(.headRefName | ${branchSelector}))[0] | if . == null then "" else "\\(.number)\\t\\(.headRefName)" end`
  ])
  const [existingNumber = "", existingBranch = ""] = existing.split("\t")

  if (generatedTree === baseTree) {
    yield* Effect.log(`${options.base} already matches the generated tree.`)
    if (existingNumber !== "") {
      yield* runCommand("gh", repositoryRoot, [
        "pr",
        "close",
        existingNumber,
        "--delete-branch",
        "--comment",
        `Closing because ${options.base} already matches the generated tree.`
      ])
    }
    return
  }

  let branch: string
  let parentSha: string
  if (existingBranch !== "") {
    yield* runCommand("git", repositoryRoot, ["fetch", "--depth", "1", "origin", existingBranch])
    branch = existingBranch
    parentSha = yield* commandString("git", repositoryRoot, ["rev-parse", "FETCH_HEAD"])
  } else {
    branch = options.head ?? `${options.headPrefix}/${yield* requireEnvironment("GITHUB_RUN_ID")}`
    parentSha = baseSha
  }

  const parentTree = yield* commandString("git", repositoryRoot, ["rev-parse", `${parentSha}^{tree}`])
  let headSha = parentSha
  if (generatedTree !== parentTree) {
    headSha = yield* commandString("git", repositoryRoot, [
      "-c",
      "user.name=github-actions[bot]",
      "-c",
      "user.email=41898282+github-actions[bot]@users.noreply.github.com",
      "commit-tree",
      generatedTree,
      "-p",
      parentSha,
      "-m",
      options.commitMessage
    ])
    const remoteBranch = yield* commandString("git", repositoryRoot, [
      "ls-remote",
      "--heads",
      "origin",
      `refs/heads/${branch}`
    ])
    const remoteHeadSha = remoteBranch.split(/\s/, 1)[0]
    yield* runCommand("git", repositoryRoot, [
      "push",
      `--force-with-lease=refs/heads/${branch}:${remoteHeadSha}`,
      "origin",
      `${headSha}:refs/heads/${branch}`
    ])
  }

  if (existingNumber !== "") {
    yield* runCommand("gh", repositoryRoot, [
      "pr",
      "edit",
      existingNumber,
      "--title",
      options.title,
      "--body",
      options.body
    ])
  } else {
    yield* runCommand("gh", repositoryRoot, [
      "pr",
      "create",
      "--base",
      options.base,
      "--head",
      branch,
      "--title",
      options.title,
      "--body",
      options.body
    ])
  }

  for (const [output, check] of Object.entries(options.checks)) {
    const checkId = yield* commandString("gh", repositoryRoot, [
      "api",
      "--method",
      "POST",
      `repos/${repository}/check-runs`,
      "-f",
      `name=${check}`,
      "-f",
      `head_sha=${headSha}`,
      "-f",
      "status=in_progress",
      "--jq",
      ".id"
    ])
    yield* Effect.tryPromise(() => appendFile(outputFile!, `${output}=${checkId}\n`))
  }
})

export const completeCheck = Effect.fnUntraced(function*(
  repositoryRoot: string,
  options: CompleteCheckOptions
) {
  if (options.checkId === "") {
    return
  }
  const repository = yield* requireEnvironment("GITHUB_REPOSITORY")
  yield* runCommand("gh", repositoryRoot, [
    "api",
    "--method",
    "PATCH",
    `repos/${repository}/check-runs/${options.checkId}`,
    "-f",
    "status=completed",
    "-f",
    `conclusion=${options.result}`,
    "-f",
    `output[title]=${options.result === "success" ? options.successMessage : options.failureMessage}`,
    "-f",
    `output[summary]=${options.summary}`
  ])
})
