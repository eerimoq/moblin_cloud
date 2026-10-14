/* @refresh reload */
import { render } from "solid-js/web";
import "./index.css";
import SongRequest from "./pages/SongRequest";

render(() => <SongRequest />, document.getElementById("root")!);
