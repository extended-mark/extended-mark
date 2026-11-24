// The maths beyond arithmetic: roots, powers, logarithms and angles, which libdecnumber does (pow, exp, ln, sqrt)
// or num.rs does as series (the angles). decimal.js does the work, and every result is rounded into decimal128 once,
// halves to even, so it agrees with num.rs digit for digit.

import { Decimal } from "decimal.js";
import { Num, NumError } from "./num.js";

type Rounding = Decimal.Rounding;

const ROUND_HALF_UP: Rounding = 4;
const ROUND_HALF_EVEN: Rounding = 6;

/** decimal128: digits, and the largest and smallest adjusted exponents of a normal number. */
const DIGITS = 34;
const EMAX = 6144;
const EMIN = -6143;

// decimal.js never limits exponents itself: decimal128's limits are applied by `roundTo`, and by the number made.
const unbounded = { minE: -9e15, maxE: 9e15, toExpNeg: -9e15, toExpPos: 9e15, modulo: 1 as Decimal.Modulo, crypto: false };
/** Wide enough that one more rounding is still correct (2p+2 digits). */
const D70 = /* @__PURE__ */ Decimal.clone({ ...unbounded, precision: 70, rounding: ROUND_HALF_EVEN });
/** libdecnumber's working precision for `pow` with a power that isn't a small whole number. */
const D44 = /* @__PURE__ */ Decimal.clone({ ...unbounded, precision: 44, rounding: ROUND_HALF_EVEN });
/** Wide enough to hold the exact product of two numbers of up to 46 digits: `pow`'s whole powers. */
const EXACT = /* @__PURE__ */ Decimal.clone({ ...unbounded, precision: 200, rounding: ROUND_HALF_EVEN });
/** num.rs's `Wide`: 120 digits, rounding halves up as libdecnumber's default context does. */
const W = /* @__PURE__ */ Decimal.clone({ ...unbounded, precision: 120, rounding: ROUND_HALF_UP });
/** Wide's largest exponent (num.rs's wide_cx); its smallest, -99,999, is far below the series' 10^-130. */
const WIDE_EMAX = 99_999;

/** Past this, exp's result is beyond decimal128 either way: e^15000 > 10^6144, e^-15000 < 10^-6176. */
const MATH_LIMIT = 15_000;

interface Rounded {
  v: Decimal;
  overflow: boolean;
  /** Subnormal and inexact, as libdecnumber's Underflow. */
  underflow: boolean;
}

/** Rounds to a context of `digits` digits and these exponents, as libdecnumber does: a value below
 * the smallest normal number keeps only the digits down to the context's tiniest exponent. */
function roundTo(x: Decimal, digits: number, rm: Rounding, emin: number, emax: number): Rounded {
  if (!x.isFinite()) {
    return { v: x, overflow: true, underflow: false };
  }
  if (x.isZero()) {
    return { v: x, overflow: false, underflow: false };
  }
  if (x.e < emin) {
    const r = x.toDP(digits - 1 - emin, rm);
    return { v: r, overflow: false, underflow: !r.eq(x) };
  }
  // decimal.js keeps 7 digits a word: few enough words can't need rounding
  const r = x.d.length * 7 <= digits ? x : x.toSD(digits, rm);
  return { v: r, overflow: r.e > emax, underflow: false };
}

const ZERO = Num.zero();
const ONE = Num.int(1);

/** A number as decimal.js holds it. */
function dec(n: Num): Decimal {
  return new D70(n.exact());
}

/** A result rounded into decimal128, or the failure of one too large. */
function fin(x: Decimal): Num {
  if (!x.isFinite()) {
    throw new NumError("TooLarge");
  }
  return Num.rounded(x.toExponential());
}

/** `fin`, with a failure turned into "no answer", as num.rs's `from_wide(..).ok()`. */
function finOrNull(x: Decimal): Num | null {
  try {
    return fin(x);
  } catch (e) {
    if (e instanceof NumError) {
      return null;
    }
    throw e;
  }
}

