import koto from "../Cargo.toml";
import * as ace from "brace";
import "brace/theme/solarized_dark";
import "./main.css";

var editor = ace.edit("editor");
editor.setTheme("ace/theme/solarized_dark");
editor.session.on("change", compile_and_run);

function compile_and_run() {
  const result = koto.compile_and_run(editor.session.getValue());
  document.getElementById("output").innerText = result;
}

compile_and_run();
