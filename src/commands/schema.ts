import { Command } from "@cliffy/command"
import {
  buildClientSchema,
  getIntrospectionQuery,
  type IntrospectionQuery,
  lexicographicSortSchema,
  printSchema,
} from "graphql"
import { getGraphQLClient } from "../utils/graphql.ts"

export const schemaCommand = new Command()
  .name("schema")
  .description("Print the GraphQL schema to stdout")
  .option("--json", "Output as JSON introspection result instead of SDL")
  .option(
    "-o, --output <file:string>",
    "Write schema to file instead of stdout",
  )
  .action(async (options) => {
    const { json, output } = options

    const client = getGraphQLClient()
    const introspectionQuery = getIntrospectionQuery()
    const result = await client.request<IntrospectionQuery>(introspectionQuery)

    let content: string
    if (json) {
      content = JSON.stringify(result, null, 2)
    } else {
      const schema = lexicographicSortSchema(buildClientSchema(result))
      content = printSchema(schema)
    }

    if (output) {
      await Deno.writeTextFile(output, content + "\n")
      console.log(`Schema written to ${output}`)
    } else {
      console.log(content)
    }
  })
