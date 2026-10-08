// Loads the wasm package directly from ../js/index.js, which is the same entry
// point npm consumers get — so if the demo works, the published package works.
import { analyzeReport, init, version } from "../js/index.js";
import { decodeShareHash, makeShareUrl } from "./permalink.js";

// Lowercase to match what the engine emits; capitalized only for display.
const SEVERITIES = ["critical", "high", "medium", "low", "info"];
const SEVERITY_RANK = Object.fromEntries(SEVERITIES.map((s, i) => [s, i]));

const el = {
  status: document.getElementById("status"),
  source: document.getElementById("source"),
  analyze: document.getElementById("analyze"),
  sample: document.getElementById("load-sample"),
  share: document.getElementById("share"),
  sharePanel: document.getElementById("share-panel"),
  shareUrl: document.getElementById("share-url"),
  shareFeedback: document.getElementById("share-feedback"),
  summary: document.getElementById("summary"),
  results: document.getElementById("results"),
  timing: document.getElementById("timing"),
};

const SAMPLE = `use soroban_sdk::{contract, contractimpl, Address, Env, Symbol};

#[contract]
pub struct Vault;

#[contractimpl]
impl Vault {
    // No require_auth: anyone can withdraw anyone else's balance.
    pub fn withdraw(env: Env, from: Address, amount: i128) {
        let balance: i128 = env.storage().persistent().get(&from).unwrap();
        // Unchecked arithmetic, and an unwrap that panics on a missing key.
        env.storage().persistent().set(&from, &(balance - amount));
    }

    pub fn set_admin(env: Env, new_admin: Address) {
        env.storage().instance().set(&Symbol::new(&env, "admin"), &new_admin);
    }
}
`;

function setStatus(text, kind = "") {
  el.status.textContent = text;
  el.status.className = `status ${kind}`.trim();
}

function titleCase(word) {
  return String(word).charAt(0).toUpperCase() + String(word).slice(1);
}

function severityVar(severity) {
  return `var(--${String(severity).toLowerCase()}, var(--info))`;
}

function renderSummary(summary) {
  const counts = {
    critical: summary.critical,
    high: summary.high,
    medium: summary.medium,
    low: summary.low,
    info: summary.info,
  };

  el.summary.replaceChildren(
    ...SEVERITIES.filter((s) => counts[s] > 0).map((severity) => {
      const li = document.createElement("li");
      const dot = document.createElement("span");
      dot.className = "dot";
      dot.style.background = severityVar(severity);
      // The count and the word carry the meaning; the dot is decoration.
      dot.setAttribute("aria-hidden", "true");
      li.append(dot, `${titleCase(severity)} ${counts[severity]}`);
      return li;
    }),
  );
  el.summary.hidden = el.summary.childElementCount === 0;
}

function renderFindings(findings) {
  if (findings.length === 0) {
    el.results.replaceChildren(
      Object.assign(document.createElement("p"), {
        className: "empty",
        textContent: "No findings. That is not the same as no vulnerabilities.",
      }),
    );
    return;
  }

  // Most severe first; stable within a severity so repeat runs do not reshuffle.
  const sorted = [...findings].sort(
    (a, b) => (SEVERITY_RANK[a.severity] ?? 99) - (SEVERITY_RANK[b.severity] ?? 99),
  );

  el.results.replaceChildren(
    ...sorted.map((f) => {
      const card = document.createElement("article");
      card.className = "finding";
      card.style.borderLeftColor = severityVar(f.severity);

      const head = document.createElement("div");
      head.className = "finding-head";

      const sev = document.createElement("span");
      sev.className = "sev";
      sev.style.color = severityVar(f.severity);
      sev.textContent = titleCase(f.severity);

      const code = document.createElement("span");
      code.className = "code";
      code.textContent = f.code;

      head.append(sev, code);

      const message = document.createElement("p");
      message.textContent = f.message;

      const loc = document.createElement("p");
      loc.className = "loc";
      // Some detectors already encode the line into `location` ("line 10"),
      // so only append it when it adds something.
      const lineLabel = f.line == null ? null : `line ${f.line}`;
      loc.textContent = [
        f.location,
        f.function_name && `fn ${f.function_name}`,
        lineLabel && !String(f.location).includes(lineLabel) ? lineLabel : null,
      ]
        .filter(Boolean)
        .join(" · ");

      card.append(head, message, loc);
      return card;
    }),
  );
}

