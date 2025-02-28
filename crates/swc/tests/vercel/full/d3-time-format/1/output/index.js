import * as a from "@swc/helpers";
import { timeDay as b, timeSunday as c, timeMonday as d, timeThursday as e, timeYear as f, utcDay as g, utcSunday as h, utcMonday as i, utcThursday as j, utcYear as k } from "d3-time";
function l(a) {
    if (0 <= a.y && a.y < 100) {
        var b = new Date(-1, a.m, a.d, a.H, a.M, a.S, a.L);
        return b.setFullYear(a.y), b;
    }
    return new Date(a.y, a.m, a.d, a.H, a.M, a.S, a.L);
}
function m(a) {
    if (0 <= a.y && a.y < 100) {
        var b = new Date(Date.UTC(-1, a.m, a.d, a.H, a.M, a.S, a.L));
        return b.setUTCFullYear(a.y), b;
    }
    return new Date(Date.UTC(a.y, a.m, a.d, a.H, a.M, a.S, a.L));
}
function n(a, b, c) {
    return {
        y: a,
        m: b,
        d: c,
        H: 0,
        M: 0,
        S: 0,
        L: 0
    };
}
export default function o(c) {
    var e = function(b, c) {
        return function(f) {
            var e, h, j, g = [], d = -1, i = 0, k = b.length;
            for(a._instanceof(f, Date) || (f = new Date(+f)); ++d < k;)37 === b.charCodeAt(d) && (g.push(b.slice(i, d)), null != (h = p[e = b.charAt(++d)]) ? e = b.charAt(++d) : h = "e" === e ? " " : "0", (j = c[e]) && (e = j(f, h)), g.push(e), i = d + 1);
            return g.push(b.slice(i, d)), g.join("");
        };
    }, aa = function(a, c) {
        return function(j) {
            var f, h, e = n(1900, void 0, 1);
            if (ua(e, a, j += "", 0) != j.length) return null;
            if ("Q" in e) return new Date(e.Q);
            if ("s" in e) return new Date(1000 * e.s + ("L" in e ? e.L : 0));
            if (!c || "Z" in e || (e.Z = 0), "p" in e && (e.H = e.H % 12 + 12 * e.p), void 0 === e.m && (e.m = "q" in e ? e.q : 0), "V" in e) {
                if (e.V < 1 || e.V > 53) return null;
                "w" in e || (e.w = 1), "Z" in e ? (f = (h = (f = m(n(e.y, 0, 1))).getUTCDay()) > 4 || 0 === h ? i.ceil(f) : i(f), f = g.offset(f, (e.V - 1) * 7), e.y = f.getUTCFullYear(), e.m = f.getUTCMonth(), e.d = f.getUTCDate() + (e.w + 6) % 7) : (f = (h = (f = l(n(e.y, 0, 1))).getDay()) > 4 || 0 === h ? d.ceil(f) : d(f), f = b.offset(f, (e.V - 1) * 7), e.y = f.getFullYear(), e.m = f.getMonth(), e.d = f.getDate() + (e.w + 6) % 7);
            } else ("W" in e || "U" in e) && ("w" in e || (e.w = "u" in e ? e.u % 7 : "W" in e ? 1 : 0), h = "Z" in e ? m(n(e.y, 0, 1)).getUTCDay() : l(n(e.y, 0, 1)).getDay(), e.m = 0, e.d = "W" in e ? (e.w + 6) % 7 + 7 * e.W - (h + 5) % 7 : e.w + 7 * e.U - (h + 6) % 7);
            return "Z" in e ? (e.H += e.Z / 100 | 0, e.M += e.Z % 100, m(e)) : l(e);
        };
    }, ua = function(g, b, e, a) {
        for(var c, f, d = 0, h = b.length, i = e.length; d < h;){
            if (a >= i) return -1;
            if (37 === (c = b.charCodeAt(d++))) {
                if (!(f = Qa[(c = b.charAt(d++)) in p ? b.charAt(d++) : c]) || (a = f(g, e, a)) < 0) return -1;
            } else if (c != e.charCodeAt(a++)) return -1;
        }
        return a;
    }, j = c.dateTime, k = c.date, o = c.time, q = c.periods, r = c.days, s = c.shortDays, t = c.months, u = c.shortMonths, Ga = v(q), Ha = w(q), Ia = v(r), Ja = w(r), Ka = v(s), La = w(s), Ma = v(t), Na = w(t), Oa = v(u), Pa = w(u), f = {
        a: function(a) {
            return s[a.getDay()];
        },
        A: function(a) {
            return r[a.getDay()];
        },
        b: function(a) {
            return u[a.getMonth()];
        },
        B: function(a) {
            return t[a.getMonth()];
        },
        c: null,
        d: R,
        e: R,
        f: W,
        g: fa,
        G: ha,
        H: S,
        I: T,
        j: U,
        L: V,
        m: X,
        M: Y,
        p: function(a) {
            return q[+(a.getHours() >= 12)];
        },
        q: function(a) {
            return 1 + ~~(a.getMonth() / 3);
        },
        Q: Ea,
        s: Fa,
        S: Z,
        u: $,
        U: _,
        V: ba,
        w: ca,
        W: da,
        x: null,
        X: null,
        y: ea,
        Y: ga,
        Z: ia,
        "%": Da
    }, h = {
        a: function(a) {
            return s[a.getUTCDay()];
        },
        A: function(a) {
            return r[a.getUTCDay()];
        },
        b: function(a) {
            return u[a.getUTCMonth()];
        },
        B: function(a) {
            return t[a.getUTCMonth()];
        },
        c: null,
        d: ja,
        e: ja,
        f: oa,
        g: za,
        G: Ba,
        H: ka,
        I: la,
        j: ma,
        L: na,
        m: pa,
        M: qa,
        p: function(a) {
            return q[+(a.getUTCHours() >= 12)];
        },
        q: function(a) {
            return 1 + ~~(a.getUTCMonth() / 3);
        },
        Q: Ea,
        s: Fa,
        S: ra,
        u: sa,
        U: ta,
        V: va,
        w: wa,
        W: xa,
        x: null,
        X: null,
        y: ya,
        Y: Aa,
        Z: Ca,
        "%": Da
    }, Qa = {
        a: function(c, d, b) {
            var a = Ka.exec(d.slice(b));
            return a ? (c.w = La.get(a[0].toLowerCase()), b + a[0].length) : -1;
        },
        A: function(c, d, b) {
            var a = Ia.exec(d.slice(b));
            return a ? (c.w = Ja.get(a[0].toLowerCase()), b + a[0].length) : -1;
        },
        b: function(c, d, b) {
            var a = Oa.exec(d.slice(b));
            return a ? (c.m = Pa.get(a[0].toLowerCase()), b + a[0].length) : -1;
        },
        B: function(c, d, b) {
            var a = Ma.exec(d.slice(b));
            return a ? (c.m = Na.get(a[0].toLowerCase()), b + a[0].length) : -1;
        },
        c: function(a, b, c) {
            return ua(a, j, b, c);
        },
        d: H,
        e: H,
        f: N,
        g: D,
        G: C,
        H: J,
        I: J,
        j: I,
        L: M,
        m: G,
        M: K,
        p: function(c, d, b) {
            var a = Ga.exec(d.slice(b));
            return a ? (c.p = Ha.get(a[0].toLowerCase()), b + a[0].length) : -1;
        },
        q: F,
        Q: P,
        s: Q,
        S: L,
        u: y,
        U: z,
        V: A,
        w: x,
        W: B,
        x: function(a, b, c) {
            return ua(a, k, b, c);
        },
        X: function(a, b, c) {
            return ua(a, o, b, c);
        },
        y: D,
        Y: C,
        Z: E,
        "%": O
    };
    return f.x = e(k, f), f.X = e(o, f), f.c = e(j, f), h.x = e(k, h), h.X = e(o, h), h.c = e(j, h), {
        format: function(b) {
            var a = e(b += "", f);
            return a.toString = function() {
                return b;
            }, a;
        },
        parse: function(b) {
            var a = aa(b += "", !1);
            return a.toString = function() {
                return b;
            }, a;
        },
        utcFormat: function(b) {
            var a = e(b += "", h);
            return a.toString = function() {
                return b;
            }, a;
        },
        utcParse: function(b) {
            var a = aa(b += "", !0);
            return a.toString = function() {
                return b;
            }, a;
        }
    };
};
var p = {
    "-": "",
    "_": " ",
    "0": "0"
}, q = /^\s*\d+/, r = /^%/, s = /[\\^$*+?|[\]().{}]/g;
function t(a, f, c) {
    var d = a < 0 ? "-" : "", b = (d ? -a : a) + "", e = b.length;
    return d + (e < c ? new Array(c - e + 1).join(f) + b : b);
}
function u(a) {
    return a.replace(s, "\\$&");
}
function v(a) {
    return new RegExp("^(?:" + a.map(u).join("|") + ")", "i");
}
function w(a) {
    return new Map(a.map(function(a, b) {
        return [
            a.toLowerCase(),
            b
        ];
    }));
}
function x(c, d, a) {
    var b = q.exec(d.slice(a, a + 1));
    return b ? (c.w = +b[0], a + b[0].length) : -1;
}
function y(c, d, a) {
    var b = q.exec(d.slice(a, a + 1));
    return b ? (c.u = +b[0], a + b[0].length) : -1;
}
function z(c, d, a) {
    var b = q.exec(d.slice(a, a + 2));
    return b ? (c.U = +b[0], a + b[0].length) : -1;
}
function A(c, d, a) {
    var b = q.exec(d.slice(a, a + 2));
    return b ? (c.V = +b[0], a + b[0].length) : -1;
}
function B(c, d, a) {
    var b = q.exec(d.slice(a, a + 2));
    return b ? (c.W = +b[0], a + b[0].length) : -1;
}
function C(c, d, a) {
    var b = q.exec(d.slice(a, a + 4));
    return b ? (c.y = +b[0], a + b[0].length) : -1;
}
function D(c, d, b) {
    var a = q.exec(d.slice(b, b + 2));
    return a ? (c.y = +a[0] + (+a[0] > 68 ? 1900 : 2000), b + a[0].length) : -1;
}
function E(c, d, b) {
    var a = /^(Z)|([+-]\d\d)(?::?(\d\d))?/.exec(d.slice(b, b + 6));
    return a ? (c.Z = a[1] ? 0 : -(a[2] + (a[3] || "00")), b + a[0].length) : -1;
}
function F(c, d, a) {
    var b = q.exec(d.slice(a, a + 1));
    return b ? (c.q = 3 * b[0] - 3, a + b[0].length) : -1;
}
function G(c, d, a) {
    var b = q.exec(d.slice(a, a + 2));
    return b ? (c.m = b[0] - 1, a + b[0].length) : -1;
}
function H(c, d, a) {
    var b = q.exec(d.slice(a, a + 2));
    return b ? (c.d = +b[0], a + b[0].length) : -1;
}
function I(c, d, a) {
    var b = q.exec(d.slice(a, a + 3));
    return b ? (c.m = 0, c.d = +b[0], a + b[0].length) : -1;
}
function J(c, d, a) {
    var b = q.exec(d.slice(a, a + 2));
    return b ? (c.H = +b[0], a + b[0].length) : -1;
}
function K(c, d, a) {
    var b = q.exec(d.slice(a, a + 2));
    return b ? (c.M = +b[0], a + b[0].length) : -1;
}
function L(c, d, a) {
    var b = q.exec(d.slice(a, a + 2));
    return b ? (c.S = +b[0], a + b[0].length) : -1;
}
function M(c, d, a) {
    var b = q.exec(d.slice(a, a + 3));
    return b ? (c.L = +b[0], a + b[0].length) : -1;
}
function N(c, d, a) {
    var b = q.exec(d.slice(a, a + 6));
    return b ? (c.L = Math.floor(b[0] / 1000), a + b[0].length) : -1;
}
function O(d, c, a) {
    var b = r.exec(c.slice(a, a + 1));
    return b ? a + b[0].length : -1;
}
function P(c, d, b) {
    var a = q.exec(d.slice(b));
    return a ? (c.Q = +a[0], b + a[0].length) : -1;
}
function Q(c, d, b) {
    var a = q.exec(d.slice(b));
    return a ? (c.s = +a[0], b + a[0].length) : -1;
}
function R(a, b) {
    return t(a.getDate(), b, 2);
}
function S(a, b) {
    return t(a.getHours(), b, 2);
}
function T(a, b) {
    return t(a.getHours() % 12 || 12, b, 2);
}
function U(a, c) {
    return t(1 + b.count(f(a), a), c, 3);
}
function V(a, b) {
    return t(a.getMilliseconds(), b, 3);
}
function W(a, b) {
    return V(a, b) + "000";
}
function X(a, b) {
    return t(a.getMonth() + 1, b, 2);
}
function Y(a, b) {
    return t(a.getMinutes(), b, 2);
}
function Z(a, b) {
    return t(a.getSeconds(), b, 2);
}
function $(b) {
    var a = b.getDay();
    return 0 === a ? 7 : a;
}
function _(a, b) {
    return t(c.count(f(a) - 1, a), b, 2);
}
function aa(a) {
    var b = a.getDay();
    return b >= 4 || 0 === b ? e(a) : e.ceil(a);
}
function ba(a, b) {
    return a = aa(a), t(e.count(f(a), a) + (4 === f(a).getDay()), b, 2);
}
function ca(a) {
    return a.getDay();
}
function da(a, b) {
    return t(d.count(f(a) - 1, a), b, 2);
}
function ea(a, b) {
    return t(a.getFullYear() % 100, b, 2);
}
function fa(a, b) {
    return t((a = aa(a)).getFullYear() % 100, b, 2);
}
function ga(a, b) {
    return t(a.getFullYear() % 10000, b, 4);
}
function ha(a, c) {
    var b = a.getDay();
    return t((a = b >= 4 || 0 === b ? e(a) : e.ceil(a)).getFullYear() % 10000, c, 4);
}
function ia(b) {
    var a = b.getTimezoneOffset();
    return (a > 0 ? "-" : (a *= -1, "+")) + t(a / 60 | 0, "0", 2) + t(a % 60, "0", 2);
}
function ja(a, b) {
    return t(a.getUTCDate(), b, 2);
}
function ka(a, b) {
    return t(a.getUTCHours(), b, 2);
}
function la(a, b) {
    return t(a.getUTCHours() % 12 || 12, b, 2);
}
function ma(a, b) {
    return t(1 + g.count(k(a), a), b, 3);
}
function na(a, b) {
    return t(a.getUTCMilliseconds(), b, 3);
}
function oa(a, b) {
    return na(a, b) + "000";
}
function pa(a, b) {
    return t(a.getUTCMonth() + 1, b, 2);
}
function qa(a, b) {
    return t(a.getUTCMinutes(), b, 2);
}
function ra(a, b) {
    return t(a.getUTCSeconds(), b, 2);
}
function sa(b) {
    var a = b.getUTCDay();
    return 0 === a ? 7 : a;
}
function ta(a, b) {
    return t(h.count(k(a) - 1, a), b, 2);
}
function ua(a) {
    var b = a.getUTCDay();
    return b >= 4 || 0 === b ? j(a) : j.ceil(a);
}
function va(a, b) {
    return a = ua(a), t(j.count(k(a), a) + (4 === k(a).getUTCDay()), b, 2);
}
function wa(a) {
    return a.getUTCDay();
}
function xa(a, b) {
    return t(i.count(k(a) - 1, a), b, 2);
}
function ya(a, b) {
    return t(a.getUTCFullYear() % 100, b, 2);
}
function za(a, b) {
    return t((a = ua(a)).getUTCFullYear() % 100, b, 2);
}
function Aa(a, b) {
    return t(a.getUTCFullYear() % 10000, b, 4);
}
function Ba(a, c) {
    var b = a.getUTCDay();
    return t((a = b >= 4 || 0 === b ? j(a) : j.ceil(a)).getUTCFullYear() % 10000, c, 4);
}
function Ca() {
    return "+0000";
}
function Da() {
    return "%";
}
function Ea(a) {
    return +a;
}
function Fa(a) {
    return Math.floor(+a / 1000);
}
