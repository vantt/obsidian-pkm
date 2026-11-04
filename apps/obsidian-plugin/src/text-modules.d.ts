// Templates are bundled as text (esbuild `text` loader; a small transform in vitest.config.ts for tests).
declare module '*.md' {
  const text: string;
  export default text;
}
declare module '*.base' {
  const text: string;
  export default text;
}