export function sqrt(n: Num): Num | null {
  return n.isNegative() ? null : finOrNull(dec(n).sqrt());
}

/** `null` for 0 to a negative power, or a negative number to a power that isn't whole.
 * libdecnumber's decNumberPower, step for step: a whole power below 2^31 by repeated squaring
 * at 34 + (its digits) + 2 digits, halves to even, any other as exp(ln(x) × p) at 44 digits,
 * and the result rounded to 34 digits, halves to even, as every result is (page 2). */
export function pow(n: Num, p: Num): Num | null {
  if (n.isZero() && p.isNegative()) {
    return null;
  }
  if (n.isNegative() && !p.isInt()) {
    return null;
  }
  return power(dec(n), dec(p));
}

export function exp(n: Num): Num {
  const d = dec(n);
  if (d.gt(MATH_LIMIT)) {
    throw new NumError("TooLarge");
  }
  if (d.lt(-MATH_LIMIT)) {
    return ZERO;
  }
  return fin(d.exp());
}

/** `null` at 0 or below. */
export function ln(n: Num): Num | null {
  return n.isZero() || n.isNegative() ? null : finOrNull(dec(n).ln());
}

export function sin(n: Num): Num {
  return fin(series.sin(series.reduce(wide(n))));
}

export function cos(n: Num): Num {
  return fin(series.cos(series.reduce(wide(n))));
}

export function tan(n: Num): Num {
  const r = series.reduce(wide(n));
  const c = series.cos(r);
  if (c.isZero()) {
    throw new NumError("TooLarge");
  }
  return fin(series.div(series.sin(r), c));
}

/** `null` outside -1..1. */
export function asin(n: Num): Num | null {
  const w = series.asin(wide(n));
  return w === null ? null : finOrNull(w);
}

/** `null` outside -1..1. */
export function acos(n: Num): Num | null {
  const a = series.asin(wide(n));
  return a === null ? null : finOrNull(series.sub(series.halfPi(), a));
}

export function atan(n: Num): Num {
  return fin(series.atan(wide(n)));
}

/** The angle of the point (x, y), from -π to π. */
export function atan2(y: Num, x: Num): Num {
  return fin(series.atan2(wide(y), wide(x)));
}

export function sinh(n: Num): Num {
  return fin(series.hyperbolic(wide(n), "sinh"));
}

export function cosh(n: Num): Num {
  return fin(series.hyperbolic(wide(n), "cosh"));
}

export function tanh(n: Num): Num {
  return fin(series.hyperbolic(wide(n), "tanh"));
}

let PI: Num | undefined;

export function pi(): Num {
  return (PI ??= fin(series.pi()));
}

function wide(n: Num): Decimal {
  return new W(n.exact());
}

// ---- pow, as libdecnumber's decNumberPower

const BADINT = "bad";
const BIG = "big";

/** decGetInt: a whole number that fits (below 10^9, or above -2·10^9), or why not. */
function getInt(d: Decimal): number | typeof BADINT | typeof BIG {
  if (d.isZero()) {
    return 0;
  }
  if (!d.isInteger()) {
    return BADINT;
  }
  const integralLength = d.e + 1;
  if (integralLength > 10) {
    return BIG;
  }
  const n = Number(d.toFixed());
  if (integralLength === 10 && ((d.isNeg() && -n > 1999999997) || (!d.isNeg() && n > 999999999))) {
    return BIG;
  }
  return n;
}

