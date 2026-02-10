import type { CodegenConfig } from "@graphql-codegen/cli"

const config: CodegenConfig = {
  schema: "graphql/schema.graphql",
  documents: ["src/**/*.ts"],
  importExtension: ".ts",
  generates: {
    "src/__codegen__/": {
      preset: "client",
      plugins: [],
      config: {
        enumsAsTypes: true,
        useTypeImports: true,
      },
      presetConfig: {
        gqlTagName: "gql",
        fragmentMasking: false,
        importExtension: ".ts",
      },
    },
  },
}

export default config
