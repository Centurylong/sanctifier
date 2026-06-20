"use client";

interface CodeSnippetProps {
  code: string;
  highlightLine?: number;
  language?: string;
}

export function CodeSnippet({ code, highlightLine, language = "rust" }: CodeSnippetProps) {
  const lines = code.split("\n");

  return (
    <pre
      className="overflow-x-auto rounded-lg p-4 text-sm font-mono"
      style={{
        backgroundColor: "var(--muted)",
        color: "var(--muted-foreground)",
      }}
      role="region"
      aria-label={`Code snippet${language ? ` in ${language}` : ""}`}
    >
      <code>
        {lines.map((line, i) => (
          <div
            key={i}
            className={`px-2 py-0.5 -mx-2 ${
              highlightLine === i + 1 ? "border-l-2" : ""
            }`}
            style={
              highlightLine === i + 1
                ? {
                    backgroundColor: "var(--warning)",
                    opacity: 0.2,
                    borderColor: "var(--warning)",
                  }
                : {}
            }
          >
            <span
              className="select-none w-8 inline-block mr-4"
              style={{ color: "var(--muted-foreground)", opacity: 0.5 }}
              aria-hidden="true"
            >
              {i + 1}
            </span>
            {line || " "}
          </div>
        ))}
      </code>
    </pre>
  );
}
