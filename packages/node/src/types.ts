/** Bytes are snapshotted once before native work is scheduled. */
export type BinaryInput = Buffer | Uint8Array;

export interface PolyglotInput {
  format: string;
  data: BinaryInput;
  label?: string;
}

export interface InspectionOptions {
  /** Default 64 MiB. Positive integer, at most 2^32 - 1. */
  maxInputBytes?: number;
  /** Default 64 MiB. Caps total image bytes decoded per handler. */
  maxDecodedBytes?: number;
}

export interface ValidateOptions extends InspectionOptions {
  /** Each interpretation is checked independently; at least one is required. */
  formats: string[];
}

export interface ValidationResult {
  valid: boolean;
  format: string;
  errors: string[];
  warnings: string[];
}

export type MetadataValue = string | number | boolean | null | MetadataValue[] | { [key: string]: MetadataValue };

export interface FormatInfo {
  format: string;
  source: 'native' | 'javascript';
  metadata: Record<string, MetadataValue>;
  /** JavaScript detection hooks have no validator and return valid: false. */
  validation: ValidationResult;
}

export interface InspectionResult {
  /** Candidate matches, including recognized but malformed structures. */
  formats: string[];
  metadata: Record<string, FormatInfo>;
  sizeBytes: number;
}

/** Detection only. Runs in JavaScript and cannot enable Rust composition. */
export interface CustomFormat {
  name: string;
  detect: (buffer: Buffer) => boolean | Promise<boolean>;
}

export type CompatibilityStatus = 'supported' | 'conditional' | 'unsupported';

export interface CompatibilityResult {
  status: CompatibilityStatus;
  strategies: string[];
  limitations: string[];
  explanation: string | null;
}

export interface CompatibilityOptions {
  formats: string[];
  strategy?: string | 'auto';
}

export interface CreateOptions extends InspectionOptions {
  inputs: PolyglotInput[];
  strategy?: string | 'auto';
  /** Defaults to true; false produces an explicitly unvalidated candidate. */
  validate?: boolean;
  /** Default 64 MiB. Input budget applies to the sum of all input buffers. */
  maxOutputBytes?: number;
}

export interface CompositionInputReport {
  format: string;
  label: string | null;
  sizeBytes: number;
  validation: ValidationResult | null;
}

export interface CompositionReport {
  strategy: string;
  compatibility: CompatibilityResult;
  inputs: CompositionInputReport[];
  /** Independent native checks of the complete output, one per distinct format. */
  validation: ValidationResult[];
  /** Native validation only; this does not imply external decoding verification. */
  validated: boolean;
  sizeBytes: number;
  warnings: string[];
}

export interface StrategyRequirements {
  description: string;
  minInputs: number;
  requiresNativeHandlers: boolean;
  limitations: string[];
}
