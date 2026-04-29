import Pill from "./Pill.svelte";
import { mount } from "svelte";

const app = mount(Pill, { target: document.getElementById("pill-app")! });
export default app;
