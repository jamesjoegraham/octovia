/* tslint:disable */
/* eslint-disable */

/**
 * Render a state diagram from the text DSL.
 *
 * # Arguments
 * * `dsl` - The text DSL string (sequence-first syntax).
 * * `viewport_width` - Optional viewport width in pixels (default: 1200).
 * * `viewport_height` - Optional viewport height in pixels (default: 800).
 * * `theme` - Optional theme string: "transit", "ember", "forest", "light", "monochrome".
 *
 * # Returns
 * An SVG string, or a JS error.
 */
export function render_from_dsl(dsl: string, viewport_width?: number | null, viewport_height?: number | null, theme?: string | null): string;

/**
 * Render a state diagram from a JSON description.
 *
 * # Returns
 * An SVG string, or a JS error.
 */
export function render_from_json(json: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly render_from_dsl: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number, number];
    readonly render_from_json: (a: number, b: number) => [number, number, number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
