const vscode = require("vscode");
const { LanguageClient, TransportKind } = require("vscode-languageclient/node");

let client;

function activate(context) {
  // Find the ash binary on PATH (or use the workspace setting)
  const config = vscode.workspace.getConfiguration("ash");
  const ashPath = config.get("serverPath") || "ash";

  const serverOptions = {
    command: ashPath,
    args: ["lsp"],
    transport: TransportKind.stdio,
  };

  const clientOptions = {
    documentSelector: [{ scheme: "file", language: "ash" }],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/*.ash"),
    },
  };

  client = new LanguageClient("ash-lsp", "Ash Language Server", serverOptions, clientOptions);
  client.start();
}

function deactivate() {
  if (client) return client.stop();
}

module.exports = { activate, deactivate };
