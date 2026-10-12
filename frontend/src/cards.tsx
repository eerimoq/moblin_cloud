/* @refresh reload */
import { render } from "solid-js/web";
import "./index.css";
import "./cards.css";
import Cards from "./pages/Cards";

render(() => <Cards />, document.getElementById("root")!);
