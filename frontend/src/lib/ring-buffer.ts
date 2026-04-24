export function addToRingBuffer<T>(buffer: T[], item: T, maxSize: number): T[] {
  return [item, ...buffer].slice(0, maxSize)
}
