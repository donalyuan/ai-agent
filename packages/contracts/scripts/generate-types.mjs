import {
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { compileFromFile } from "json-schema-to-typescript";
import prettier from "prettier";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const schemaDirectory = join(packageRoot, "schemas");
const outputDirectory = join(packageRoot, "src");

const schemaFiles = (await readdir(schemaDirectory))
  .filter((file) => file.endsWith(".schema.json"))
  .sort();

if (schemaFiles.length !== 9) {
  throw new Error(`Expected 9 schema files, found ${schemaFiles.length}.`);
}

const schemaBaseId = "https://video-agent.local/schemas/";
const normalizeForGenerator = (value) => {
  if (Array.isArray(value)) {
    return value.map(normalizeForGenerator);
  }
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value)
        .filter(([key]) => key !== "$id")
        .map(([key, nestedValue]) => [key, normalizeForGenerator(nestedValue)]),
    );
  }
  return typeof value === "string" && value.startsWith(schemaBaseId)
    ? `./${value.slice(schemaBaseId.length)}`
    : value;
};

// json-schema-to-typescript resolves absolute references over HTTP. It receives
// a temporary local view only; published schemas retain their stable absolute IDs.
const temporaryDirectory = await mkdtemp(
  join(tmpdir(), "video-agent-contracts-"),
);
let generatedModules;
try {
  await Promise.all(
    schemaFiles.map(async (file) => {
      const schema = JSON.parse(
        await readFile(join(schemaDirectory, file), "utf8"),
      );
      await writeFile(
        join(temporaryDirectory, file),
        `${JSON.stringify(normalizeForGenerator(schema), null, 2)}\n`,
      );
    }),
  );
  generatedModules = await Promise.all(
    schemaFiles.map(async (file) => {
      const schema = JSON.parse(
        await readFile(join(schemaDirectory, file), "utf8"),
      );
      const declaration = await compileFromFile(
        join(temporaryDirectory, file),
        {
          bannerComment:
            "/* This file is generated from JSON Schema. Do not edit manually. */",
          unreachableDefinitions: true,
        },
      );
      return { declaration, file, title: schema.title };
    }),
  );
} finally {
  await rm(temporaryDirectory, { force: true, recursive: true });
}

await mkdir(outputDirectory, { recursive: true });
const generatedDirectory = join(outputDirectory, "generated");
await mkdir(generatedDirectory, { recursive: true });
await Promise.all(
  generatedModules.map(async ({ declaration, file }) =>
    writeFile(
      join(generatedDirectory, file.replace(".schema.json", ".ts")),
      await prettier.format(declaration, { parser: "typescript" }),
    ),
  ),
);
await rm(join(outputDirectory, "generated.ts"), { force: true });
const indexSource = `// Re-exported names are derived from JSON Schema titles.\n${generatedModules
  .map(
    ({ file, title }) =>
      `export type { ${title} } from "./generated/${file.replace(".schema.json", ".js")}";`,
  )
  .join("\n")}\n`;
await writeFile(
  join(outputDirectory, "index.ts"),
  await prettier.format(indexSource, { parser: "typescript" }),
);
