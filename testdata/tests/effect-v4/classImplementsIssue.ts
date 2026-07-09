import {PubSub } from "effect"

export class ReplayWindowImpl<A> implements PubSub.PubSub.ReplayWindow<A> {
    index: number = 1
    remaining: number = 1

    fastForward() {
    }
    take(): A | undefined {
      return 1 as A
    }
    takeN(n: number): Array<A> {
      return 1 as any
    }
    takeAll(): Array<A> {
      return 1 as any
    }
  }