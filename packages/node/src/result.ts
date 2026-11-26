import { Buffer } from 'node:buffer';
import type { CompositionReport } from './types.js';

/** A composition candidate and its evidence. Inspect the report before use. */
export class PolyglotResult {
  readonly #data: Buffer;

  constructor(data: Buffer, public readonly report: CompositionReport) {
    this.#data = data;
  }

  get sizeBytes(): number { return this.#data.length; }

  /** An owned copy; modifying it does not change subsequent reads. */
  toBuffer(): Buffer { return Buffer.from(this.#data); }
}
