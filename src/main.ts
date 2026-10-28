import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";

const target = document.getElementById("app")!;
const params = new URLSearchParams(window.location.search);

// Session G dev harness: `?idle=1` mounts the ambient screen on its own so it
// can be reviewed/screenshotted without the app shell. Dynamic import keeps it
// out of the initial production payload.
if (params.get("idle") === "1") {
  void import("./lib/components/IdleHarness.svelte").then(({ default: IdleHarness }) => {
    mount(IdleHarness, { target });
  });
} else {
  mount(App, { target });
}