function power(lhs: Decimal, rhs: Decimal): Num | null {
  const n = getInt(rhs);
  if (lhs.isZero()) {
    if (n === 0) {
      // 0 ** 0 is an invalid operation: a NaN, which num.rs reports as too large
      throw new NumError("TooLarge");
    }
    if (rhs.isNeg()) {
      throw new NumError("TooLarge");
    }
    return ZERO;
  }
  if (typeof n !== "number") {
    if (lhs.isNeg()) {
      // a negative number to a whole power too big for the integer path: invalid
      throw new NumError("TooLarge");
    }
    const ln = new D44(lhs).ln();
    if (ln.isZero()) {
      return ONE;
    }
    const m = ln.times(rhs);
    if (m.gt(MATH_LIMIT)) {
      throw new NumError("TooLarge");
    }
    if (m.lt(-MATH_LIMIT)) {
      return ZERO;
    }
    return fin(m.exp());
  }
  if (n === 0) {
    return ONE;
  }
  // the integer path: working digits, halves to even, inside decimal128's exponents
  const digits = DIGITS + String(Math.abs(n)).length + 2;
  let overflow = false;
  let underflow = false;
  const ctx = (x: Decimal): Decimal => {
    const r = roundTo(x, digits, ROUND_HALF_EVEN, EMIN, EMAX);
    if (r.overflow) {
      overflow = true;
      return new EXACT(Infinity);
    }
    if (r.underflow) {
      underflow = true;
    }
    return r.v;
  };
  let x = new EXACT(lhs);
  if (n < 0) {
    x = ctx(new EXACT(1).div(x));
  }
  let bits = Math.abs(n) | 0;
  let dac = new EXACT(1);
  let seen = false;
  for (let i = 1; ; i++) {
    if (overflow || (underflow && dac.isZero())) {
      break;
    }
    bits = (bits << 1) | 0;
    if (bits < 0) {
      seen = true;
      dac = ctx(dac.times(x));
    }
    if (i === 31) {
      break;
    }
    if (!seen) {
      continue;
    }
    dac = ctx(dac.times(dac));
  }
  if (overflow) {
    throw new NumError("TooLarge");
  }
  if (underflow && !(dac.e < EMIN || dac.isZero())) {
    // decFinalize keeps the working digits; num.rs's from_wide rounds them halves to even
    return fin(dac);
  }
  return fin(dac);
}

// ---- the maths libdecnumber doesn't do, as series in 120 digits, rounded to 34 at the end

