import { Kind, parse, print } from "graphql"
import type { OperationInput } from "./graphql-match.ts"

// Frozen 2.6.0 declares teamKey but never uses or supplies it. Only the actual
// comparison copy may drop that exact declaration; raw requests stay intact.
export const C018_SOURCE_GET_LABEL_BY_NAME = print(parse(`
  query GetLabelByName($name: String!, $teamKey: String) {
    issueLabels(filter: { name: { eqIgnoreCase: $name } }) {
      nodes { id name color team { key name } }
    }
  }
`))

export function withoutFrozenLabelTeamDeclaration(
  input: OperationInput,
): OperationInput {
  if (
    input.operationName != null && input.operationName !== "GetLabelByName" ||
    input.variables == null || Object.keys(input.variables).length !== 1 ||
    !Object.hasOwn(input.variables, "name")
  ) return input
  let document
  try {
    document = parse(input.document)
  } catch {
    return input // The ordinary matcher still reports the parse failure.
  }
  if (print(document) !== C018_SOURCE_GET_LABEL_BY_NAME) return input
  return {
    ...input,
    document: print({
      ...document,
      definitions: document.definitions.map((definition) =>
        definition.kind === Kind.OPERATION_DEFINITION
          ? {
            ...definition,
            variableDefinitions: definition.variableDefinitions?.filter(
              (variable) => variable.variable.name.value !== "teamKey",
            ),
          }
          : definition
      ),
    }),
  }
}
