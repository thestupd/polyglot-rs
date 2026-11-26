import { Buffer } from 'node:buffer';
import { PolyglotError, fromNativeError } from './errors.js';
import { native } from './native.js';
import { PolyglotResult } from './result.js';
import type { CompatibilityOptions, CompatibilityResult, CompositionReport, CreateOptions, StrategyRequirements } from './types.js';
import type { BinaryInput, CustomFormat, FormatInfo, InspectionOptions, InspectionResult, ValidateOptions, ValidationResult } from './types.js';

export * from './types.js';
export { PolyglotResult } from './result.js';
export { PolyglotError } from './errors.js';
export type { PolyglotErrorCode } from './errors.js';

const DEFAULT_LIMIT = 64 * 1024 * 1024;
const NAME = /^[a-z0-9-]+$/;

function formatName(value: unknown): asserts value is string {
  if (typeof value !== 'string' || !NAME.test(value)) {
    throw new PolyglotError('ERR_INVALID_INPUT', 'Format names must contain lowercase ASCII letters, digits or hyphens');
  }
}

function strategyName(value: unknown): string {
  if (value === undefined) return 'auto';
  formatName(value);
  return value;
}

function limits(options: InspectionOptions): [number, number] {
  if (!options || typeof options !== 'object' || Array.isArray(options)) {
    throw new PolyglotError('ERR_INVALID_INPUT', 'Options must be an object');
  }
  const values = [options.maxInputBytes ?? DEFAULT_LIMIT, options.maxDecodedBytes ?? DEFAULT_LIMIT] as [number, number];
  if (values.some((value) => !Number.isInteger(value) || value <= 0 || value > 0xffff_ffff)) {
    throw new PolyglotError('ERR_INVALID_INPUT', 'Limits must be positive integers at most 2^32 - 1');
  }
  return values;
}

function bytes(data: BinaryInput, maxInputBytes: number): Buffer {
  if (!(data instanceof Uint8Array) || data.buffer instanceof SharedArrayBuffer) {
    throw new PolyglotError('ERR_INVALID_INPUT', 'Expected a Buffer or Uint8Array backed by an unshared ArrayBuffer');
  }
  if (data.byteLength > maxInputBytes) throw new PolyglotError('ERR_LIMIT_EXCEEDED', 'Input exceeds maxInputBytes');
  // View only the requested subarray. Rust makes the single owned snapshot.
  return Buffer.from(data.buffer, data.byteOffset, data.byteLength);
}

/** An isolated registry. Use separate instances to avoid global detection hooks. */
export class PolyglotEngine {
  private readonly hooks = new Map<string, CustomFormat['detect']>();

  registerFormat(format: CustomFormat): void {
    if (!format || typeof format !== 'object') throw new PolyglotError('ERR_INVALID_INPUT', 'Expected a custom format object');
    formatName(format.name);
    if (typeof format.detect !== 'function') throw new PolyglotError('ERR_INVALID_INPUT', 'detect must be a function');
    if (this.hooks.has(format.name) || native().nativeFormats().includes(format.name)) {
      throw new PolyglotError('ERR_DUPLICATE_FORMAT', `Format ${format.name} is already registered`);
    }
    this.hooks.set(format.name, format.detect);
  }

  unregisterFormat(name: string): boolean { return this.hooks.delete(name); }

  listStrategies(): Record<string, StrategyRequirements> {
    try {
      return Object.fromEntries(JSON.parse(native().strategiesNative()) as [string, StrategyRequirements][]);
    } catch (error) { throw fromNativeError(error); }
  }

  async checkCompatibility(options: CompatibilityOptions): Promise<CompatibilityResult> {
    if (!options || typeof options !== 'object' || !Array.isArray(options.formats) || options.formats.length < 2 || options.formats.length > 256) {
      throw new PolyglotError('ERR_INVALID_INPUT', 'Compatibility requires between 2 and 256 format names');
    }
    const formats = [...options.formats];
    formats.forEach(formatName);
    const strategy = strategyName(options.strategy);
    try {
      return JSON.parse(await native().checkCompatibilityNative(formats, strategy)) as CompatibilityResult;
    } catch (error) { throw fromNativeError(error); }
  }

