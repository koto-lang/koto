import * as koto from "../pkg/index.js";

import * as ace from "brace";
import "brace/theme/solarized_dark";
import "./main.css";

var editorDiv = document.getElementById("editor");
editorDiv.innerHTML = `\
# Fizz buzz in Koto

fizz_buzz = |n|
  match n % 3, n % 5
    0, 0 then "Fizz Buzz"
    0, _ then "Fizz"
    _, 0 then "Buzz"
    else n

for n in 1..20
  print fizz_buzz n
`;

var editor = ace.edit("editor");
editor.setTheme("ace/theme/solarized_dark");
editor.session.on("change", compile_and_run);
editor.setShowPrintMargin(false);
editor.setBehavioursEnabled(false);

function compile_and_run() {
  const result = koto.compile_and_run(editor.session.getValue());
  document.getElementById("output").innerText = result;
}

compile_and_run();
