import { Buffer } from 'node:buffer';
import type { CompositionRegion, CompositionReport } from './types.js';

/** A composition candidate and its evidence. Inspect the report before use. */
export class PolyglotResult {
  readonly #data: Buffer;

  constructor(data: Buffer, public readonly report: CompositionReport) {
    this.#data = data;
  }

  get sizeBytes(): number { return this.#data.length; }

  /** An owned copy; modifying it does not change subsequent reads. */
  toBuffer(): Buffer { return Buffer.from(this.#data); }

  /** The byte range of one interpretation, if the strategy declared regions. */
  region(format: string): CompositionRegion | undefined {
    const region = this.report.regions.find((r) => r.format === format);
    return region && { ...region };
  }

  /**
   * The carried payload region, if any. For the `png`, `gif` and `jpeg`
   * carrier strategies this is the embedded payload, whatever format name it
   * was given — it is the region whose format is not the carrier image's.
   */
  payloadRegion(): CompositionRegion | undefined {
    const { regions, strategy } = this.report;
    const region = regions.find((r) => r.format !== strategy) ?? regions.at(-1);
    return region && { ...region };
  }

  /**
   * The payload's byte range as `"<length>@<offset>"`. Undefined when there is
   * no payload region (e.g. plain `append`). The string is a generic
   * length@offset pair the caller can use however it needs.
   */
  byteRange(): string | undefined {
    const region = this.payloadRegion();
    return region && `${region.length}@${region.offset}`;
  }
}
