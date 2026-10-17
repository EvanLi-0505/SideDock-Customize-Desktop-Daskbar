// Quick answers of the launcher search: arithmetic, web addresses, web search.
// No `eval`: arithmetic goes through a small parser that only knows numbers and operators.

import type { SearchEngine } from "./types.ts";

// ---------------- arithmetic ----------------

/** full-width and math symbols people type, mapped to the parser's */
const SYMBOLS: Record<string, string> = {
  "×": "*",
  "x": "*",
  "X": "*",
  "÷": "/",
  "＋": "+",
  "－": "-",
  "−": "-",
  "＊": "*",
  "／": "/",
  "（": "(",
  "）": ")",
  "％": "%",
  "＾": "^",
  "，": ".",
  "。": ".",
};

type Token = { type: "num"; value: number } | { type: "op"; value: string };

function tokenize(text: string): Token[] | null {
  const tokens: Token[] = [];
  let i = 0;
  while (i < text.length) {
    const c = SYMBOLS[text[i]!] ?? text[i]!;
    if (c === " ") {
      i++;
    } else if (/[0-9.]/.test(c)) {
      let j = i;
      let s = "";
      while (j < text.length) {
        const d = SYMBOLS[text[j]!] ?? text[j]!;
        if (!/[0-9.]/.test(d)) break;
        s += d;
        j++;
      }
      if ((s.match(/\./g) ?? []).length > 1) return null;
      const value = Number(s);
      if (!Number.isFinite(value)) return null;
      tokens.push({ type: "num", value });
      i = j;
    } else if ("+-*/%^()".includes(c)) {
      tokens.push({ type: "op", value: c });
      i++;
    } else {
      return null;
    }
  }
  return tokens;
}

/** Evaluates `query` when it is plain arithmetic with at least one operation, else null. */
export function evaluateMath(query: string): number | null {
  const text = query.trim().replace(/=$/, "").trim();
  if (!text || text.length > 200) return null;
  // a date ("2024-10-18", "2024/10/18") is not a subtraction
  if (/^\d{4}[-/.]\d{1,2}[-/.]\d{1,2}$/.test(text)) return null;
  const parsed = tokenize(text);
  if (!parsed || parsed.length < 3) return null;
  // a non-null binding for the parser's closures
  const tokens: Token[] = parsed;
  // a lone (negative) number is not a calculation
  if (!tokens.some((t, i) => t.type === "op" && t.value !== "(" && t.value !== ")" && i > 0)) return null;

  let pos = 0;
  const peek = () => tokens[pos];
  const isOp = (value: string) => {
    const t = peek();
    return t?.type === "op" && t.value === value;
  };

  // expr := term (('+' | '-') term)*
  function expr(): number | null {
    let left = term();
    while (left !== null && (isOp("+") || isOp("-"))) {
      const op = (tokens[pos++] as { value: string }).value;
      const right = term();
      if (right === null) return null;
      left = op === "+" ? left + right : left - right;
    }
    return left;
  }

  // term := power (('*' | '/' | '%') power)*
  function term(): number | null {
    let left = power();
    while (left !== null && (isOp("*") || isOp("/") || isOp("%"))) {
      const op = (tokens[pos++] as { value: string }).value;
      const right = power();
      if (right === null) return null;
      if (op === "*") left *= right;
      else if (op === "/") left /= right;
      else left %= right;
    }
    return left;
  }

  // power := unary ('^' power)?   (right associative)
  function power(): number | null {
    const base = unary();
    if (base === null) return null;
    if (isOp("^")) {
      pos++;
      const exponent = power();
      return exponent === null ? null : base ** exponent;
    }
    return base;
  }

  // unary := ('-' | '+') unary | primary
  function unary(): number | null {
    if (isOp("-")) {
      pos++;
      const v = unary();
      return v === null ? null : -v;
    }
    if (isOp("+")) {
      pos++;
      return unary();
    }
    return primary();
  }

  // primary := number | '(' expr ')'
  function primary(): number | null {
    const t = peek();
    if (!t) return null;
    if (t.type === "num") {
      pos++;
      return t.value;
    }
    if (t.value === "(") {
      pos++;
      const v = expr();
      if (v === null || !isOp(")")) return null;
      pos++;
      return v;
    }
    return null;
  }

  const result = expr();
  if (result === null || pos !== tokens.length || !Number.isFinite(result)) return null;
  return result;
}

/** 0.1 + 0.2 shows as 0.3: 12 significant digits are plenty for a quick answer. */
export function formatNumber(value: number): string {
  if (Number.isInteger(value) && Math.abs(value) < 1e15) return String(value);
  return String(Number(value.toPrecision(12)));
}

// ---------------- web addresses ----------------

/** top-level domains accepted without "http(s)://", so "report.docx" stays a file name */
const TLDS = new Set(
  "com net org io dev app ai co me info xyz top site online tech cloud cn hk tw jp kr uk de fr eu us ca au ru in edu gov tv gg so ly sh fm im cc".split(" "),
);

/** The address `query` stands for, if it is one ("github.com", "https://...", "localhost:3000"). */
export function asUrl(query: string): string | null {
  const s = query.trim();
  if (!s || /\s/.test(s) || s.length > 2048) return null;
  if (/^https?:\/\/[^\s/]+\S*$/i.test(s)) return s;
  if (/^(localhost|(\d{1,3}\.){3}\d{1,3})(:\d{1,5})?(\/\S*)?$/i.test(s)) return `http://${s}`;
  const m = s.match(/^((?:[a-z0-9-]+\.)+([a-z]{2,}))(:\d{1,5})?(\/\S*)?$/i);
  if (m && TLDS.has(m[2]!.toLowerCase())) return `https://${s}`;
  return null;
}

// ---------------- web search ----------------

const ENGINES: Record<Exclude<SearchEngine, "None">, { name: string; url: string }> = {
  Bing: { name: "Bing", url: "https://www.bing.com/search?q=" },
  Google: { name: "Google", url: "https://www.google.com/search?q=" },
  Baidu: { name: "百度", url: "https://www.baidu.com/s?wd=" },
  DuckDuckGo: { name: "DuckDuckGo", url: "https://duckduckgo.com/?q=" },
};

export function engineName(engine: SearchEngine): string {
  return engine === "None" ? "" : ENGINES[engine].name;
}

export function webSearchUrl(engine: SearchEngine, query: string): string | null {
  if (engine === "None" || !query.trim()) return null;
  return ENGINES[engine].url + encodeURIComponent(query.trim());
}
