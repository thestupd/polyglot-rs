export type PolyglotErrorCode =
  | 'ERR_INVALID_INPUT'
  | 'ERR_LIMIT_EXCEEDED'
  | 'ERR_DUPLICATE_FORMAT'
  | 'ERR_DUPLICATE_STRATEGY'
  | 'ERR_UNKNOWN_STRATEGY'
  | 'ERR_UNSUPPORTED_COMBINATION'
  | 'ERR_INVALID_COMPOSITION'
  | 'ERR_NATIVE_UNAVAILABLE'
  | 'ERR_NATIVE_FAILURE'
  | 'ERR_CUSTOM_DETECTION';

export class PolyglotError extends Error {
  constructor(
    public readonly code: PolyglotErrorCode,
    message: string,
    options?: ErrorOptions,
  ) {
    super(message, options);
    this.name = 'PolyglotError';
  }
}

export function fromNativeError(error: unknown): PolyglotError {
  if (error instanceof PolyglotError) return error;
  const message = error instanceof Error ? error.message : String(error);
  const code = (['ERR_INVALID_INPUT', 'ERR_LIMIT_EXCEEDED', 'ERR_DUPLICATE_FORMAT', 'ERR_DUPLICATE_STRATEGY', 'ERR_UNKNOWN_STRATEGY', 'ERR_UNSUPPORTED_COMBINATION', 'ERR_INVALID_COMPOSITION'] as const)
    .find((candidate) => message.startsWith(`${candidate}:`));
  return new PolyglotError(code ?? 'ERR_NATIVE_FAILURE', message, { cause: error });
}
