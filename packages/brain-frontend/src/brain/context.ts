import type { BrainContextMetadata, GenerationTerminal } from './generation.js';

type ActiveContext = Readonly<{
  sessionId: string;
  generationId: string;
  context?: BrainContextMetadata;
}>;

/** Bounded, in-memory context summary projection; never a transcript store. */
export class BrainContextState {
  private readonly active = new Map<string, ActiveContext>();
  private readonly latestCompleted = new Map<string, BrainContextMetadata>();

  public begin(sessionId: string, generationId: string): void {
    const key = generationKey(sessionId, generationId);
    if (this.active.has(key)) throw new Error('context_generation_already_active');
    this.active.set(key, Object.freeze({ sessionId, generationId }));
  }

  public record(sessionId: string, generationId: string, context: BrainContextMetadata): void {
    const key = generationKey(sessionId, generationId);
    const active = this.active.get(key);
    if (active === undefined) throw new Error('context_generation_not_active');
    this.active.set(key, Object.freeze({ ...active, context: cloneContext(context) }));
  }

  public settle(sessionId: string, generationId: string, terminal: GenerationTerminal): void {
    const key = generationKey(sessionId, generationId);
    const active = this.active.get(key);
    if (active === undefined) return;
    this.active.delete(key);
    if (terminal !== 'completed' || active.context === undefined) return;
    if (active.context.retrievalCount === 0) {
      this.latestCompleted.delete(sessionId);
      return;
    }
    this.latestCompleted.set(sessionId, active.context);
  }

  public latestForSession(sessionId: string): BrainContextMetadata | undefined {
    return this.latestCompleted.get(sessionId);
  }

  public clearSession(sessionId: string): void {
    this.latestCompleted.delete(sessionId);
    for (const [key, active] of this.active) {
      if (active.sessionId === sessionId) this.active.delete(key);
    }
  }

  public counts(): Readonly<{ active: number; latestCompleted: number }> {
    return Object.freeze({ active: this.active.size, latestCompleted: this.latestCompleted.size });
  }
}

function generationKey(sessionId: string, generationId: string): string {
  return `${sessionId}\u0000${generationId}`;
}

function cloneContext(context: BrainContextMetadata): BrainContextMetadata {
  return Object.freeze({
    ...context,
    retrievalChannels: Object.freeze([...context.retrievalChannels]),
    sourceRefs: Object.freeze([...context.sourceRefs]),
  });
}
