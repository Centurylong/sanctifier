# Command-Line Help for `sanctifier`

This document contains the help content for the `sanctifier` command-line program.

**Command Overview:**

* [`sanctifier`↴](#sanctifier)
* [`sanctifier analyze`↴](#sanctifier-analyze)
* [`sanctifier badge`↴](#sanctifier-badge)
* [`sanctifier report`↴](#sanctifier-report)
* [`sanctifier init`↴](#sanctifier-init)
* [`sanctifier update`↴](#sanctifier-update)
* [`sanctifier kani`↴](#sanctifier-kani)
* [`sanctifier fix`↴](#sanctifier-fix)
* [`sanctifier generate-docs`↴](#sanctifier-generate-docs)

## `sanctifier`

Stellar Soroban Security & Formal Verification Suite

**Usage:** `sanctifier <COMMAND>`

###### **Subcommands:**

* `analyze` — Analyze a Soroban contract for vulnerabilities
* `badge` — Generate a security badge from a JSON report
* `report` — Generate a summary report
* `init` — Initialize a new Sanctifier project
* `update` — Update the sanctifier binary to the latest Sanctifier binary
* `kani` — Translate Soroban contract into a Kani-verifiable harness
* `fix` — Automatically fix basic vulnerabilities and code issues
* `generate-docs` — Print the CLI reference as Markdown (used to keep docs/cli.md up to date)



## `sanctifier analyze`

Analyze a Soroban contract for vulnerabilities

**Usage:** `sanctifier analyze [OPTIONS] [PATH]`

###### **Arguments:**

* `<PATH>` — Path to the Soroban contract or project directory

  Default value: `.`

###### **Options:**

* `-f`, `--format <FORMAT>` — Output format (text, json)

  Default value: `text`
* `-l`, `--limit <LIMIT>` — Maximum ledger entry size limit in bytes

  Default value: `64000`
* `--llm-explain` — Enable LLM-assisted explanations for findings

  Default value: `false`



## `sanctifier badge`

Generate a security badge from a JSON report

**Usage:** `sanctifier badge [OPTIONS]`

###### **Options:**

* `-r`, `--report <REPORT>` — Path to Sanctifier JSON report (from `sanctifier analyze --format json`)

  Default value: `sanctifier-report.json`
* `--svg-output <SVG_OUTPUT>` — Where to write generated badge SVG

  Default value: `sanctifier-security.svg`
* `--markdown-output <MARKDOWN_OUTPUT>` — Where to write generated markdown snippet
* `--badge-url <BADGE_URL>` — Public URL for the SVG (used by markdown output). Falls back to local SVG path



## `sanctifier report`

Generate a summary report

**Usage:** `sanctifier report [OPTIONS]`

###### **Options:**

* `-o`, `--output <OUTPUT>` — Optional path to save the generated report



## `sanctifier init`

Initialize a new Sanctifier project

**Usage:** `sanctifier init [OPTIONS]`

###### **Options:**

* `-f`, `--force` — Force overwrite existing configuration file



## `sanctifier update`

Update the sanctifier binary to the latest Sanctifier binary

**Usage:** `sanctifier update`



## `sanctifier kani`

Translate Soroban contract into a Kani-verifiable harness

**Usage:** `sanctifier kani [OPTIONS] <PATH>`

###### **Arguments:**

* `<PATH>` — Path to the .rs file to translate

###### **Options:**

* `-o`, `--output <OUTPUT>` — Optional path to save the generated harness



## `sanctifier fix`

Automatically fix basic vulnerabilities and code issues

**Usage:** `sanctifier fix [OPTIONS] <PATH>`

###### **Arguments:**

* `<PATH>` — Path to the Soroban contract or project directory

###### **Options:**

* `-y`, `--yes` — Apply fixes without confirmation
* `-d`, `--dry-run` — Show what would be changed without modifying files



## `sanctifier generate-docs`

Print the CLI reference as Markdown (used to keep docs/cli.md up to date)

**Usage:** `sanctifier generate-docs`



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>