  async create(options: CreateOptions): Promise<PolyglotResult> {
    const [maxInputBytes, maxDecodedBytes] = limits(options);
    if (!Array.isArray(options.inputs) || options.inputs.length < 2 || options.inputs.length > 256) {
      throw new PolyglotError('ERR_INVALID_INPUT', 'Composition requires between 2 and 256 inputs');
    }
    if (options.validate !== undefined && typeof options.validate !== 'boolean') {
      throw new PolyglotError('ERR_INVALID_INPUT', 'validate must be a boolean');
    }
    const strategy = strategyName(options.strategy);
    const maxOutputBytes = options.maxOutputBytes ?? DEFAULT_LIMIT;
    if (!Number.isInteger(maxOutputBytes) || maxOutputBytes <= 0 || maxOutputBytes > 0xffff_ffff) {
      throw new PolyglotError('ERR_INVALID_INPUT', 'maxOutputBytes must be a positive integer at most 2^32 - 1');
    }
    let total = 0;
    const inputs = options.inputs.map((input) => {
      if (!input || typeof input !== 'object') throw new PolyglotError('ERR_INVALID_INPUT', 'Each input must be an object');
      formatName(input.format);
      if (input.label !== undefined && typeof input.label !== 'string') throw new PolyglotError('ERR_INVALID_INPUT', 'Input labels must be strings');
      const data = bytes(input.data, maxInputBytes);
      total += data.length;
      if (total > maxInputBytes) throw new PolyglotError('ERR_LIMIT_EXCEEDED', 'Total input size exceeds maxInputBytes');
      return input.label === undefined ? { format: input.format, data } : { format: input.format, data, label: input.label };
    });
    try {
      const result = await native().createNative(inputs, strategy, options.validate ?? true, maxInputBytes, maxDecodedBytes, maxOutputBytes);
      return new PolyglotResult(result.data, JSON.parse(result.reportJson) as CompositionReport);
    } catch (error) { throw fromNativeError(error); }
  }

  async inspect(data: BinaryInput, options: InspectionOptions = {}): Promise<InspectionResult> {
    const [maxInputBytes, maxDecodedBytes] = limits(options);
    const input = bytes(data, maxInputBytes);
    const hooks = [...this.hooks.entries()];
    // Take a private hook snapshot before yielding. Each hook gets its own copy
    // so a mutating hook cannot affect another hook or the native result.
    const hookInput = hooks.length ? Buffer.from(input) : undefined;
    try {
      const result = JSON.parse(await native().inspectNative(input, maxInputBytes, maxDecodedBytes)) as InspectionResult;
      for (const info of Object.values(result.metadata)) info.source = 'native';
      for (const [name, detect] of hooks) {
        let matched: unknown;
        try { matched = await detect(Buffer.from(hookInput!)); }
        catch (cause) { throw new PolyglotError('ERR_CUSTOM_DETECTION', `Detection hook ${name} failed`, { cause }); }
        if (typeof matched !== 'boolean') throw new PolyglotError('ERR_CUSTOM_DETECTION', `Detection hook ${name} must return a boolean`);
        if (matched) {
          const info: FormatInfo = {
            format: name, source: 'javascript', metadata: {},
            validation: { valid: false, format: name, errors: ['JavaScript detection hook has no native validator'], warnings: ['Detection is not validation or composition support'] },
          };
          Object.defineProperty(result.metadata, name, { value: info, enumerable: true, configurable: true, writable: true });
          result.formats.push(name);
        }
      }
      result.formats.sort();
      return result;
    } catch (error) { throw fromNativeError(error); }
  }

  async validate(data: BinaryInput, options: ValidateOptions): Promise<ValidationResult[]> {
    const [maxInputBytes, maxDecodedBytes] = limits(options);
    const input = bytes(data, maxInputBytes);
    if (!Array.isArray(options.formats) || options.formats.length === 0 || options.formats.length > 256) {
      throw new PolyglotError('ERR_INVALID_INPUT', 'formats must contain between 1 and 256 format names');
    }
    const formats = [...options.formats];
    formats.forEach(formatName);
    try {
      return JSON.parse(await native().validateNative(input, formats, maxInputBytes, maxDecodedBytes)) as ValidationResult[];
    } catch (error) { throw fromNativeError(error); }
  }
}

/** Shared convenience registry. Native format handlers always run in Rust. */
export const Polyglot = new PolyglotEngine();
