import type { CodegenConfig } from "@graphql-codegen/cli";

const config: CodegenConfig = {
  schema: "graphql/schema.graphql",
  documents: ["main.ts"],
  generates: {
    "__generated__/": {
      preset: "client",
      plugins: [],
      config: {
        enumsAsTypes: true,
      },
      presetConfig: {
        gqlTagName: "gql",
        fragmentMasking: false,
      },
    },
  },
};

export default config;
