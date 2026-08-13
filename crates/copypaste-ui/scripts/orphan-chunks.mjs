/**
 * `@vitejs/plugin-legacy` renders one module graph into two outputs, and the
 * marker behind `import.meta.env.LEGACY` is replaced per output — in
 * `renderChunk`, after chunking. The modern output therefore loses the branch
 * that imported `legacyPolyfills` and keeps the 66 kB chunk that branch put in
 * the graph: packaged on every device, fetched on none.
 *
 * Rollup's answer to that is `generateBundle`, where an output is still a set
 * of files. Removal is conditional on nothing in the output naming the chunk,
 * because deleting one something loads is the outage this exists to prevent.
 */
import path from "node:path";

/**
 * The files nothing else in the output names. `extra` is for a caller holding
 * the entry html outside the file list, which the built gate does.
 */
export function unreferenced(files, extra = "") {
  return files
    .filter(({ name }) => {
      const elsewhere = files.filter((other) => other.name !== name);
      return !extra.includes(name) && !elsewhere.some((other) => other.code.includes(name));
    })
    .map(({ name }) => name);
}

function contents(output) {
  return output.type === "chunk" ? output.code : String(output.source ?? "");
}

export function dropOrphanChunks(chunkName) {
  return {
    name: "copypaste:drop-orphan-chunks",
    apply: "build",
    enforce: "post",
    generateBundle(_options, bundle) {
      const files = Object.values(bundle).map((output) => ({
        fileName: output.fileName,
        name: path.posix.basename(output.fileName),
        code: contents(output),
      }));
      const orphans = new Set(unreferenced(files));

      for (const file of files) {
        if (!chunkName.test(file.name)) continue;
        if (!orphans.has(file.name)) continue;
        delete bundle[file.fileName];
      }
    },
  };
}
