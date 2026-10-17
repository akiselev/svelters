export const settings = $state({ factor: 2 });
export function doubled(n) { return n * settings.factor; }
