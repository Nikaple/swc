import * as swcHelpers from "@swc/helpers";
// @target: esnext, es2022, es2015, es5
var A = function A() {
    "use strict";
    swcHelpers.classCallCheck(this, A);
};
A.bar = A.foo + 1;
var __ = {
    writable: true,
    value: function() {
        A.foo + 2;
    }()
};
A.foo = 1;
