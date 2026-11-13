// Line diff (LCS) for comparing two bodies.

export type DiffOp = { kind: "same" | "del" | "add"; a?: number; b?: number; text: string };

/** Pretty-print JSON so structurally equal bodies line up; other text is returned unchanged. */
export function normalise(text: string | null): string {
  if (!text) return "";
  const t = text.trim();
  if (t.startsWith("{") || t.startsWith("[")) {
    try {
      return JSON.stringify(JSON.parse(t), null, 2);
    } catch {
      /* not JSON */
    }
  }
  return text;
}

/** Diff two texts line by line. Falls back to a whole-block replace for very large inputs. */
export function diffLines(a: string, b: string, maxCells = 4_000_000): DiffOp[] {
  const A = a === "" ? [] : a.split("\n");
  const B = b === "" ? [] : b.split("\n");
  // trim the common prefix and suffix so the LCS table stays small
  let pre = 0;
  while (pre < A.length && pre < B.length && A[pre] === B[pre]) pre++;
  let suf = 0;
  while (suf < A.length - pre && suf < B.length - pre && A[A.length - 1 - suf] === B[B.length - 1 - suf]) suf++;
  const a2 = A.slice(pre, A.length - suf);
  const b2 = B.slice(pre, B.length - suf);
  const out: DiffOp[] = [];
  for (let i = 0; i < pre; i++) out.push({ kind: "same", a: i + 1, b: i + 1, text: A[i] });

  const n = a2.length, m = b2.length;
  if (n * m > maxCells) {
    a2.forEach((t, i) => out.push({ kind: "del", a: pre + i + 1, text: t }));
    b2.forEach((t, j) => out.push({ kind: "add", b: pre + j + 1, text: t }));
  } else {
    // dp[i][j] = LCS length of a2[i..] and b2[j..]
    const w = m + 1;
    const dp = new Uint32Array((n + 1) * w);
    for (let i = n - 1; i >= 0; i--)
      for (let j = m - 1; j >= 0; j--)
        dp[i * w + j] = a2[i] === b2[j] ? dp[(i + 1) * w + j + 1] + 1 : Math.max(dp[(i + 1) * w + j], dp[i * w + j + 1]);
    let i = 0, j = 0;
    while (i < n || j < m) {
      if (i < n && j < m && a2[i] === b2[j]) {
        out.push({ kind: "same", a: pre + i + 1, b: pre + j + 1, text: a2[i] });
        i++;
        j++;
      } else if (j < m && (i >= n || dp[i * w + j + 1] > dp[(i + 1) * w + j])) {
        out.push({ kind: "add", b: pre + j + 1, text: b2[j] });
        j++;
      } else {
        out.push({ kind: "del", a: pre + i + 1, text: a2[i] });
        i++;
      }
    }
  }
  for (let k = 0; k < suf; k++) {
    const ia = A.length - suf + k, ib = B.length - suf + k;
    out.push({ kind: "same", a: ia + 1, b: ib + 1, text: A[ia] });
  }
  return out;
}
