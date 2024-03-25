const path = require("path");

const HtmlWebpackPlugin = require("html-webpack-plugin");
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");

const dist = path.resolve(__dirname, "dist");

module.exports = {
  mode: "production",
  entry: "./public/index.js",
  experiments: {
    syncWebAssembly: true,
  },
  module: {
    rules: [
      {
        test: /\.css$/i,
        use: ["style-loader", "css-loader"],
      },
    ],
  },
  plugins: [
    new HtmlWebpackPlugin({
      title: "Koto Test",
      template: "./public/index.html",
    }),

    new WasmPackPlugin({
      crateDirectory: __dirname,
    }),
  ],
  output: {
    filename: "[name].js",
    path: dist,
    clean: true,
  },
  devServer: {
    static: dist,
  },
};
