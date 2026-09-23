// Run only through verify.ts: it gives this process an empty environment, a
// synthetic cwd, and no network/process/FFI permission before importing cli.
import { cli } from "../../src/cli.ts"

interface Inspectable {
  getName(): string
  getAliases(): string[]
  getDescription(): string
  getUsage(): string
  getArgsDefinition(): string | undefined
  getArguments(): ReturnType<typeof cli.getArguments>
  getBaseTypes(): ReturnType<typeof cli.getBaseTypes>
  getTypes(): ReturnType<typeof cli.getTypes>
  getExamples(): ReturnType<typeof cli.getExamples>
  getBaseEnvVars(hidden?: boolean): ReturnType<typeof cli.getBaseEnvVars>
  getGlobalEnvVars(hidden?: boolean): ReturnType<typeof cli.getGlobalEnvVars>
  getBaseOptions(hidden?: boolean): ReturnType<typeof cli.getBaseOptions>
  getGlobalOptions(hidden?: boolean): ReturnType<typeof cli.getGlobalOptions>
  getBaseCommands(hidden?: boolean): Inspectable[]
  getCommand(name: string, hidden?: boolean): Inspectable | undefined
}

function argument(arg: ReturnType<Inspectable["getArguments"]>[number]) {
  return {
    name: arg.name,
    type: arg.type,
    action: arg.action,
    optional: arg.optional ?? false,
    variadic: arg.variadic ?? false,
    list: arg.list ?? false,
  }
}

function option(
  opt: ReturnType<Inspectable["getBaseOptions"]>[number],
  scope: string,
) {
  return {
    scope,
    name: opt.name,
    flags: opt.flags,
    typeDefinition: opt.typeDefinition ?? null,
    args: opt.args.map(argument),
    required: opt.required ?? false,
    default: opt.default ?? null,
    collect: opt.collect ?? false,
    conflicts: opt.conflicts ?? [],
    depends: opt.depends ?? [],
    hidden: opt.hidden ?? false,
    global: opt.global ?? false,
    valueHandler: opt.value == null ? "none" : "unknown_function",
  }
}

function typeDefinition(
  definition: ReturnType<Inspectable["getTypes"]>[number],
  command: Inspectable,
) {
  const handler = definition.handler
  let values: unknown = "not_available"
  if (typeof handler !== "function" && typeof handler.values === "function") {
    const result: unknown = Reflect.apply(handler.values, handler, [command])
    values = Array.isArray(result) && result.every((value) =>
        typeof value === "string" || typeof value === "number" ||
        typeof value === "boolean"
      )
      ? result
      : "unknown_non_scalar_or_async_values"
  }
  return {
    name: definition.name,
    global: definition.global ?? false,
    override: definition.override ?? false,
    handlerKind: typeof handler === "function"
      ? "function_handler"
      : handler.constructor.name,
    values,
  }
}

function envVar(env: ReturnType<Inspectable["getBaseEnvVars"]>[number]) {
  return {
    name: env.name,
    names: env.names,
    description: env.description,
    type: env.type,
    details: argument(env.details),
    hidden: env.hidden ?? false,
    required: env.required ?? false,
    global: env.global ?? false,
  }
}

type Route = ReturnType<typeof route>
function route(command: Inspectable, parts: string[], parent?: Inspectable) {
  const children = command.getBaseCommands(true)
  const visible = new Set(
    parent?.getBaseCommands().map((child) => child.getName()),
  )
  const aliases = command.getAliases()
  return {
    path: parts.join(" "),
    name: command.getName(),
    aliases,
    aliasResolution: parent == null ? [] : aliases.map((alias) => ({
      alias,
      resolves: parent.getCommand(alias, true) === command,
    })),
    hidden: parent == null ? false : !visible.has(command.getName()),
    description: command.getDescription(),
    usage: command.getUsage(),
    argsDefinition: command.getArgsDefinition() ?? null,
    arguments: command.getArguments().map(argument),
    localTypes: command.getBaseTypes().map((definition) =>
      typeDefinition(definition, command)
    ),
    allTypes: command.getTypes().map((definition) =>
      typeDefinition(definition, command)
    ),
    examples: command.getExamples().map((example) => ({
      name: example.name,
      description: example.description,
    })),
    localEnvVars: command.getBaseEnvVars(true).map(envVar),
    inheritedGlobalEnvVars: command.getGlobalEnvVars(true).map(envVar),
    localOptions: command.getBaseOptions(true).map((opt) =>
      option(opt, "local")
    ),
    inheritedGlobalOptions: command.getGlobalOptions(true).map((opt) =>
      option(opt, "inherited_global")
    ),
    kind: parts[1] === "completions" && parts.length > 2
      ? "generated_completion_child"
      : parts.length === 2 && parts[1] === "completions"
      ? "source_leaf"
      : children.length === 0
      ? "source_leaf"
      : "parent_route",
    parentAction: children.length === 0 || parts[1] === "completions"
      ? "not_applicable"
      : "pending_safe_fixture",
    children: children.map((child) => child.getName()),
  }
}

function walk(
  command: Inspectable,
  parts: string[],
  parent?: Inspectable,
): Route[] {
  return [
    route(command, parts, parent),
    ...command.getBaseCommands(true).flatMap(
      (child) => walk(child, [...parts, child.getName()], command),
    ),
  ]
}

console.log(JSON.stringify({ routes: walk(cli, ["linear"]) }))
