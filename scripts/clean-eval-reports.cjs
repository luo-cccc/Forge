const fs = require("fs");
const path = require("path");

const reportsDir = path.join(__dirname, "..", "reports");
const reportPath = path.join(reportsDir, "eval_report.json");

if (fs.existsSync(reportPath)) {
  fs.rmSync(reportPath, { force: true });
}

if (fs.existsSync(reportsDir) && fs.readdirSync(reportsDir).length === 0) {
  fs.rmdirSync(reportsDir);
}
