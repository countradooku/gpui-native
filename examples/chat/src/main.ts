import { render } from "gpui-vue"

import App from "./App.vue"

render(App, {
  title: "Waku · 5,000 messages",
  appName: "GPUI Vue Chat",
  width: 1180,
  height: 820,
  minWidth: 820,
  minHeight: 600,
  titlebarTransparent: true,
  windowBackground: "blurred",
  trafficLightX: 16,
  trafficLightY: 17,
})
