/* First-party native loader for gpui-native. */
const { existsSync } = require("node:fs")
const { join } = require("node:path")

function platformFilename() {
  const platform = process.platform
  const arch = process.arch
  if (platform === "linux") return `gpui-native.linux-${arch}-gnu.node`
  if (platform === "darwin") return `gpui-native.darwin-${arch}.node`
  if (platform === "win32") return `gpui-native.win32-${arch}-msvc.node`
  return null
}

function loadBinding() {
  const requested = process.env.GPUI_NATIVE_LIBRARY_PATH ?? process.env.GPUI_VUE_NATIVE_LIBRARY_PATH
  const filename = platformFilename()
  const candidates = [
    requested,
    filename && join(__dirname, filename),
    join(__dirname, "gpui-native.node"),
  ].filter(Boolean)

  const errors = []
  for (const candidate of candidates) {
    if (!existsSync(candidate)) continue
    try {
      return require(candidate)
    } catch (error) {
      errors.push(error)
    }
  }

  const detail = errors.map((error) => `\n- ${error.message}`).join("")
  throw new Error(
    `Could not load the first-party gpui-native addon for ${process.platform}/${process.arch}. ` +
      `Run \"bun --filter @gpui-native/core build\" first.${detail}`,
  )
}

module.exports = loadBinding()
