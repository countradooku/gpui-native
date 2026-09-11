import MagicString from "magic-string"
import { compile, compileModule, parse, VERSION, type CompileResult } from "svelte/compiler"
import ts from "typescript"

export const SVELTE_VERSION = "5.57.0"
const tags = new Set([
  "div",
  "text",
  "img",
  "svg",
  "canvas",
  "input",
  "textarea",
  "anchored",
  "code",
  "diff",
  "markdown",
  "virtual-list",
])
const imports: Record<string, string> = {
  svelte: "@gpui-native/svelte/public",
  "svelte/internal/client": "@gpui-native/svelte/engine",
  "svelte/store": "@gpui-native/svelte/store",
  "svelte/reactivity": "@gpui-native/svelte/reactivity",
}
export function nativeRuntimeImport(name: string): string | undefined {
  return imports[name]
}
export function rewriteImports(code: string): string {
  const source = ts.createSourceFile("module.ts", code, ts.ScriptTarget.Latest, true)
  const edited = new MagicString(code)
  const replace = (node: ts.Expression | undefined) => {
    if (!node || !ts.isStringLiteral(node)) return
    const mapped = nativeRuntimeImport(node.text)
    if (mapped) edited.overwrite(node.getStart(source), node.end, JSON.stringify(mapped))
  }
  const visit = (node: ts.Node) => {
    if (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) replace(node.moduleSpecifier)
    else if (ts.isCallExpression(node) && node.expression.kind === ts.SyntaxKind.ImportKeyword)
      replace(node.arguments[0])
    ts.forEachChild(node, visit)
  }
  visit(source)
  return edited.toString()
}
export function compileNative(source: string, filename = "App.svelte"): CompileResult {
  if (VERSION !== SVELTE_VERSION)
    throw new Error(`GPUI Svelte requires Svelte ${SVELTE_VERSION}; found ${VERSION}`)
  if (/\.svelte\.[jt]s$/.test(filename)) {
    const result = compileModule(
      filename.endsWith(".ts")
        ? ts.transpileModule(source, {
            compilerOptions: {
              target: ts.ScriptTarget.ES2022,
              module: ts.ModuleKind.ESNext,
              verbatimModuleSyntax: true,
            },
          }).outputText
        : source,
      { filename, generate: "client", dev: false },
    )
    result.js.code = rewriteImports(result.js.code)
    return result
  }
  const ast = parse(source, { modern: true, filename })
  if (ast.css) throw new Error(`${filename}: GPUI uses native style objects, not CSS stylesheets`)
  const edited = new MagicString(source)
  let hasNative = false
  let hostName = "GpuiNativeHostInternal"
  while (source.includes(hostName)) hostName += "_"
  const walk = (value: unknown): void => {
    if (!value || typeof value !== "object") return
    const node = value as {
      type?: string
      name?: string
      start?: number
      end?: number
      attributes?: { type: string; name: string }[]
    }
    if (node.type === "RegularElement") {
      if (!tags.has(node.name!))
        throw new Error(`${filename}: unsupported native element <${node.name}>`)
      hasNative = true
      const start = node.start!
      edited.overwrite(start + 1, start + 1 + node.name!.length, `${hostName} type="${node.name}"`)
      const end = node.end!
      const closing = source.lastIndexOf(`</${node.name}`, end - 1)
      if (closing >= start) edited.overwrite(closing + 2, closing + 2 + node.name!.length, hostName)
    }
    if (
      [
        "SvelteWindow",
        "SvelteDocument",
        "SvelteBody",
        "SvelteHead",
        "SvelteElement",
        "HtmlTag",
      ].includes(node.type ?? "")
    )
      throw new Error(
        `${filename}: ${node.type} is a browser-only feature; use GPUI components and window APIs`,
      )
    for (const attr of node.attributes ?? [])
      if (
        [
          "StyleDirective",
          "ClassDirective",
          "TransitionDirective",
          "AnimateDirective",
          "UseDirective",
          "OnDirective",
        ].includes(attr.type)
      )
        throw new Error(
          `${filename}: ${attr.type} is not a native GPUI directive; use style/motion props`,
        )
    for (const [key, child] of Object.entries(value))
      if (!["parent", "metadata", "instance", "module", "css"].includes(key)) {
        if (Array.isArray(child)) child.forEach(walk)
        else walk(child)
      }
  }
  walk(ast.fragment)
  if (hasNative) {
    const statement = `import { Native as ${hostName} } from "@gpui-native/svelte/components";`
    if (ast.instance) edited.appendLeft(source.indexOf(">", ast.instance.start) + 1, statement)
    else edited.prepend(`<script>${statement}</script>`)
  }
  const result = compile(edited.toString(), {
    filename,
    generate: "client",
    runes: true,
    dev: false,
    fragments: "tree",
    sourcemap: JSON.parse(
      edited.generateMap({ source: filename, includeContent: true, hires: true }).toString(),
    ),
  })
  result.js.code = rewriteImports(result.js.code).replace(
    /import ['"]svelte\/internal\/disclose-version['"];?\n?/g,
    "",
  )
  return result
}
