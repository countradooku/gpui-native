import { render } from "gpui-vue"

import App from "./App.vue"

render(App, {
  title: "Vector Grid · 100,000 live instruments",
  appName: "GPUI Vue Market Stream",
  width: 1440,
  height: 900,
  minWidth: 1080,
  minHeight: 700,
  titlebarTransparent: true,
  windowBackground: "opaque",
  trafficLightX: 16,
  trafficLightY: 17,
})
