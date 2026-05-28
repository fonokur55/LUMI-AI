import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import { useCallback, useState } from "react";
import "./MarkdownMessage.css";
// highlight.js téma - sötét, illeszkedik a LUMI-hoz.
import "highlight.js/styles/github-dark.css";

type Props = {
  content: string;
};

/**
 * AKASHA chat-buborékok Markdown renderelője.
 *
 * Felismeri:
 *  - **félkövér** és *dőlt*
 *  - `inline kód`
 *  - ```nyelv\nkód-blokk\n``` (szintaxiskiemeléssel, copy gombbal)
 *  - listák (- * 1.), idézetek (>), címsorok (#), linkek
 *  - táblázatok és áthúzott szöveg (GitHub-flavored extras)
 *
 * A modell bármilyen Markdown-t kibocsáthat - itt biztonsággal HTML-re alakítjuk.
 * (react-markdown alapból nem engedi a `<script>`, `<iframe>` stb. injekciókat.)
 */
export function MarkdownMessage({ content }: Props) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      rehypePlugins={[[rehypeHighlight, { ignoreMissing: true, detect: true }]]}
      components={{
        // Kódblokkok: copy gombbal + nyelv-fejléccel ha van megadva.
        // Inline kód: külön, csak <code> stilizálva.
        code(props) {
          const { children, className, node, ...rest } = props as {
            children?: React.ReactNode;
            className?: string;
            node?: { position?: { start?: { line: number } } };
            inline?: boolean;
          };
          const text = String(children ?? "").replace(/\n$/, "");
          const langMatch = /language-([\w-]+)/.exec(className || "");
          // react-markdown a `<pre><code>`-ban hívja meg a code componens-t,
          // inline esetben pedig csak `<code>`-ban. A `node` `position`-jét
          // figyelve döntjük el, hogy multi-line block-e vagy nem - egyszerűbb
          // viszont a `className` jelenlétére hagyatkozni: a block-okra mindig
          // generálódik nyelv osztály (akár "language-text"), inline-ra nem.
          const isBlock = !!className && className.startsWith("language-");
          if (isBlock) {
            return (
              <CodeBlock
                language={langMatch?.[1] ?? "text"}
                code={text}
                className={className}
              />
            );
          }
          return (
            <code className="md-inline-code" {...rest}>
              {children}
            </code>
          );
        },
        // Külsős linkek mindig új ablakban nyíljanak (a Tauri webview-ben
        // egyébként a default click is a webview-en belül navigálna).
        a({ href, children, ...rest }) {
          return (
            <a
              href={href}
              target="_blank"
              rel="noopener noreferrer"
              {...rest}
            >
              {children}
            </a>
          );
        },
        // Saját class-ok hogy a CSS könnyebben megcélozhassa.
        ul: (p) => <ul className="md-ul" {...p} />,
        ol: (p) => <ol className="md-ol" {...p} />,
        blockquote: (p) => <blockquote className="md-blockquote" {...p} />,
        h1: (p) => <h1 className="md-h1" {...p} />,
        h2: (p) => <h2 className="md-h2" {...p} />,
        h3: (p) => <h3 className="md-h3" {...p} />,
        table: (p) => (
          <div className="md-table-wrap">
            <table className="md-table" {...p} />
          </div>
        ),
      }}
    >
      {content}
    </ReactMarkdown>
  );
}

function CodeBlock({
  code,
  language,
  className,
}: {
  code: string;
  language: string;
  className?: string;
}) {
  const [copied, setCopied] = useState(false);

  const copy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1800);
    } catch {
      // ignore
    }
  }, [code]);

  return (
    <div className="md-codeblock">
      <div className="md-codeblock__head">
        <span className="md-codeblock__lang">
          <span className="md-codeblock__dot" aria-hidden />
          {language}
        </span>
        <CopyButton copied={copied} onClick={copy} />
      </div>
      <pre className="md-codeblock__pre">
        <code className={className}>{code}</code>
      </pre>
      {/* Alsó copy mindig látszik - a user explicit kérése. */}
      <div className="md-codeblock__foot">
        <CopyButton copied={copied} onClick={copy} withLabel />
      </div>
    </div>
  );
}

/**
 * Egységes másolás-gomb komponens: copy.png ikon, success állapotban
 * zöld pipa. Két variáns: csak ikon (kódkártya fejlécében) és ikon+label
 * ("Kód másolása", az alsó verzióban).
 */
export function CopyButton({
  copied,
  onClick,
  withLabel = false,
}: {
  copied: boolean;
  onClick: () => void;
  withLabel?: boolean;
}) {
  return (
    <button
      type="button"
      className={`copy-btn ${copied ? "is-copied" : ""}`}
      onClick={onClick}
      aria-label={copied ? "Másolva" : "Másolás"}
      title={copied ? "Másolva!" : "Másolás"}
    >
      {copied ? (
        <>
          <span className="copy-btn__check" aria-hidden>
            ✓
          </span>
          {withLabel && <span className="copy-btn__label">Másolva</span>}
        </>
      ) : (
        <>
          <img
            className="copy-btn__icon"
            src="/icons/copy.png"
            alt=""
            width={14}
            height={14}
          />
          {withLabel && <span className="copy-btn__label">Másolás</span>}
        </>
      )}
    </button>
  );
}
