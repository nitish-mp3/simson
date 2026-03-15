import resolve from "@rollup/plugin-node-resolve";
import typescript from "@rollup/plugin-typescript";
import { terser } from "rollup-plugin-terser";
import json from "@rollup/plugin-json";

const production = !process.env.ROLLUP_WATCH;

export default {
  input: "src/voip-card.ts",
  output: {
    file: "build/ha-voip-card.js",
    format: "es",
    sourcemap: !production,
    inlineDynamicImports: true,
  },
  plugins: [
    resolve({
      browser: true,
      dedupe: ["lit", "@lit/reactive-element"],
    }),
    typescript({
      tsconfig: "./tsconfig.json",
      declaration: false,
      declarationMap: false,
      sourceMap: !production,
    }),
    json(),
    production &&
      terser({
        ecma: 2020,
        module: true,
        compress: {
          passes: 2,
          drop_console: false,
        },
        output: {
          comments: false,
        },
      }),
  ],
  onwarn(warning, warn) {
    // Suppress circular dependency warnings from lit
    if (warning.code === "CIRCULAR_DEPENDENCY") return;
    warn(warning);
  },
};
