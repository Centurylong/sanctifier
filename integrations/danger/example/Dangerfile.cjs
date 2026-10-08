// Run with: npx danger ci --dangerfile integrations/danger/example/Dangerfile.cjs
const { danger, warn, fail, message, markdown } = require("danger");
const { runSanctifierCli } = require("../sanctifier-danger.cjs");
const changedFiles = [...(danger.git.modified_files || []), ...(danger.git.created_files || [])];
runSanctifierCli({ warn, fail, message, markdown }, {
  projectPath: process.env.SANCTIFIER_TARGET || ".",
  changedFiles,
  maxAnnotations: 12
});