function run() {
  const source = el.source.value;
  if (!source.trim()) {
    setStatus("Paste some contract source first.", "error");
    return;
  }

  el.analyze.disabled = true;
  try {
    const started = performance.now();
    const report = analyzeReport(source);
    const elapsed = performance.now() - started;

    renderSummary(report.summary);
    renderFindings(report.findings);
    // The acceptance criterion for this demo is a sub-2s round-trip-free
    // analysis, so the number is shown rather than claimed.
    el.timing.textContent = `${report.findings.length} finding(s) in ${elapsed.toFixed(0)} ms`;
    setStatus("Analysis ran locally — nothing left this tab.", "ready");
  } catch (err) {
    setStatus(`Analysis failed: ${err.message}`, "error");
    el.timing.textContent = "";
  } finally {
    el.analyze.disabled = false;
  }
}

// The actual source stays entirely client-side. Only an explicit share click
// creates a URL containing source; do not send it to a paste service.
function shareSource() {
  try {
    const url = makeShareUrl(el.source.value, window.location.href);
    el.shareUrl.value = url;
    el.sharePanel.hidden = false;
    el.shareFeedback.textContent = "Share link ready. Anyone with this link can read its embedded source.";
    // Clipboard is not available on insecure HTTP origins. The visible input
    // remains usable for manual copying in either case.
    if (navigator.clipboard?.writeText && window.isSecureContext) {
      navigator.clipboard.writeText(url).then(
        () => { el.shareFeedback.textContent = "Link copied. The recipient can see its source."; },
        () => { el.shareFeedback.textContent = "Select and copy the link manually."; },
      );
    } else {
      el.shareFeedback.textContent = "Select and copy the link manually.";
    }
  } catch (error) {
    el.sharePanel.hidden = true;
    el.shareFeedback.textContent = error.message;
  }
}

function restoreLink() {
  try {
    const decoded = decodeShareHash(window.location.hash);
    if (decoded === null) return false;
    el.source.value = decoded;
    el.shareUrl.value = window.location.href;
    el.sharePanel.hidden = false;
    el.shareFeedback.textContent = "Source loaded from the share link; review it before analyzing.";
    return true;
  } catch (error) {
    el.sharePanel.hidden = true;
    el.shareFeedback.textContent = error.message;
    return false;
  }
}

el.share.addEventListener("click", shareSource);
el.shareUrl.addEventListener("click", () => el.shareUrl.select());
el.source.addEventListener("input", () => {
  // A URL created before this edit no longer represents the visible source.
  el.sharePanel.hidden = true;
  el.shareFeedback.textContent = "";
});
let engineReady = false;
const restoredOnLoad = restoreLink();
window.addEventListener("hashchange", () => {
  if (restoreLink() && engineReady) run();
});

el.sample.addEventListener("click", () => {
  el.source.value = SAMPLE;
  el.source.focus();
});

el.analyze.addEventListener("click", run);

// Cmd/Ctrl+Enter from the textarea, because that is what anyone who has used
// a code editor will try first.
el.source.addEventListener("keydown", (event) => {
  if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
    event.preventDefault();
    run();
  }
});

init()
  .then(() => {
    engineReady = true;
    el.analyze.disabled = false;
    setStatus(`Analysis engine ready (v${version()}). Nothing you paste leaves this tab.`, "ready");
    if (restoredOnLoad) run();
  })
  .catch((err) => {
    setStatus(
      `Could not load the wasm module: ${err.message}. Serve this directory over HTTP — file:// cannot fetch the .wasm.`,
      "error",
    );
  });
