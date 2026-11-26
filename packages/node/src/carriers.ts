import { Buffer } from 'node:buffer';

/** The image formats that can act as a carrier for an embedded payload. */
export const CARRIER_FORMATS = ['png', 'gif', 'jpeg'] as const;
export type CarrierFormat = (typeof CARRIER_FORMATS)[number];

// Minimal 1x1 images used as the default carrier when only a payload is given.
// Each validates against this library's own handler for its format.
const DEFAULT_CARRIERS: Record<CarrierFormat, string> = {
  png: 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAACXBIWXMAAAABAAAAAQBPJcTWAAAADElEQVR4nGNkYGAAAAAIAAI76MGHAAAAAElFTkSuQmCC',
  gif: 'R0lGODlhAQABAIAAAAAAAP///yH5BAAAAAAALAAAAAABAAEAAAICRAEAOw==',
  jpeg: '/9j/4AAQSkZJRgABAgAAAQABAAD//gAQTGF2YzYyLjExLjEwMAD/2wBDAAgEBAQEBAUFBQUFBQYGBgYGBgYGBgYGBgYHBwcICAgHBwcGBgcHCAgICAkJCQgICAgJCQoKCgwMCwsODg4RERT/xABLAAEBAAAAAAAAAAAAAAAAAAAACAEBAAAAAAAAAAAAAAAAAAAAARABAAAAAAAAAAAAAAAAAAAAABEBAAAAAAAAAAAAAAAAAAAAAP/AABEIAAEAAQMBIgACEQADEQD/2gAMAwEAAhEDEQA/AJ/AAf/Z',
};

export function isCarrierFormat(value: string): value is CarrierFormat {
  return (CARRIER_FORMATS as readonly string[]).includes(value);
}

/** A fresh 1x1 carrier image for the given format. */
export function defaultCarrier(format: CarrierFormat): Buffer {
  return Buffer.from(DEFAULT_CARRIERS[format], 'base64');
}