const series = /* @__PURE__ */ (() => {
  /** W's operations already round to 120 digits, halves up: what is left of Wide's context is its
   * exponent limit, which these series never reach. */
  const c = (x: Decimal): Decimal => (x.isFinite() && x.e > WIDE_EMAX ? new W(Infinity) : x);

  function int(n: number): Decimal {
    return new W(n);
  }

  /** Below 10^-130: past every digit that matters. */
  function small(w: Decimal): boolean {
    return w.isZero() || w.e + 1 < -130;
  }

  function add(a: Decimal, b: Decimal): Decimal {
    return c(a.plus(b));
  }

  function sub(a: Decimal, b: Decimal): Decimal {
    return c(a.minus(b));
  }

  function mul(a: Decimal, b: Decimal): Decimal {
    return c(a.times(b));
  }

  function div(a: Decimal, b: Decimal): Decimal {
    return c(a.div(b));
  }

  /** libdecnumber's minus: 0 - x, so zero is never negative. */
  function neg(a: Decimal): Decimal {
    return a.isZero() ? new W(0) : a.neg();
  }

  function lt(a: Decimal, b: Decimal): boolean {
    return a.lt(b);
  }

  function sqrt(a: Decimal): Decimal {
    return c(a.sqrt());
  }

  function isNegative(a: Decimal): boolean {
    // libdecnumber's sign bit: a zero from a division can be negative
    return a.isNeg();
  }

  /** atan(1/n) for a whole n, by its series. */
  function atanInv(n: number): Decimal {
    const x = div(int(1), int(n));
    const x2 = mul(x, x);
    let term = x;
    let sum = x;
    for (let k = 1; ; k++) {
      term = mul(term, x2);
      const t = div(term, int(2 * k + 1));
      if (small(t)) {
        return sum;
      }
      sum = k % 2 === 1 ? sub(sum, t) : add(sum, t);
    }
  }

  let PI_WIDE: Decimal | undefined;

  /** π, by Machin's formula. */
  function pi(): Decimal {
    if (PI_WIDE === undefined) {
      PI_WIDE = sub(mul(int(16), atanInv(5)), mul(int(4), atanInv(239)));
    }
    return PI_WIDE;
  }

  function halfPi(): Decimal {
    return div(pi(), int(2));
  }

  /** The same angle, between -π and π. */
  function reduce(x: Decimal): Decimal {
    const twoPi = mul(pi(), int(2));
    const k = div(x, twoPi).toDP(0, ROUND_HALF_EVEN);
    return sub(x, mul(k, twoPi));
  }

  function sin(x: Decimal): Decimal {
    const x2 = mul(x, x);
    let term = x;
    let sum = x;
    for (let k = 1; ; k++) {
      term = neg(div(mul(term, x2), int(2 * k * (2 * k + 1))));
      if (small(term)) {
        return sum;
      }
      sum = add(sum, term);
    }
  }

  function cos(x: Decimal): Decimal {
    const x2 = mul(x, x);
    let term = int(1);
    let sum = int(1);
    for (let k = 1; ; k++) {
      term = neg(div(mul(term, x2), int((2 * k - 1) * (2 * k))));
      if (small(term)) {
        return sum;
      }
      sum = add(sum, term);
    }
  }

  function atan(x: Decimal): Decimal {
    const one = int(1);
    if (isNegative(x)) {
      return neg(atan(neg(x)));
    }
    if (lt(one, x)) {
      return sub(halfPi(), atan(div(one, x)));
    }
    // halve the angle until it is small: atan(x) = 2 atan(x / (1 + sqrt(1 + x²)))
    let y = x;
    let doublings = 0;
    const limit = new W("0.01");
    while (lt(limit, y)) {
      y = div(y, add(one, sqrt(add(one, mul(y, y)))));
      doublings += 1;
    }
    const y2 = mul(y, y);
    let term = y;
    let sum = y;
    for (let k = 1; ; k++) {
      term = neg(mul(term, y2));
      const t = div(term, int(2 * k + 1));
      if (small(t)) {
        break;
      }
      sum = add(sum, t);
    }
    for (let k = 0; k < doublings; k++) {
      sum = mul(sum, int(2));
    }
    return sum;
  }

  function asin(x: Decimal): Decimal | null {
    const one = int(1);
    const ax = isNegative(x) ? neg(x) : x;
    if (lt(one, ax)) {
      return null;
    }
    if (!lt(ax, one)) {
      const h = halfPi();
      return isNegative(x) ? neg(h) : h;
    }
    return atan(div(x, sqrt(sub(one, mul(x, x)))));
  }

  function atan2(y: Decimal, x: Decimal): Decimal {
    if (x.isZero()) {
      if (y.isZero()) {
        return int(0);
      }
      const h = halfPi();
      return isNegative(y) ? neg(h) : h;
    }
    const a = atan(div(y, x));
    if (!isNegative(x)) {
      return a;
    }
    return isNegative(y) ? sub(a, pi()) : add(a, pi());
  }

  function hyperbolic(x: Decimal, which: "sinh" | "cosh" | "tanh"): Decimal {
    const one = int(1);
    const ax = isNegative(x) ? neg(x) : x;
    const big = int(20000);
    if (lt(big, ax)) {
      if (which === "tanh") {
        return isNegative(x) ? neg(one) : one;
      }
      throw new NumError("TooLarge");
    }
    let sinh: Decimal;
    if (lt(ax, one)) {
      // sinh near 0 by its series, to keep its digits
      const x2 = mul(x, x);
      let term = x;
      let s = x;
      for (let k = 1; ; k++) {
        term = div(mul(term, x2), int(2 * k * (2 * k + 1)));
        if (small(term)) {
          break;
        }
        s = add(s, term);
      }
      sinh = s;
    } else {
      const e = c(x.exp());
      sinh = div(sub(e, div(one, e)), int(2));
    }
    const e = c(x.exp());
    const cosh = div(add(e, div(one, e)), int(2));
    return which === "sinh" ? sinh : which === "cosh" ? cosh : div(sinh, cosh);
  }

  return { int, add, sub, div, pi, halfPi, reduce, sin, cos, atan, asin, atan2, hyperbolic };
})();
